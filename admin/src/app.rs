mod environments;

use crate::AdminStore;
use crate::components::{
    button::{ButtonSize, ButtonVariant, button_variants},
    card::{card, card_description, card_header, card_title},
};
use std::sync::Arc;
use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::{Router, RouterBuilderDiscoverExt, Slot, layout, module_router, page};
use topcoat::view::{View, view};

pub fn router(store: Arc<dyn AdminStore>) -> Router {
    module_router!().discover().app_context(store).build()
}

#[layout]
async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>"Baradir admin"</title>
                <style>(include_str!(concat!(env!("OUT_DIR"), "/tailwind.css")))</style>
            </head>
            <body class="bg-background text-foreground">
                (slot)
            </body>
        </html>
    })
}

#[page]
async fn home(cx: &Cx) -> Result<impl View> {
    let store: &Arc<dyn AdminStore> = app_context(cx);
    Ok(view! {
        <main class="mx-auto flex min-h-screen max-w-3xl flex-col justify-center gap-6 p-8">
            card(
                card_header(
                    card_title("Baradir admin")
                    card_description("Environments and apps running on this host.")
                )
                <ul class="flex flex-col gap-1 px-6 text-sm">
                    for env in store.list_environments() {
                        let url = format!("http://manager.{}.baradir.localhost:9090", env);
                        <li><a href=(url)>(env)</a></li>
                    }
                </ul>
                <div class="px-6">
                    <a
                        href="/environments/new"
                        class=(button_variants(ButtonVariant::Primary, ButtonSize::Md))
                    >
                        "Create environment"
                    </a>
                </div>
            )
        </main>
    })
}
