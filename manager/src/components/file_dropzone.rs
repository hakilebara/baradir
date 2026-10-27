use topcoat::{
    Result,
    icon::{icon, iconify::iconify_icon},
    view::{Attributes, View, class, component, view},
};

/// A drop target for one file, submitted with its form like a plain file input.
///
/// The transparent input covers the whole zone, so the browser accepts dropped
/// files and opens the file picker on click without any script. The input is
/// required, which lets the zone switch to its selected state through `:valid`.
///
/// `attrs` adds attributes and classes to the `<label>`.
///
/// ```ignore
/// view! {
///     file_dropzone(name: "file", accept: Some(".wasm"))
/// }
/// ```
#[component]
pub async fn file_dropzone(
    name: &str,
    #[default] accept: Option<&str>,
    #[default] mut attrs: Attributes,
) -> Result<impl View> {
    Ok(view! {
        <label
            data-slot="file-dropzone"
            class=(class!(
                "group/dropzone relative flex w-full cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border p-6 text-center text-sm text-muted-foreground transition-colors hover:bg-muted/50 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[input:valid]:border-solid has-[input:valid]:border-primary has-[input:valid]:text-foreground [&>svg]:size-6",
                attrs.remove("class"),
            ))
            (attrs)
        >
            icon(data: iconify_icon!("lucide:upload"))
            <span class="group-has-[input:valid]/dropzone:hidden">"Drop a file here or click to browse"</span>
            <span class="hidden group-has-[input:valid]/dropzone:inline">"File selected"</span>
            <input
                type="file"
                name=(name)
                accept=(accept)
                required=""
                class="absolute inset-0 cursor-pointer opacity-0"
            />
        </label>
    })
}
