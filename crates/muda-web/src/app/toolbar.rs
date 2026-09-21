use leptos::prelude::*;

/// Toolbar actions stay inert until at least one file is queued.
pub(crate) fn toolbar_disabled(queue_len: usize) -> bool {
    queue_len == 0
}

#[component]
pub(crate) fn Toolbar(
    strip_all: impl Fn(web_sys::MouseEvent) + 'static + Clone,
    download_all: impl Fn(web_sys::MouseEvent) + 'static + Clone,
    clear: impl Fn(web_sys::MouseEvent) + 'static + Clone,
    disabled: Signal<bool>,
) -> impl IntoView {
    let strip_sticky = strip_all.clone();
    view! {
        <div class="toolbar">
            <button
                type="button"
                class="primary toolbar-strip"
                on:click=strip_all
                disabled=move || disabled.get()
            >
                "Strip all"
            </button>
            <button
                type="button"
                on:click=download_all
                disabled=move || disabled.get()
            >
                "Download all (ZIP)"
            </button>
            <button
                type="button"
                class="ghost"
                on:click=clear
                disabled=move || disabled.get()
            >
                "Clear"
            </button>
        </div>
        <div class="sticky-strip">
            <button
                type="button"
                class="primary"
                on:click=strip_sticky
                disabled=move || disabled.get()
            >
                "Strip all"
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_disabled_without_files() {
        assert!(toolbar_disabled(0));
    }

    #[test]
    fn toolbar_enabled_with_files() {
        assert!(!toolbar_disabled(1));
        assert!(!toolbar_disabled(5));
    }
}
