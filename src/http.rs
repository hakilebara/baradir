use crate::App;
use crate::slug::generate_slug;
use http_body_util::BodyExt;
use rusqlite::{Connection, params};
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
    pub apps: Arc<Mutex<HashMap<String, App>>>,
    pub conn: Arc<Mutex<Connection>>,
    pub engine: wasmtime::Engine,
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
        let env_slug = v[1];

        /*
         * If a request targets the naked domain of baradir
         * we create a new environment and redirect the user to it
         */
        if is_root {
            // Create new environment
            let domain_slug = generate_slug();
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;

            tx.execute(
                "INSERT INTO environment (slug) VALUES (?1)",
                params![domain_slug],
            )?;
            let env_id = tx.last_insert_rowid();

            let manager_id: i64 =
                tx.query_row("SELECT id FROM app WHERE name = 'manager'", [], |row| {
                    row.get(0)
                })?;

            let hello_id: i64 =
                tx.query_row("SELECT id FROM app WHERE name = 'hello'", [], |row| {
                    row.get(0)
                })?;

            tx.execute(
                "INSERT INTO environment_app (environment_id, app_id) VALUES (?1, ?2)",
                params![env_id, manager_id],
            )?;

            tx.execute(
                "INSERT INTO environment_app (environment_id, app_id) VALUES (?1, ?2)",
                params![env_id, hello_id],
            )?;

            let _ = tx.commit();

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

        // I have a env name and an app name from the HTTP host
        // I want to check that I can find this env/app relation in the environment_app table
        let exists: bool = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT * FROM environment_app ea
                JOIN environment e ON e.id = ea.environment_id
                JOIN app a ON a.id = ea.app_id
                WHERE e.slug = ?1 AND a.name = ?2",
                params![env_slug, app_name],
                |_| Ok(()),
            )
            .is_ok()
        };

        // TODO: return a 404
        if !exists {
            panic!("APP not found")
        }

        // Create per-http-request state within a `Store` and prepare the
        // initial resources  passed to the `handle` function.
        let pre = self
            .apps
            .lock()
            .unwrap()
            .get(app_name)
            .unwrap()
            .pre
            .clone()
            .unwrap();
        let mut store = Store::new(
            pre.engine(),
            MyClientState {
                table: ResourceTable::new(),
                wasi: WasiCtx::builder().inherit_stdio().build(),
                http: WasiHttpCtx::new(),
                conn: self.conn.clone(),
                apps: self.apps.clone(),
                engine: self.engine.clone(),
                domain: format!("{}.baradir.local:8080", env_slug),
                env_slug: env_slug.to_string(),
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
    conn: Arc<Mutex<Connection>>,
    apps: Arc<Mutex<HashMap<String, App>>>,
    engine: wasmtime::Engine,
    domain: String,
    env_slug: String,
}

impl crate::wit::bindings::ManagerAppImports for MyClientState {
    fn list_apps(&mut self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name FROM app").unwrap();
        stmt.query_map([], |row| Ok(row.get(0)?))
            .unwrap()
            .map(|app| app.unwrap())
            .collect()
    }

    fn get_domain(&mut self) -> String {
        self.domain.clone()
    }

    fn install_app_from_binary(&mut self, name: String, bytes: Vec<u8>) -> Result<String, ()> {
        let folder_filepath = format!("./workspaces/{}/{}/", self.env_slug, name);
        let wasm_filepath = format!("{}/{}.wasm", folder_filepath, name);

        std::fs::create_dir_all(folder_filepath).unwrap();

        std::fs::write(&wasm_filepath, &bytes).unwrap();

        let component = wasmtime::component::Component::from_binary(&self.engine, &bytes).unwrap();

        let mut linker: wasmtime::component::Linker<MyClientState> =
            wasmtime::component::Linker::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).expect("could not add to linker");
        wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)
            .expect("could not add to http linker");

        let pre = wasmtime_wasi_http::p2::bindings::ProxyPre::new(
            linker.instantiate_pre(&component).unwrap(),
        )
        .unwrap();

        // TODO: factor this out in a function
        {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();

            tx.execute(
                "INSERT INTO app (name, filepath) VALUES (?1, ?2)",
                params![name, wasm_filepath],
            )
            .unwrap();
            let app_id = tx.last_insert_rowid();

            let env_id: i64 = tx
                .query_row(
                    "SELECT id FROM environment WHERE slug = (?1)",
                    params![self.env_slug],
                    |row| row.get(0),
                )
                .unwrap();

            tx.execute(
                "INSERT INTO environment_app (environment_id, app_id) VALUES (?1, ?2)",
                params![env_id, app_id],
            )
            .unwrap();

            let _ = tx.commit();
        }

        self.apps.lock().unwrap().insert(
            name.clone(),
            App {
                name: name.clone(),
                filepath: wasm_filepath,
                pre: Some(pre),
            },
        );

        let url = format!("http://{}.{}", name, &self.domain);

        Ok(url)
    }
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
