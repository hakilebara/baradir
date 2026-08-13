use hyper::server::conn::http1;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use wasmtime::bail;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Result, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::ProxyPre;
use wasmtime_wasi_http::p2::bindings::http::types::Scheme;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;
use wasmtime_wasi_http::{
    WasiHttpCtx,
    p2::{WasiHttpCtxView, WasiHttpView},
};

pub struct Config {
    tcp_port: u16,
}

impl Config {
    pub fn new(tcp_port: u16) -> Config {
        Config { tcp_port }
    }
}

struct App<'a> {
    filepath: &'a str,
    pre: Option<ProxyPre<MyClientState>>,
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
    let server = Arc::new(MyServer { apps });
    // let listener = TcpListener::bind(format!("127.0.0.1:{}", args.port)).await?;
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

struct MyServer<'a> {
    apps: HashMap<&'a str, App<'a>>,
}

impl MyServer<'_> {
    async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<HyperOutgoingBody>> {
        let raw_host = req.headers().get("host").unwrap().to_str().unwrap();
        let v: Vec<_> = raw_host.split('.').take(2).collect();
        let app_name = v[0];
        let _env = v[1];

        if !self.apps.contains_key(app_name) {
            panic!("APP not found")
        }

        // Create per-http-request state within a `Store` and prepare the
        // initial resources  passed to the `handle` function.
        let pre = self.apps.get(app_name).unwrap().pre.clone().unwrap();
        let mut store = Store::new(
            pre.engine(),
            MyClientState {
                table: ResourceTable::new(),
                wasi: WasiCtx::builder().inherit_stdio().build(),
                http: WasiHttpCtx::new(),
            },
        );
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let req = store
            .data_mut()
            .http()
            .new_incoming_request(Scheme::Http, req)?;
        let out = store.data_mut().http().new_response_outparam(sender)?;

        // Run the http request itself in a separate task so the task can
        // optionally continue to execute beyond after the initial
        // headers/response code are sent.
        let task = tokio::task::spawn(async move {
            let proxy = pre.instantiate_async(&mut store).await?;

            if let Err(e) = proxy
                .wasi_http_incoming_handler()
                .call_handle(store, req, out)
                .await
            {
                return Err(e);
            }

            Ok(())
        });

        match receiver.await {
            // If the client calls `response-outparam::set` then one of these
            // methods will be called.
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(e)) => Err(e.into()),

            // Otherwise the `sender` will get dropped along with the `Store`
            // meaning that the oneshot will get disconnected and here we can
            // inspect the `task` result to see what happened
            Err(_) => {
                let e = match task.await {
                    Ok(Ok(())) => {
                        bail!("guest never invoked `response-outparam::set` method")
                    }
                    Ok(Err(e)) => e,
                    Err(e) => e.into(),
                };
                return Err(e.context("guest never invoked `response-outparam::set` method"));
            }
        }
    }
}

struct MyClientState {
    wasi: WasiCtx,
    http: WasiHttpCtx,
    table: ResourceTable,
}

impl WasiView for MyClientState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for MyClientState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: Default::default(),
        }
    }
}
