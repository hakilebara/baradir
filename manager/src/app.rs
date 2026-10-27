use crate::bindings::{get_domain, install_app_from_binary, list_apps};
use crate::components::{
    button::button,
    card::{card, card_content, card_description, card_header, card_title},
    empty::{
        EmptyMediaVariant, empty, empty_content, empty_description, empty_header, empty_media,
        empty_title,
    },
    file_dropzone::file_dropzone,
    input::input,
    sidebar::{
        SidebarCollapsible, sidebar, sidebar_content, sidebar_group, sidebar_group_content,
        sidebar_group_label, sidebar_header, sidebar_inset, sidebar_menu, sidebar_menu_button,
        sidebar_menu_item, sidebar_provider,
    },
};
use topcoat::Result;
use topcoat::icon::{icon, iconify::iconify_icon};
use topcoat::router::content::multipart::Multipart;
use topcoat::router::error::see_other;
use topcoat::router::{BodyLimit, Router, RouterBuilderDiscoverExt, Slot, layout, module_router, page};
use topcoat::view::{Unescaped, View, attributes, component, view};

/// Installed components are uploaded whole, well above the 2 MiB default.
const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

pub fn router() -> Router {
    module_router!()
        .discover()
        .layer(BodyLimit::max(MAX_UPLOAD_BYTES))
        .build()
}

#[layout]
async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>"Baradir"</title>
                // Inlined because the component cannot serve asset files. Browsers do not
                // decode entities in <style>, so the trusted build output must not be escaped.
                <style>(Unescaped::new_unchecked(include_str!(concat!(env!("OUT_DIR"), "/tailwind.css"))))</style>
            </head>
            <body>
                (slot)
            </body>
        </html>
    })
}

fn app_url(name: &str, domain: &str) -> String {
    format!("http://{name}.{domain}")
}

#[component]
async fn app_sidebar(apps: &[String], domain: &str) -> Result<impl View> {
    Ok(view! {
        sidebar(
            collapsible: SidebarCollapsible::None,
            sidebar_header(
                <a href="/" class="flex h-8 items-center gap-2 px-2 font-semibold">
                    icon(data: iconify_icon!("lucide:box"))
                    <span>"Baradir"</span>
                </a>
            )
            sidebar_content(
                sidebar_group(
                    sidebar_group_label("Applications")
                    sidebar_group_content(
                        sidebar_menu(
                            for name in apps {
                                let url = app_url(name, domain);
                                sidebar_menu_item(
                                    sidebar_menu_button(
                                        href: Some(url.as_str()),
                                        icon(data: iconify_icon!("lucide:app-window"))
                                        <span>(name)</span>
                                    )
                                )
                            }
                        )
                    )
                )
            )
        )
    })
}

#[component]
async fn upload_form(error: Option<&str>) -> Result<impl View> {
    Ok(view! {
        <form method="post" action="/" enctype="multipart/form-data" class="flex w-full flex-col gap-3">
            if let Some(error) = error {
                <p class="text-sm text-destructive">(error)</p>
            }
            input(attrs: attributes! { type="text" name="name" placeholder="App name" required="" })
            file_dropzone(name: "file", accept: Some(".wasm"))
            button(attrs: attributes! { type="submit" }, "Upload")
        </form>
    })
}

#[component]
async fn dashboard(error: Option<&str>) -> Result<impl View> {
    let apps = list_apps();
    let domain = get_domain();

    Ok(view! {
        sidebar_provider(
            app_sidebar(apps: &apps, domain: &domain)
            sidebar_inset(
                sidebar_header(
                    <h1 class="text-sm font-medium">"Installed applications"</h1>
                )
                <div class="flex flex-1 flex-col gap-6 p-6">
                    if apps.is_empty() {
                        empty(
                            empty_header(
                                empty_media(
                                    variant: EmptyMediaVariant::Icon,
                                    icon(data: iconify_icon!("lucide:package-open"))
                                )
                                empty_title("No applications yet")
                                empty_description(
                                    "Upload a WebAssembly component to install your first application."
                                )
                            )
                            empty_content(
                                upload_form(error: error)
                            )
                        )
                    } else {
                        <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                            for name in &apps {
                                <a href=(app_url(name, &domain)) class="rounded-xl transition hover:opacity-80">
                                    card(
                                        card_header(
                                            card_title((name))
                                            card_description((app_url(name, &domain)))
                                        )
                                    )
                                </a>
                            }
                        </div>
                        card(
                            attrs: attributes! { class="max-w-md" },
                            card_header(
                                card_title("Upload a new application")
                                card_description("Install a WebAssembly component from a .wasm file.")
                            )
                            card_content(
                                upload_form(error: error)
                            )
                        )
                    }
                </div>
            )
        )
    })
}

#[page]
async fn home() -> Result<impl View> {
    Ok(view! { dashboard(error: None) })
}

#[page(POST)]
async fn upload(mut multipart: Multipart) -> Result<impl View> {
    let mut name = String::new();
    let mut bytes = Vec::new();
    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("name") => name = field.text().await?,
            Some("file") => bytes = field.bytes().await?.to_vec(),
            _ => (),
        }
    }

    match install_app_from_binary(&name, &bytes) {
        Ok(_url) => Err(see_other("/").into()),
        Err(message) => Ok(view! { dashboard(error: Some(message.as_str())) }),
    }
}
