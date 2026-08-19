mod http;
mod slug;

use http::{MyClientState, MyServer};
use hyper::server::conn::http1;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Result};
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::ProxyPre;

pub struct Config {
    tcp_port: u16,
    conn: Connection,
}

impl Config {
    pub fn new(tcp_port: u16, conn: Connection) -> Config {
        Config { tcp_port, conn }
    }
}

pub struct App<'a> {
    pub filepath: &'a str,
    pub pre: Option<ProxyPre<MyClientState>>,
}

pub async fn run(config: Config) -> Result<()> {
    // Prepare the `Engine` for Wasmtime
    let engine = Engine::default();

    let mut apps = HashMap::from([
        (
            "hello",
            App {
                filepath: "apps/hello-http.wasm",
                pre: None,
            },
        ),
        (
            "manager",
            App {
                filepath: "apps/manager.wasm",
                pre: None,
            },
        ),
    ]);

    // Compile the component on the command line to machine code
    for (_name, app) in &mut apps {
        let component = Component::from_file(&engine, app.filepath)?;

        // Prepare the `ProxyPre` which is a pre-instantiated version of the
        // component. This will make per-request instantiation
        // much quicker.
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;
        let pre = ProxyPre::new(linker.instantiate_pre(&component)?)?;
        app.pre = Some(pre);
    }

    // Prepare our server state and start listening for connections.
    let server = Arc::new(MyServer {
        apps,
        conn: Arc::new(Mutex::new(config.conn)),
    });
    let listener = TcpListener::bind(format!("127.0.0.1:{}", config.tcp_port)).await?;
    println!("Listening on {}", listener.local_addr()?);

    loop {
        // Accept a TCP connection and serve all of its requests in a separate
        // tokio task. Note that for now this only works with HTTP/1.1.
        let (client, addr) = listener.accept().await?;
        println!("serving new client from {addr}");

        let server = server.clone();
        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .keep_alive(false)
                .serve_connection(
                    TokioIo::new(client),
                    hyper::service::service_fn(move |req| {
                        let server = server.clone();
                        async move { server.handle_request(req).await }
                    }),
                )
                .await
            {
                eprintln!("error serving client[{addr}]: {e:?}");
            }
        });
    }
}
