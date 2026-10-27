use crate::db::{AppRow, Db};
use crate::executor::{InstanceState, ManagerInstanceState};
use crate::registry::Registry;
use crate::Config;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmtime::component::Linker;
use wasmtime_wasi_http::p2::bindings::ProxyPre;

pub struct App {
    pub name: String,
    pub digest: String,
    pub pre: Option<ProxyPre<InstanceState>>,
}

pub struct ManagerApp {
    pub pre: ProxyPre<ManagerInstanceState>,
}

impl From<AppRow> for App {
    fn from(row: AppRow) -> Self {
        App {
            name: row.name,
            digest: row.digest,
            pre: None,
        }
    }
}

pub struct NewApp {
    pub name: String,
    pub digest: String,
}

pub struct Runtime {
    pub db: Arc<Mutex<Db>>,
    pub admin: baradir_admin::Admin,
    pub config: Config,
    pub manager: ManagerApp,
    pub app_store: Arc<Mutex<HashMap<String, App>>>,
    pub engine: wasmtime::Engine, // TODO: remove and use pre.engine()
    pub registry: Registry,
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
        if name == crate::MANAGER {
            anyhow::bail!("`{name}` is a reserved app name");
        }

        // disk write
        let digest = self.registry.put(&bytes)?;

        // Compile component
        let component = wasmtime::component::Component::from_binary(&self.engine, &bytes)?;
        let linker = Runtime::build_linker(&self.engine)?;
        let pre =
            wasmtime_wasi_http::p2::bindings::ProxyPre::new(linker.instantiate_pre(&component)?)?;

        let mut db = self.db.lock().unwrap();
        let mut app: App = db
            .add_app_to_environment(
                NewApp {
                    name: name.clone(),
                    digest,
                },
                &env_slug,
            )?
            .into();
        app.pre = Some(pre);

        let url = format!(
            "http://{}.{}.{}:{}",
            &app.name, env_slug, &self.config.base_domain, &self.config.tcp_port
        );

        self.app_store.lock().unwrap().insert(name, app);
        Ok(url)
    }
}

#[derive(Clone)]
pub struct DbStore {
    db: Arc<Mutex<Db>>,
}

impl DbStore {
    pub fn new(db: Arc<Mutex<Db>>) -> Self {
        Self { db }
    }
}

impl baradir_admin::AdminStore for DbStore {
    fn list_environments(&self) -> Vec<String> {
        self.db.lock().unwrap().get_env_slugs().unwrap()
    }

    fn create_environment(&self) -> Result<String, String> {
        let slug = crate::slug::generate_slug();
        let mut db = self.db.lock().unwrap();
        db.create_environment(&slug).map_err(|e| e.to_string())?;
        Ok(slug)
    }
}
