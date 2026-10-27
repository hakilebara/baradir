mod app;
#[allow(dead_code)] // unused parts of vendored topcoat-ui components
mod components;

mod bindings {
    wit_bindgen::generate!({
        path: "wit/world.wit",
    });
}

use http_body_util::BodyExt;
use std::sync::OnceLock;
use topcoat::router::Router;
use wstd::http::{Body, Error, Request, Response};

fn router() -> &'static Router {
    static ROUTER: OnceLock<Router> = OnceLock::new();
    ROUTER.get_or_init(app::router)
}

#[wstd::http_server]
async fn main(req: Request<Body>) -> Result<Response<Body>, Error> {
    let (parts, body) = req.into_parts();
    let req = http::Request::from_parts(parts, topcoat::router::Body::new(body.into_boxed_body()));

    let res = router().handle(req).await;

    let (parts, body) = res.into_parts();
    let body = Body::from_http_body(body.map_err(|e| anyhow::anyhow!(e)));
    Ok(Response::from_parts(parts, body))
}
