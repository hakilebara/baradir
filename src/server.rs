use crate::db::Db;
use crate::executor::InstanceState;
use crate::runtime::{App, Runtime};
use crate::{Access, Config};
use hyper::server::conn::http1;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use wasmtime::Engine;
use wasmtime::component::{Component, HasSelf};
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::ProxyPre;

pub async fn run(config: Config, db: Db) -> anyhow::Result<()> {
    let engine = Engine::default();

    let app_store = Arc::new(Mutex::new(HashMap::new()));

    let app_rows = db.get_apps()?;

    for row in app_rows {
        let mut app: App = row.into();

        // Compile the component on the command line to machine code
        let component = Component::from_file(&engine, app.filepath.clone())?;

        // Prepare the `ProxyPre` which is a pre-instantiated version of the
        // component. This will make per-request instantiation
        // much quicker.
        let mut linker = Runtime::build_linker(&engine)?;

        crate::wit::bindings::ManagerApp::add_to_linker::<InstanceState, HasSelf<InstanceState>>(
            &mut linker,
            |state| state,
        )?;

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
    let runtime = Arc::new(Runtime {
        db: Arc::new(Mutex::new(db)),
        apps: app_store,
        config: config,
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
