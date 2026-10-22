pub mod app;
mod components;

use bytes::Bytes;
use http_body_util::BodyExt;
use std::sync::Arc;
use topcoat::router::{Body, Router};

pub trait AdminStore: Send + Sync + 'static {
    fn list_environments(&self) -> Vec<String>;
    fn create_environment(&self) -> Result<String, String>;
}

pub struct Admin {
    router: Router,
}

impl Admin {
    pub fn new(store: impl AdminStore) -> Self {
        Self {
            router: app::router(Arc::new(store)),
        }
    }

    pub async fn handle(&self, req: http::Request<Bytes>) -> http::Response<Bytes> {
        let (parts, body) = req.into_parts();
        let req = http::Request::from_parts(parts, Body::from(body));

        let res = self.router.handle(req).await;

        let (parts, body) = res.into_parts();
        let body = body.collect().await.map(|c| c.to_bytes()).unwrap_or_default();

        http::Response::from_parts(parts, body)
    }
}
