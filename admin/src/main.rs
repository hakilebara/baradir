#[tokio::main]
async fn main() {
    topcoat::start(baradir_admin::app::router())
        .await
        .unwrap();
}
