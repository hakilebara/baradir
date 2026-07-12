use wasmtime::bail;
use hyper::server::conn::http1;
use std::sync::Arc;
use tokio::net::TcpListener;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Result, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::p2::bindings::ProxyPre;
use wasmtime_wasi_http::p2::bindings::http::types::Scheme;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::{WasiHttpCtx, p2::{WasiHttpView, WasiHttpCtxView}};

#[tokio::main]
async fn main() -> Result<()> {
    let component = std::env::args().nth(1).unwrap();

    // Prepare the `Engine` for Wasmtime
    let engine = Engine::default();

    // Compile the component on the command line to machine code
    let component = Component::from_file(&engine, &component)?;

    // Prepare the `ProxyPre` which is a pre-instantiated version of the
    // component that we have. This will make per-request instantiation
    // much quicker.
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;
    let pre = ProxyPre::new(linker.instantiate_pre(&component)?)?;

    // Prepare our server state and start listening for connections.
    let server = Arc::new(MyServer { pre });
    let listener = TcpListener::bind("127.0.0.1:8000").await?;
    println!("Listening on {}", listener.local_addr()?);

    loop {
        // Accept a TCP connection and serve all of its requests in a separate
        // tokio task. Note that for now this only works with HTTP/1.1.
        let (client, addr) = listener.accept().await?;
        println!("serving new client from {addr}");

        let server = server.clone();
        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .keep_alive(true)
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

struct MyServer {
    pre: ProxyPre<MyClientState>,
}

impl MyServer {
    async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<HyperOutgoingBody>> {
        // Create per-http-request state within a `Store` and prepare the
        // initial resources  passed to the `handle` function.
        let mut store = Store::new(
            self.pre.engine(),
            MyClientState {
                table: ResourceTable::new(),
                wasi: WasiCtx::builder().inherit_stdio().build(),
                http: WasiHttpCtx::new(),
            },
        );
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let req = store.data_mut().http().new_incoming_request(Scheme::Http, req)?;
        let out = store.data_mut().http().new_response_outparam(sender)?;
        let pre = self.pre.clone();

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
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
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
