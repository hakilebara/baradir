use crate::db::Db;
use crate::executor::ManagerInstanceState;
use crate::registry::Registry;
use crate::routes::{Route, route};
use crate::runtime::{App, DbStore, ManagerApp, Runtime};
use crate::{Access, Config};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::StatusCode;
use hyper::server::conn::http1;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use wasmtime::Engine;
use wasmtime::component::Linker;
use wasmtime::component::{Component, HasSelf};
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::ProxyPre;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;

pub async fn run(config: Config, db: Db) -> anyhow::Result<()> {
    let engine = Engine::default();

    let registry = Registry::new(config.registry_dir());

    let app_store = Arc::new(Mutex::new(HashMap::new()));

    // pre-compile manager component
    // TODO: refactor into its own function
    static MANAGER_WASM_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/manager.wasm"));
    let component = wasmtime::component::Component::from_binary(&engine, MANAGER_WASM_BYTES)?;
    let mut linker: Linker<ManagerInstanceState> = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;
    crate::wit::bindings::ManagerApp::add_to_linker::<
        ManagerInstanceState,
        HasSelf<ManagerInstanceState>,
    >(&mut linker, |state| state)?;
    let manager = ManagerApp {
        // `ProxyPre` is a pre-instantiated version of the
        // component. This will make per-request instantiation much quicker.
        pre: ProxyPre::new(linker.instantiate_pre(&component)?)?,
    };

    for row in db.get_apps()? {
        let mut app: App = row.into();

        // Compile the component on the command line to machine code
        let component = Component::from_file(&engine, registry.path(&app.digest))?;

        let linker = Runtime::build_linker(&engine)?;

        let pre = ProxyPre::new(linker.instantiate_pre(&component)?)?;

        app.pre = Some(pre);
        app_store.lock().unwrap().insert(app.name.clone(), app);
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
    let db = Arc::new(Mutex::new(db));
    let admin = baradir_admin::Admin::new(DbStore::new(db.clone()));

    let runtime = Arc::new(Runtime {
        db,
        admin,
        manager,
        app_store,
        config: config,
        engine: engine.clone(),
        registry,
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

fn empty(status: StatusCode) -> hyper::Response<HyperOutgoingBody> {
    hyper::Response::builder()
        .status(status)
        .body(
            http_body_util::Empty::<bytes::Bytes>::new()
                .map_err(|_| unreachable!("infallible"))
                .boxed_unsync(),
        )
        .unwrap()
}

fn bytes_body(body: Bytes) -> HyperOutgoingBody {
    Full::new(body)
        .map_err(|_| unreachable!("infallible"))
        .boxed_unsync()
}

impl Runtime {
    pub async fn handle_request(
        self: Arc<Self>,
        req: hyper::Request<hyper::body::Incoming>,
        access: Access,
    ) -> wasmtime::Result<hyper::Response<HyperOutgoingBody>> {
        let route = route(
            access,
            req.headers().get("host").map(|h| h.to_str()).transpose()?,
            &self.config.base_domain,
            &self.db.lock().unwrap(),
        );

        return match route {
            Route::Manager { env } => self.execute_manager(req, env).await,
            Route::Component { app, .. } => self.execute(req, app).await,
            Route::Admin => {
                let (parts, body) = req.into_parts();
                let body = body.collect().await?.to_bytes();
                let req = http::Request::from_parts(parts, body);

                let res = self.admin.handle(req).await;

                let (parts, body) = res.into_parts();
                Ok(hyper::Response::from_parts(parts, bytes_body(body)))
            }
            Route::WebHost => {
                todo!("implement web hosting");
            }
            Route::NotFound => Ok(empty(StatusCode::NOT_FOUND)),
            Route::Forbidden => Ok(empty(StatusCode::FORBIDDEN)),
        };
    }
}
