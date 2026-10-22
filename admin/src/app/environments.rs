use crate::AdminStore;
use crate::components::button::button;
use crate::components::card::{card, card_description, card_header, card_title};
use std::sync::Arc;
use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::see_other;
use topcoat::router::page;
use topcoat::view::{View, attributes, component, view};

const MAIN: &str = "mx-auto flex min-h-screen max-w-3xl flex-col justify-center gap-6 p-8";

#[component]
async fn create_card(error: Option<&str>) -> Result<impl View> {
    Ok(view! {
        card(
            card_header(
                card_title("Create environment")
                card_description("A new environment gets a generated name.")
            )
            if let Some(error) = error {
                <p class="px-6 text-sm text-destructive">(error)</p>
            }
            <form method="post" action="/environments/new" class="px-6">
                button(
                    attrs: attributes! { type="submit" class="w-full" },
                    "Create environment"
                )
            </form>
        )
    })
}

#[page("./new")]
async fn new_environment() -> Result<impl View> {
    Ok(view! {
        <main class=(MAIN)>
            create_card(error: None)
        </main>
    })
}

#[page(POST "./new")]
async fn create_environment(cx: &Cx) -> Result<impl View> {
    let store: &Arc<dyn AdminStore> = app_context(cx);
    match store.create_environment() {
        Ok(_) => Err(see_other("/").into()),
        Err(message) => Ok(view! {
            <main class=(MAIN)>
                create_card(error: Some(message.as_str()))
            </main>
        }),
    }
}
