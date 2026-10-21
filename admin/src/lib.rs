pub mod app;
mod components;

use bytes::Bytes;
use http_body_util::BodyExt;
use std::sync::LazyLock;
use topcoat::router::{Body, Router};

static ROUTER: LazyLock<Router> = LazyLock::new(app::router);

pub trait AdminStore {
    fn list_environments(&self) -> Vec<String>;
}

pub async fn handle(req: http::Request<Bytes>) -> http::Response<Bytes> {
    let (parts, body) = req.into_parts();
    let req = http::Request::from_parts(parts, Body::from(body));

    let res = ROUTER.handle(req).await;

    let (parts, body) = res.into_parts();
    let body = body.collect().await.map(|c| c.to_bytes()).unwrap_or_default();

    http::Response::from_parts(parts, body)
}
