use crate::App;
use crate::slug::generate_slug;
use http_body_util::BodyExt;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmtime::bail;
use wasmtime::component::ResourceTable;
use wasmtime::{Result, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::p2::bindings::http::types::Scheme;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;
use wasmtime_wasi_http::{
    WasiHttpCtx,
    p2::{WasiHttpCtxView, WasiHttpView},
};

pub struct MyServer {
    pub apps: HashMap<String, App>,
    pub conn: Arc<Mutex<Connection>>,
}

impl MyServer {
    pub async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<HyperOutgoingBody>> {
        let raw_host = req.headers().get("host").unwrap().to_str().unwrap();
        let v: Vec<_> = raw_host.split('.').take(2).collect();

        let host = raw_host.split(":").next().unwrap();

        // TODO: refactor hard coded domain
        let base = "baradir.local";
        let is_root = host == base;

        let app_name = v[0];
        let _env = v[1];

        /*
         * If a request targets the naked domain of baradir
         * we create a new environment and redirect the user to it
         */
        if is_root {
            // Create new environment
            let domain_slug = generate_slug();
            self.conn.lock().unwrap().execute(
                "INSERT INTO environment (slug) VALUES (?1)",
                ((&domain_slug),),
            )?;
            // let row_id = self.conn.lock().unwrap().last_insert_rowid();

            // TODO: add necessary entries in the environment_apps join table
            // for a 'anagerm app and a 'hello' app

            // redirect to the subdomain of the newly created environment
            println!("redirect to mgr.{domain_slug}.baradir.local");
            return Ok(hyper::Response::builder()
                // TODO: replace "location" and the return code with
                // the constants provided by the hyper library
                .header(
                    "Location",
                    format!("http://manager.{domain_slug}.baradir.local:8080"),
                )
                .status(302)
                .body(
                    http_body_util::Empty::<bytes::Bytes>::new()
                        .map_err(|_| unreachable!("infaillable"))
                        .boxed_unsync(),
                )?);
        }

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

pub struct MyClientState {
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
