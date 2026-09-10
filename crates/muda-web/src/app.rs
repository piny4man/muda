use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};
use muda_core::{sniff_kind, strip_image, ImageKind, StripReport};
use wasm_bindgen::JsCast;

use crate::zip_store::zip_store;

/// Per-file cap shown in the UI and enforced before stripping.
pub const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const STRIP_CONCURRENCY: usize = 2;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/muda-web.css"/>
        <Title text="muda — metadata eraser"/>
        <Router>
            <main>
                <Routes fallback=|| view! { <p class="muted">"Page not found."</p> }.into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[server]
pub async fn health() -> Result<String, ServerFnError> {
    Ok("ok".into())
}

/// Stub for a later optional upload path for formats the browser cannot strip.
/// v1 does not accept image bytes.
#[server]
pub async fn upload_unsupported(_filename: String) -> Result<(), ServerFnError> {
    Err(ServerFnError::ServerError(
        "501 Not Implemented: optional upload for unsupported formats is not available in v1"
            .into(),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Queued,
    Stripping,
    Done,
    Error,
    Unsupported,
}

#[derive(Clone)]
struct FileItem {
    id: u64,
    name: String,
    size: usize,
    kind: Option<ImageKind>,
    preview_url: String,
    original: Arc<[u8]>,
    status: RwSignal<Status>,
    report: RwSignal<Option<StripReport>>,
    output: RwSignal<Option<Vec<u8>>>,
    error: RwSignal<Option<String>>,
}

#[component]
fn HomePage() -> impl IntoView {
    let items = RwSignal::new(Vec::<FileItem>::new());
    let next_id = RwSignal::new(1u64);
    let dragging = RwSignal::new(false);
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();
    let banner = RwSignal::new(Option::<String>::None);

    let add_files = move |files: Vec<web_sys::File>| {
        leptos::task::spawn_local(async move {
            for file in files {
                let name = file.name();
                let size = file.size() as usize;
                let preview_url = match web_sys::Url::create_object_url_with_blob(&file) {
                    Ok(url) => url,
                    Err(_) => String::new(),
                };
                let bytes = match read_file_bytes(file).await {
                    Ok(b) => b,
                    Err(e) => {
                        banner.set(Some(format!("Could not read {name}: {e}")));
                        if !preview_url.is_empty() {
                            let _ = web_sys::Url::revoke_object_url(&preview_url);
                        }
                        continue;
                    }
                };
                let kind = sniff_kind(&bytes);
                let id = next_id.get_untracked();
                next_id.set(id + 1);

                let (status, error) = if kind.is_none() {
                    (
                        Status::Unsupported,
                        Some("Not a JPEG or PNG. This format is not supported in v1.".to_string()),
                    )
                } else if size > MAX_FILE_BYTES {
                    (
                        Status::Error,
                        Some(format!(
                            "File is larger than {} MB.",
                            MAX_FILE_BYTES / (1024 * 1024)
                        )),
                    )
                } else {
                    (Status::Queued, None)
                };

                let item = FileItem {
                    id,
                    name,
                    size,
                    kind,
                    preview_url,
                    original: Arc::from(bytes),
                    status: RwSignal::new(status),
                    report: RwSignal::new(None),
                    output: RwSignal::new(None),
                    error: RwSignal::new(error),
                };
                items.update(|list| list.push(item));
            }
        });
    };

    let on_input_change = move |_| {
        let Some(input) = input_ref.get() else { return };
        let Some(list) = input.files() else { return };
        let files = collect_file_list(&list);
        input.set_value("");
        add_files(files);
    };

    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        dragging.set(false);
        let Some(dt) = ev.data_transfer() else { return };
        let Some(list) = dt.files() else { return };
        add_files(collect_file_list(&list));
    };

    let strip_all = move |_| {
        let ready: Vec<FileItem> = items
            .get()
            .into_iter()
            .filter(|item| {
                item.kind.is_some()
                    && matches!(item.status.get(), Status::Queued | Status::Error)
                    && item.size <= MAX_FILE_BYTES
            })
            .collect();
        run_strip_queue(ready);
    };

    let download_all = move |_| {
        let entries: Vec<(String, Vec<u8>)> = items
            .get()
            .into_iter()
            .filter_map(|item| {
                let bytes = item.output.get()?;
                let name = item
                    .report
                    .get()
                    .map(|r| r.output_name)
                    .unwrap_or_else(|| format!("{}.cleaned", item.name));
                Some((name, bytes))
            })
            .collect();
        if entries.is_empty() {
            banner.set(Some("Strip files before downloading a ZIP.".into()));
            return;
        }
        let borrowed: Vec<(String, &[u8])> = entries
            .iter()
            .map(|(n, b)| (n.clone(), b.as_slice()))
            .collect();
        let zip = zip_store(&borrowed);
        if let Err(e) = download_bytes("muda-cleaned.zip", "application/zip", &zip) {
            banner.set(Some(e));
        }
    };

    let clear = move |_| {
        items.update(|list| {
            for item in list.drain(..) {
                if !item.preview_url.is_empty() {
                    let _ = web_sys::Url::revoke_object_url(&item.preview_url);
                }
            }
        });
    };

    view! {
        <div class="page">
            <header class="hero">
                <p class="brand">"muda"</p>
                <h1>"Metadata eraser"</h1>
                <p class="claim">
                    "JPEG and PNG are processed in your browser. Files are not uploaded."
                </p>
            </header>

            <section
                class=move || {
                    if dragging.get() { "dropzone dragging" } else { "dropzone" }
                }
                aria-label="File dropzone. JPEG and PNG only."
            >
                <input
                    node_ref=input_ref
                    id="file-input"
                    class="sr-only"
                    type="file"
                    multiple
                    accept="image/jpeg,image/png,.jpg,.jpeg,.png"
                    on:change=on_input_change
                />
                <label
                    class="dropzone-label"
                    for="file-input"
                    on:dragover=move |ev| {
                        ev.prevent_default();
                        dragging.set(true);
                    }
                    on:dragleave=move |ev| {
                        ev.prevent_default();
                        dragging.set(false);
                    }
                    on:drop=on_drop
                >
                    <strong>"Drop JPEG or PNG files here, or click to browse"</strong>
                    <span class="muted">
                        {format!(
                            "Multiple files · JPEG and PNG · max {} MB each",
                            MAX_FILE_BYTES / (1024 * 1024)
                        )}
                    </span>
                </label>
            </section>

            <Show when=move || banner.get().is_some()>
                <p class="banner" role="status">{move || banner.get().unwrap_or_default()}</p>
            </Show>

            <div class="toolbar">
                <button type="button" class="primary" on:click=strip_all>
                    "Strip all"
                </button>
                <button type="button" on:click=download_all>
                    "Download all (ZIP)"
                </button>
                <button type="button" class="ghost" on:click=clear>
                    "Clear"
                </button>
            </div>

            <fieldset class="selective" disabled>
                <legend>"Keep selected tags (coming soon)"</legend>
                <label><input type="checkbox" disabled/> " GPS"</label>
                <label><input type="checkbox" disabled/> " Camera"</label>
                <label><input type="checkbox" disabled/> " Software"</label>
                <label><input type="checkbox" disabled/> " Comments"</label>
            </fieldset>

            <Show
                when=move || !items.get().is_empty()
                fallback=|| view! {
                    <p class="empty">
                        "No files yet. Add JPEG or PNG images to strip metadata in your browser."
                    </p>
                }
            >
                <ul class="files" role="list">
                    <For
                        each=move || items.get()
                        key=|item| item.id
                        children=move |item| {
                            view! { <FileRow item=item items=items /> }
                        }
                    />
                </ul>
            </Show>

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
        </div>
    }
}

#[component]
fn FileRow(item: FileItem, items: RwSignal<Vec<FileItem>>) -> impl IntoView {
    let preview = item.preview_url.clone();
    on_cleanup(move || {
        if !preview.is_empty() {
            let _ = web_sys::Url::revoke_object_url(&preview);
        }
    });

    let item_strip = item.clone();
    let strip_one = move |_| {
        if item_strip.kind.is_none() || item_strip.size > MAX_FILE_BYTES {
            return;
        }
        run_strip_queue(vec![item_strip.clone()]);
    };

    let item_dl = item.clone();
    let download_one = move |_| {
        let Some(bytes) = item_dl.output.get() else {
            return;
        };
        let report = item_dl.report.get();
        let name = report
            .as_ref()
            .map(|r| r.output_name.clone())
            .unwrap_or_else(|| format!("{}.cleaned", item_dl.name));
        let mime = match item_dl.kind {
            Some(ImageKind::Png) => "image/png",
            _ => "image/jpeg",
        };
        let _ = download_bytes(&name, mime, &bytes);
    };

    let id = item.id;
    let preview_url = item.preview_url.clone();
    let remove = move |_| {
        items.update(|list| {
            if let Some(pos) = list.iter().position(|i| i.id == id) {
                let removed = list.remove(pos);
                if !removed.preview_url.is_empty() {
                    let _ = web_sys::Url::revoke_object_url(&removed.preview_url);
                }
            }
        });
    };

    let kind_label = match item.kind {
        Some(ImageKind::Jpeg) => "JPEG",
        Some(ImageKind::Png) => "PNG",
        None => "unsupported",
    };
    let display_name = item.name.clone();
    let thumb_src = item.preview_url.clone();
    let show_thumb = !preview_url.is_empty() && item.kind.is_some();
    let thumb_view = if show_thumb {
        view! { <img class="thumb" src=thumb_src alt=display_name.clone() /> }.into_any()
    } else {
        view! { <div class="thumb placeholder" aria-hidden="true"></div> }.into_any()
    };

    view! {
        <li class="file">
            <div class="thumb-wrap">
                {thumb_view}
            </div>
            <div class="file-body">
                <div class="file-head">
                    <span class="file-name">{display_name}</span>
                    <span class="pill">{kind_label}</span>
                    <span class="muted">{format_bytes(item.size)}</span>
                    <StatusPill status=item.status />
                </div>
                <p class="error" role="alert">
                    {move || item.error.get().unwrap_or_default()}
                </p>
                <ReportView report=item.report output_len=item.output />
                <div class="row-actions">
                    <button
                        type="button"
                        on:click=strip_one
                        disabled=move || {
                            item.kind.is_none()
                                || matches!(item.status.get(), Status::Stripping | Status::Done)
                                || item.size > MAX_FILE_BYTES
                        }
                    >
                        "Strip"
                    </button>
                    <button
                        type="button"
                        on:click=download_one
                        disabled=move || item.output.get().is_none()
                    >
                        "Download"
                    </button>
                    <button type="button" class="ghost" on:click=remove>
                        "Remove"
                    </button>
                </div>
            </div>
        </li>
    }
}

#[component]
fn StatusPill(status: RwSignal<Status>) -> impl IntoView {
    view! {
        <span class=move || {
            match status.get() {
                Status::Queued => "pill",
                Status::Stripping => "pill warn",
                Status::Done => "pill ok",
                Status::Error | Status::Unsupported => "pill bad",
            }
        }>
            {move || match status.get() {
                Status::Queued => "queued",
                Status::Stripping => "stripping",
                Status::Done => "done",
                Status::Error => "error",
                Status::Unsupported => "unsupported",
            }}
        </span>
    }
}

#[component]
fn ReportView(
    report: RwSignal<Option<StripReport>>,
    output_len: RwSignal<Option<Vec<u8>>>,
) -> impl IntoView {
    view! {
        <Show when=move || report.get().is_some()>
            {move || {
                let Some(r) = report.get() else {
                    return view! { <></> }.into_any();
                };
                let out = output_len.get().map(|b| b.len()).unwrap_or(r.output_bytes);
                let tags = r
                    .removed
                    .iter()
                    .map(|t| format!("{}: {}", t.family.as_str(), t.label))
                    .collect::<Vec<_>>()
                    .join(" · ");
                let families = unique_families(&r);
                view! {
                    <div class="report">
                        <p>
                            {format_bytes(r.input_bytes)}
                            " → "
                            {format_bytes(out)}
                            " · removed "
                            {families}
                        </p>
                        <p class="muted tags">{tags}</p>
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}

fn unique_families(report: &StripReport) -> String {
    let mut seen = Vec::new();
    for tag in &report.removed {
        let name = tag.family.as_str();
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    if seen.is_empty() {
        "no identity tags found".into()
    } else {
        seen.join(", ")
    }
}

fn collect_file_list(list: &web_sys::FileList) -> Vec<web_sys::File> {
    (0..list.length()).filter_map(|i| list.item(i)).collect()
}

async fn read_file_bytes(file: web_sys::File) -> Result<Vec<u8>, String> {
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
        .await
        .map_err(|e| format!("{e:?}"))?;
    let array = js_sys::Uint8Array::new(&buffer);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    Ok(bytes)
}

fn run_strip_queue(items: Vec<FileItem>) {
    if items.is_empty() {
        return;
    }
    let queue = Rc::new(RefCell::new(VecDeque::from(items)));
    for _ in 0..STRIP_CONCURRENCY {
        let queue = Rc::clone(&queue);
        leptos::task::spawn_local(async move {
            loop {
                let next = queue.borrow_mut().pop_front();
                let Some(item) = next else { break };
                item.status.set(Status::Stripping);
                item.error.set(None);
                gloo_timers::future::TimeoutFuture::new(0).await;
                let name = item.name.clone();
                let bytes = Arc::clone(&item.original);
                match strip_image(&name, &bytes) {
                    Ok((out, report)) => {
                        item.output.set(Some(out));
                        item.report.set(Some(report));
                        item.status.set(Status::Done);
                    }
                    Err(err) => {
                        item.error.set(Some(err.to_string()));
                        item.status.set(Status::Error);
                    }
                }
                gloo_timers::future::TimeoutFuture::new(0).await;
            }
        });
    }
}

fn download_bytes(filename: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("blob: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("object url: {e:?}"))?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| "no document".to_string())?;
    let anchor = document
        .create_element("a")
        .map_err(|e| format!("element: {e:?}"))?;
    anchor
        .set_attribute("href", &url)
        .map_err(|e| format!("href: {e:?}"))?;
    anchor
        .set_attribute("download", filename)
        .map_err(|e| format!("download: {e:?}"))?;
    let element: web_sys::HtmlElement = anchor
        .dyn_into()
        .map_err(|_| "anchor cast failed".to_string())?;
    element.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

fn format_bytes(n: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    if n < 1024 {
        format!("{n} B")
    } else if (n as f64) < MB {
        format!("{:.1} KB", n as f64 / KB)
    } else {
        format!("{:.1} MB", n as f64 / MB)
    }
}
