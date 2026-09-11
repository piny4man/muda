use std::sync::Arc;

use leptos::prelude::*;
use muda_core::sniff_kind;

use crate::zip_store::zip_store;

use super::chrome::{PrivacyFooter, SelectiveStub};
use super::dropzone::DropZone;
use super::hero::Hero;
use super::io::{collect_file_list, download_bytes, read_file_bytes};
use super::model::{FileItem, Status, MAX_FILE_BYTES};
use super::queue::FileQueue;
use super::strip::run_strip_queue;
use super::toolbar::Toolbar;

#[component]
pub(crate) fn HomePage() -> impl IntoView {
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
        <div class=move || {
            if items.get().is_empty() { "page" } else { "page has-queue" }
        }>
            <Hero/>
            <DropZone
                dragging=dragging
                input_ref=input_ref
                on_change=on_input_change
                on_drop=on_drop
            />
            <Show when=move || banner.get().is_some()>
                <p class="banner" role="status">{move || banner.get().unwrap_or_default()}</p>
            </Show>
            <Toolbar strip_all=strip_all download_all=download_all clear=clear />
            <SelectiveStub/>
            <FileQueue items=items />
            <PrivacyFooter/>
        </div>
    }
}
