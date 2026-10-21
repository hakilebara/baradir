use crate::components::{
    button::{ButtonVariant, button},
    card::{card, card_description, card_footer, card_header, card_title},
};
use topcoat::Result;
use topcoat::router::{Router, RouterBuilderDiscoverExt, Slot, layout, module_router, page};
use topcoat::view::{View, view};

pub fn router() -> Router {
    module_router!().discover().build()
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
async fn home() -> Result<impl View> {
    Ok(view! {
        <main class="mx-auto flex min-h-screen max-w-3xl flex-col justify-center gap-6 p-8">
            card(
                card_header(
                    card_title("Baradir admin")
                    card_description("Environments and apps running on this host.")
                )
                card_footer(
                    button(variant: ButtonVariant::Primary, "Create environment")
                )
            )
        </main>
    })
}
