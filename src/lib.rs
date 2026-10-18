mod executor;
mod http;
mod runtime;
mod slug;
mod wit;

use executor::InstanceState;
use hyper::server::conn::http1;
use runtime::{App, Runtime};
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use wasmtime::Engine;
use wasmtime::component::{Component, HasSelf};
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::ProxyPre;

pub struct Config {
    public_addr: String,
    private_addr: Option<String>,
    tcp_port: u16,
    base_domain: String,
    env_folder: String,
}

#[derive(Clone, Copy, Debug)]
pub enum Access {
    Public,
    Private,
}

impl Config {
    pub fn new(
        public_addr: String,
        private_addr: Option<String>,
        base_domain: String,
        env_folder: String,
    ) -> anyhow::Result<Config> {
        let tcp_port = public_addr
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
            .ok_or_else(|| {
                anyhow::anyhow!("invalid public address `{public_addr}`: expected host:port")
            })?;

        if let Some(addr) = &private_addr {
            addr.rsplit(':')
                .next()
                .and_then(|port| port.parse::<u16>().ok())
                .ok_or_else(|| {
                    anyhow::anyhow!("invalid private address `{addr}`: expected host:port")
                })?;
        }

        Ok(Config {
            public_addr,
            private_addr,
            tcp_port,
            base_domain,
            env_folder,
        })
    }
}

pub async fn run(config: Config, conn: Connection) -> anyhow::Result<()> {
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

            let mut linker = Runtime::build_linker(&engine)?;

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

    let private_listener = match &config.private_addr {
        Some(addr) => Some(
            TcpListener::bind(addr)
                .await
                .map_err(|e| anyhow::anyhow!("failed to bind private socket {addr}: {e}"))?,
        ),
        None => None,
    };

    let listener = TcpListener::bind(&config.public_addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind public socket {}: {e}", config.public_addr))?;
    println!("Listening on {}", listener.local_addr()?);

    if let Some(pl) = &private_listener {
        println!("Private socket listening on {}", pl.local_addr()?);
    }

    // Prepare our server state and start listening for connections.
    let runtime = Arc::new(Runtime {
        apps,
        config: config,
        conn: Arc::new(Mutex::new(conn)),
        engine: engine.clone(),
    });

    match private_listener {
        Some(private_listener) => tokio::select! {
            res = serve(listener, runtime.clone(), Access::Public) => res,
            res = serve(private_listener, runtime, Access::Private) => res,
        },
        None => serve(listener, runtime, Access::Public).await,
    }
}

async fn serve(listener: TcpListener, runtime: Arc<Runtime>, access: Access) -> anyhow::Result<()> {
    loop {
        // Accept a TCP connection and serve all of its requests in a separate
        // tokio task. Note that for now this only works with HTTP/1.1.
        let (client, addr) = listener.accept().await?;
        println!("[{access:?}] serving new client from {addr}");

        let runtime = runtime.clone();
        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .keep_alive(false)
                .serve_connection(
                    TokioIo::new(client),
                    hyper::service::service_fn(move |req| {
                        let runtime = runtime.clone();
                        async move { runtime.handle_request(req, access).await }
                    }),
                )
                .await
            {
                eprintln!("error serving client[{addr}]: {e:?}");
            }
        });
    }
}
