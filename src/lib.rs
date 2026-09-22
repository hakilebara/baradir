mod http;
mod runtime;
mod slug;
mod wit;

use http::InstanceState;
use hyper::server::conn::http1;
use runtime::Runtime;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Result};
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::ProxyPre;

pub struct Config {
    tcp_port: u16,
    base_domain: String,
    env_folder: String,
}

impl Config {
    pub fn new(tcp_port: u16, base_domain: String, env_folder: String) -> Config {
        Config {
            tcp_port,
            base_domain,
            env_folder,
        }
    }
}

pub struct App {
    pub name: String,
    pub filepath: String,
    pub pre: Option<ProxyPre<InstanceState>>,
}

pub async fn run(config: Config, conn: Connection) -> Result<()> {
    let engine = Engine::default();

    let apps = Arc::new(Mutex::new(HashMap::new()));

    {
        let mut stmt = conn.prepare("SELECT id, name, filepath FROM app")?;
        let app_iter = stmt.query_map([], |row| {
            Ok(App {
                name: row.get(1)?,
                filepath: row.get(2)?,
                pre: None,
            })
        })?;

        for app in app_iter {
            let mut app = app?;

            // Compile the component on the command line to machine code
            let component = Component::from_file(&engine, app.filepath.clone())?;

            // Prepare the `ProxyPre` which is a pre-instantiated version of the
            // component. This will make per-request instantiation
            // much quicker.
            let mut linker = Linker::new(&engine);
            wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
            wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;

            // confusing syntax to say the least
            crate::wit::bindings::ManagerApp::add_to_linker::<InstanceState, HasSelf<InstanceState>>(
                &mut linker,
                |state| state,
            )?;

            let pre = ProxyPre::new(linker.instantiate_pre(&component)?)?;
            app.pre = Some(pre);

            apps.lock().unwrap().insert(app.name.clone(), app);
        }
    }

    let listener = TcpListener::bind(format!("127.0.0.1:{}", config.tcp_port)).await?;
    println!("Listening on {}", listener.local_addr()?);

    // Prepare our server state and start listening for connections.
    let runtime = Arc::new(Runtime {
        apps,
        config,
        conn: Arc::new(Mutex::new(conn)),
        engine: engine.clone(),
    });

    loop {
        // Accept a TCP connection and serve all of its requests in a separate
        // tokio task. Note that for now this only works with HTTP/1.1.
        let (client, addr) = listener.accept().await?;
        println!("serving new client from {addr}");

        let runtime = runtime.clone();
        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .keep_alive(false)
                .serve_connection(
                    TokioIo::new(client),
                    hyper::service::service_fn(move |req| {
                        let runtime = runtime.clone();
                        async move { runtime.handle_request(req).await }
                    }),
                )
                .await
            {
                eprintln!("error serving client[{addr}]: {e:?}");
            }
        });
    }
}
