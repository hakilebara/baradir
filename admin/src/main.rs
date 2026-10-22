use std::sync::{Arc, Mutex};

struct FakeStore {
    environments: Mutex<Vec<String>>,
}

impl FakeStore {
    fn new() -> Self {
        Self {
            environments: Mutex::new(vec!["dev-env-1".to_string(), "dev-env-2".to_string()]),
        }
    }
}

impl baradir_admin::AdminStore for FakeStore {
    fn list_environments(&self) -> Vec<String> {
        self.environments.lock().unwrap().clone()
    }

    fn create_environment(&self) -> Result<String, String> {
        let mut environments = self.environments.lock().unwrap();
        let slug = format!("dev-env-{}", environments.len() + 1);
        environments.push(slug.clone());
        Ok(slug)
    }
}

#[tokio::main]
async fn main() {
    topcoat::start(baradir_admin::app::router(Arc::new(FakeStore::new())))
        .await
        .unwrap();
}
