use leptos::prelude::*;

use super::model::MAX_FILE_BYTES;

#[component]
pub(crate) fn DropZone(
    dragging: RwSignal<bool>,
    input_ref: NodeRef<leptos::html::Input>,
    on_change: impl Fn(web_sys::Event) + 'static + Clone,
    on_drop: impl Fn(web_sys::DragEvent) + 'static + Clone,
) -> impl IntoView {
    view! {
        <section
            class=move || {
                if dragging.get() { "dropzone dragging" } else { "dropzone" }
            }
            aria-label="File dropzone. JPEG and PNG only."
        >
            <input
                node_ref=input_ref
                id="file-input"
                class="dropzone-input"
                type="file"
                multiple
                accept="image/jpeg,image/png,.jpg,.jpeg,.png"
                on:change=on_change
                on:dragover=move |ev| {
                    ev.prevent_default();
                    dragging.set(true);
                }
                on:dragleave=move |ev| {
                    ev.prevent_default();
                    dragging.set(false);
                }
                on:drop=on_drop
            />
            <label class="dropzone-label" for="file-input">
                <strong>"Drop JPEG or PNG files here, or click to browse"</strong>
                <span class="muted">
                    {format!(
                        "Multiple files · JPEG and PNG · max {} MB each",
                        MAX_FILE_BYTES / (1024 * 1024)
                    )}
                </span>
            </label>
        </section>
    }
}
