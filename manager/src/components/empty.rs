use topcoat::{
    Result,
    view::{Attributes, Child, View, class, component, view},
};

/// How [`empty_media`] presents its content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum EmptyMediaVariant {
    /// Render the media as-is, such as an illustration or avatar.
    #[default]
    Default,
    /// Place an icon on a muted, rounded tile.
    Icon,
}

impl EmptyMediaVariant {
    fn classes(self) -> &'static str {
        match self {
            Self::Default => "bg-transparent",
            Self::Icon => {
                "flex size-10 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground [&>svg]:size-6"
            }
        }
    }
}

/// A placeholder shown in place of content that does not exist yet.
///
/// Pass an [`empty_header`] and, optionally, an [`empty_content`] with
/// actions as children. `attrs` adds attributes and classes to the `<div>`.
///
/// ```ignore
/// view! {
///     empty(
///         empty_header(
///             empty_media(variant: EmptyMediaVariant::Icon, icon(...))
///             empty_title("No projects yet")
///             empty_description("Create your first project to get started.")
///         )
///         empty_content(
///             button("Create project")
///         )
///     )
/// }
/// ```
#[component]
pub async fn empty(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div
            data-slot="empty"
            class=(class!(
                "flex min-w-0 flex-1 flex-col items-center justify-center gap-6 rounded-lg border border-dashed border-border p-6 text-center text-balance md:p-12",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </div>
    })
}

/// Groups the media, title and description of an [`empty`] state.
#[component]
pub async fn empty_header(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div
            data-slot="empty-header"
            class=(class!(
                "flex max-w-sm flex-col items-center gap-2 text-center",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </div>
    })
}

/// An icon or illustration above the title.
#[component]
pub async fn empty_media(
    #[default] variant: EmptyMediaVariant,
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div
            data-slot="empty-icon"
            class=(class!(
                "mb-2 flex shrink-0 items-center justify-center [&_svg]:pointer-events-none [&_svg]:shrink-0",
                variant.classes(),
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </div>
    })
}

/// The headline of an [`empty`] state.
#[component]
pub async fn empty_title(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div
            data-slot="empty-title"
            class=(class!("text-lg font-medium tracking-tight", attrs.remove("class")))
            (attrs)
        >
            (child)
        </div>
    })
}

/// Supporting text below the title.
#[component]
pub async fn empty_description(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div
            data-slot="empty-description"
            class=(class!(
                "text-sm/relaxed text-muted-foreground [&>a]:underline [&>a]:underline-offset-4 [&>a:hover]:text-primary",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </div>
    })
}

/// Actions or other content below the header, such as a button or form.
#[component]
pub async fn empty_content(
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    Ok(view! {
        <div
            data-slot="empty-content"
            class=(class!(
                "flex w-full max-w-sm min-w-0 flex-col items-center gap-4 text-sm text-balance",
                attrs.remove("class"),
            ))
            (attrs)
        >
            (child)
        </div>
    })
}
