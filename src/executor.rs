use crate::runtime::Runtime;
use std::sync::Arc;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::p2::bindings::http::types::Scheme;
use wasmtime_wasi_http::{
    WasiHttpCtx,
    p2::{WasiHttpCtxView, WasiHttpView, body::HyperOutgoingBody},
};

impl Runtime {
    pub async fn execute(
        self: Arc<Self>,
        req: hyper::Request<hyper::body::Incoming>,
        app: String,
        env: String,
    ) -> wasmtime::Result<hyper::Response<HyperOutgoingBody>> {
        // Create per-http-request state within a `Store` and prepare the
        // initial resources  passed to the `handle` function.
        let pre = self
            .app_store
            .lock()
            .unwrap()
            .get(&app)
            .unwrap()
            .pre
            .clone()
            .unwrap();

        if !crate::is_valid_segment(&env) || !crate::is_valid_segment(&app) {
            wasmtime::bail!("invalid app or environment segment");
        }

        // Each app instance gets its own writable directory, mounted at /data.
        let app_data_dir = self.config.environments_dir().join(&env).join(&app);
        std::fs::create_dir_all(&app_data_dir)?;

        let wasi = WasiCtx::builder()
            .inherit_stdio()
            .preopened_dir(&app_data_dir, "/data", DirPerms::all(), FilePerms::all())?
            .build();

        let mut store = wasmtime::Store::new(
            pre.engine(),
            InstanceState {
                table: ResourceTable::new(),
                wasi,
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
                        wasmtime::bail!("guest never invoked `response-outparam::set` method")
                    }
                    Ok(Err(e)) => e,
                    Err(e) => e.into(),
                };
                return Err(e.context("guest never invoked `response-outparam::set` method"));
            }
        }
    }
    pub async fn execute_manager(
        self: Arc<Self>,
        req: hyper::Request<hyper::body::Incoming>,
        env: String,
    ) -> wasmtime::Result<hyper::Response<HyperOutgoingBody>> {
        // Create per-http-request state within a `Store` and prepare the
        // initial resources  passed to the `handle` function.
        let pre = self.manager.pre.clone();

        let mut store = wasmtime::Store::new(
            pre.engine(),
            ManagerInstanceState {
                table: ResourceTable::new(),
                wasi: WasiCtx::builder().inherit_stdio().build(),
                http: WasiHttpCtx::new(),
                runtime: self.clone(),
                env_slug: env,
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
                        wasmtime::bail!("guest never invoked `response-outparam::set` method")
                    }
                    Ok(Err(e)) => e,
                    Err(e) => e.into(),
                };
                return Err(e.context("guest never invoked `response-outparam::set` method"));
            }
        }
    }
}

pub struct InstanceState {
    wasi: WasiCtx,
    http: WasiHttpCtx,
    table: ResourceTable,
}

impl WasiView for InstanceState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for InstanceState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: Default::default(),
        }
    }
}

pub struct ManagerInstanceState {
    wasi: WasiCtx,
    http: WasiHttpCtx,
    table: ResourceTable,
    runtime: Arc<Runtime>,
    env_slug: String,
}

impl crate::wit::bindings::ManagerAppImports for ManagerInstanceState {
    fn list_apps(&mut self) -> Vec<String> {
        self.runtime
            .list_apps()
            .expect("should return the list of app names")
    }

    fn get_domain(&mut self) -> String {
        self.runtime.get_domain(&self.env_slug)
    }

    fn install_app_from_binary(&mut self, name: String, bytes: Vec<u8>) -> Result<String, String> {
        self.runtime
            .install_app_from_binary(name, self.env_slug.clone(), bytes)
            .map_err(|e| e.to_string())
    }
}

impl WasiView for ManagerInstanceState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for ManagerInstanceState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: Default::default(),
        }
    }
}
