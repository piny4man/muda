use leptos::prelude::*;

#[component]
pub(crate) fn SelectiveStub() -> impl IntoView {
    view! {
        <fieldset class="selective" disabled>
            <legend>"Keep selected tags (coming soon)"</legend>
            <label><input type="checkbox" disabled/> " GPS"</label>
            <label><input type="checkbox" disabled/> " Camera"</label>
            <label><input type="checkbox" disabled/> " Software"</label>
            <label><input type="checkbox" disabled/> " Comments"</label>
        </fieldset>
    }
}

#[component]
pub(crate) fn PrivacyFooter() -> impl IntoView {
    view! {
        <footer class="foot">
            <p>
                <strong>"Formats: "</strong>
                "JPEG and PNG in v1. HEIC, RAW, WebP, TIFF, PDF, video, and audio are not supported."
            </p>
            <p>
                <strong>"Color profile kept. "</strong>
                "ICC / sRGB (and JPEG Adobe APP14) stay in the file so colors do not shift. GPS, camera, software, comments, XMP, and thumbnails are removed."
            </p>
            <p class="muted">
                "Lossless container rewrite via img-parts. Image scans are not re-encoded."
            </p>
        </footer>
    }
}
