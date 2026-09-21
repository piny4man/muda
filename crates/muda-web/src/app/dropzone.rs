use leptos::prelude::*;

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
            aria-label="File dropzone. JPEG, PNG, WebP, and PDF."
        >
            <input
                node_ref=input_ref
                id="file-input"
                class="dropzone-input"
                type="file"
                multiple
                accept="image/jpeg,image/png,image/webp,application/pdf,.jpg,.jpeg,.png,.webp,.pdf"
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
                <strong>"Drop JPEG, PNG, WebP, or PDF files here, or click to browse"</strong>
                <span class="muted">"Multiple files · JPEG, PNG, WebP, and PDF"</span>
            </label>
        </section>
    }
}
