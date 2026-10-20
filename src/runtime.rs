use crate::Config;
use crate::db::{AppRow, Db};
use crate::executor::InstanceState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmtime::component::Linker;
use wasmtime_wasi_http::p2::bindings::ProxyPre;

pub struct App {
    pub id: i64,
    pub name: String,
    pub filepath: String,
    pub pre: Option<ProxyPre<InstanceState>>,
}

impl From<AppRow> for App {
    fn from(row: AppRow) -> Self {
        App {
            id: row.id,
            name: row.name,
            filepath: row.filepath,
            pre: None,
        }
    }
}

pub struct NewApp {
    pub name: String,
    pub filepath: String,
}

pub struct Runtime {
    pub db: Arc<Mutex<Db>>,
    pub config: Config,
    pub apps: Arc<Mutex<HashMap<String, App>>>,
    pub engine: wasmtime::Engine, // TODO: remove and use pre.engine()
}

impl Runtime {
    pub fn build_linker(engine: &wasmtime::Engine) -> anyhow::Result<Linker<InstanceState>> {
        let mut linker: Linker<InstanceState> = Linker::new(engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;
        Ok(linker)
    }

    pub fn list_apps(&self) -> Result<Vec<String>, rusqlite::Error> {
        let db = self.db.lock().unwrap();
        db.get_app_names()
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
    ) -> anyhow::Result<String> {
        let folder_filepath = format!("{}/{}/{}/", self.config.env_folder, env_slug, name);
        let wasm_filepath = format!("{}/{}.wasm", folder_filepath, name);

        std::fs::create_dir_all(folder_filepath)?;
        std::fs::write(&wasm_filepath, &bytes)?;

        let component = wasmtime::component::Component::from_binary(&self.engine, &bytes)?;
        let linker = Runtime::build_linker(&self.engine)?;
        let pre = Some(wasmtime_wasi_http::p2::bindings::ProxyPre::new(
            linker.instantiate_pre(&component)?,
        )?);

        let mut db = self.db.lock().unwrap();
        let mut app = db.add_app_to_environment(
            NewApp {
                name: name.clone(),
                filepath: wasm_filepath,
            },
            &env_slug,
        )?;

        app.pre = pre;

        let url = format!(
            "http://{}.{}.{}:{}",
            &app.name, env_slug, &self.config.base_domain, &self.config.tcp_port
        );

        self.apps.lock().unwrap().insert(name, app);

        Ok(url)
    }
}
