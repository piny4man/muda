use leptos::prelude::*;

#[component]
pub(crate) fn Toolbar(
    strip_all: impl Fn(web_sys::MouseEvent) + 'static + Clone,
    download_all: impl Fn(web_sys::MouseEvent) + 'static + Clone,
    clear: impl Fn(web_sys::MouseEvent) + 'static + Clone,
) -> impl IntoView {
    let strip_sticky = strip_all.clone();
    view! {
        <div class="toolbar">
            <button type="button" class="primary toolbar-strip" on:click=strip_all>
                "Strip all"
            </button>
            <button type="button" on:click=download_all>
                "Download all (ZIP)"
            </button>
            <button type="button" class="ghost" on:click=clear>
                "Clear"
            </button>
        </div>
        <div class="sticky-strip">
            <button type="button" class="primary" on:click=strip_sticky>
                "Strip all"
            </button>
        </div>
    }
}
