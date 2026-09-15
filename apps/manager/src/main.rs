use axum::{Router, extract::Multipart, response::Redirect, routing::get, routing::post};
use maud::{Markup, html};

mod bindings {
    wit_bindgen::generate!({
        path: "wit/world.wit",
    });
}

use bindings::{get_domain, install_app_from_binary, list_apps};

#[wstd_axum::http_server]
fn main() -> Router {
    Router::new().route("/", get(home)).route("/", post(upload))
}

async fn home() -> Markup {
    let apps: Vec<String> = list_apps();
    let domain: String = get_domain();
    html! {
        h1 { "Manager App" }
        ul {
            @for name in &apps {
                li {
                    a href={ "http://" (name) "." (domain) } {(name)}
                }
            }
        }
        form method="post" enctype="multipart/form-data" {
            input type="text" name="name" placeholder="App name" required;
            input type="file" name="file" accept=".wasm";
            input type="submit" value="Upload";
        }
    }
}

async fn upload(mut multipart: Multipart) -> Redirect {
    let mut name = String::new();
    let mut bytes = Vec::new();
    while let Some(field) = multipart.next_field().await.unwrap() {
        match field.name() {
            Some("name") => name = field.text().await.unwrap(),
            Some("file") => bytes = field.bytes().await.unwrap().to_vec(),
            _ => (),
        }
    }
    let _url = install_app_from_binary(&name, &bytes).unwrap();
    Redirect::to("/")
}
