use leptos::prelude::*;

use super::model::KeepSelection;

#[component]
pub(crate) fn KeepTags(keep: RwSignal<KeepSelection>) -> impl IntoView {
    view! {
        <fieldset class="selective">
            <legend>"Keep selected tags"</legend>
            <label>
                <input
                    type="checkbox"
                    prop:checked=move || keep.get().gps
                    on:change=move |ev| keep.update(|k| k.gps = event_target_checked(&ev))
                />
                " GPS"
            </label>
            <label>
                <input
                    type="checkbox"
                    prop:checked=move || keep.get().camera
                    on:change=move |ev| keep.update(|k| k.camera = event_target_checked(&ev))
                />
                " Camera"
            </label>
            <label>
                <input
                    type="checkbox"
                    prop:checked=move || keep.get().software
                    on:change=move |ev| keep.update(|k| k.software = event_target_checked(&ev))
                />
                " Software"
            </label>
            <label>
                <input
                    type="checkbox"
                    prop:checked=move || keep.get().comment
                    on:change=move |ev| keep.update(|k| k.comment = event_target_checked(&ev))
                />
                " Comments"
            </label>
        </fieldset>
    }
}

#[component]
pub(crate) fn PrivacyFooter() -> impl IntoView {
    view! {
        <footer class="foot">
            <p>
                <strong>"Formats: "</strong>
                "JPEG, PNG, WebP, and PDF in v1. HEIC, RAW, TIFF, video, and audio are not supported."
            </p>
            <p>
                <strong>"Color profile kept. "</strong>
                "ICC / sRGB (and JPEG Adobe APP14) stay in the file so colors do not shift. XMP and thumbnails are always removed. GPS, camera, software, and comments are removed unless you keep them. PDF text stays. Document info, the trailer ID, and tags inside embedded JPEGs are removed. Encrypted PDFs are rejected."
            </p>
            <p class="muted">
                "Lossless container rewrite via img-parts. Image scans are not re-encoded. PDFs are rebuilt in one generation so older revisions are dropped."
            </p>
        </footer>
    }
}

#[component]
pub(crate) fn SiteFooter() -> impl IntoView {
    view! {
        <footer class="site-footer">
            <a href="https://github.com/piny4man/muda" target="_blank" rel="noopener noreferrer">
                "muda on GitHub"
            </a>
            <span class="sep" aria-hidden="true">"·"</span>
            <a href="https://pinya.dev" target="_blank" rel="noopener noreferrer">
                "by Pinya.dev"
            </a>
        </footer>
    }
}
