use wstd::http::{Body, Request, Response, Result, StatusCode};

#[wstd::http_server]
async fn main(req: Request<Body>) -> Result<Response<Body>> {
    match req.uri().path() {
        "/" => home(req).await,
        _ => not_found(req).await,
    }
}

async fn home(_req: Request<Body>) -> Result<Response<Body>> {
    // std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(Response::new("Hello, world!\n".into()))
}

async fn not_found(_req: Request<Body>) -> Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(().into())
        .expect("builder must succeed"))
}
