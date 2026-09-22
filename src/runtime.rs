use crate::http::InstanceState;
use crate::App;
use crate::Config;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Runtime {
    pub config: Config,
    pub conn: Arc<Mutex<Connection>>,
    pub apps: Arc<Mutex<HashMap<String, App>>>,
    pub engine: wasmtime::Engine, // TODO: remove and use pre.engine()
}

impl Runtime {
    pub fn list_apps(&self) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name FROM app").unwrap();
        stmt.query_map([], |row| Ok(row.get(0)?))
            .unwrap()
            .map(|app| app.unwrap())
            .collect()
    }

    pub fn get_domain(&self, env_slug: &str) -> String {
        format!(
            "{}.{}:{}",
            env_slug, self.config.base_domain, self.config.tcp_port
        )
    }

    pub fn install_app_from_binary(
        &self,
        name: String,
        env_slug: String,
        bytes: Vec<u8>,
    ) -> Result<String, ()> {
        let folder_filepath = format!("./{}/{}/{}/", self.config.env_folder, env_slug, name);
        let wasm_filepath = format!("{}/{}.wasm", folder_filepath, name);

        std::fs::create_dir_all(folder_filepath).unwrap();

        std::fs::write(&wasm_filepath, &bytes).unwrap();

        let component = wasmtime::component::Component::from_binary(&self.engine, &bytes).unwrap();

        let mut linker: wasmtime::component::Linker<InstanceState> =
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
                    params![env_slug],
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

        let url = format!(
            "http://{}.{}.{}:{}",
            name, env_slug, self.config.base_domain, self.config.tcp_port
        );

        Ok(url)
    }
}
