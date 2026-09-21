use leptos::prelude::*;
use muda_core::{FileKind, StripReport};

use super::io::{download_bytes, format_bytes};
use super::model::{FileItem, KeepSelection, Status};
use super::size::file_warning;
use super::strip::run_strip_queue;

#[component]
pub(crate) fn FileQueue(
    items: RwSignal<Vec<FileItem>>,
    keep: RwSignal<KeepSelection>,
) -> impl IntoView {
    view! {
        <Show
            when=move || !items.get().is_empty()
            fallback=|| view! {
                <p class="empty">
                    "No files yet. Add JPEG, PNG, WebP, or PDF files to strip metadata in your browser."
                </p>
            }
        >
            <ul class="files" role="list">
                <For
                    each=move || items.get()
                    key=|item| item.id
                    children=move |item| {
                        view! { <FileRow item=item items=items keep=keep /> }
                    }
                />
            </ul>
        </Show>
    }
}

#[component]
fn FileRow(
    item: FileItem,
    items: RwSignal<Vec<FileItem>>,
    keep: RwSignal<KeepSelection>,
) -> impl IntoView {
    let preview = item.preview_url.clone();
    on_cleanup(move || {
        if !preview.is_empty() {
            let _ = web_sys::Url::revoke_object_url(&preview);
        }
    });

    let item_strip = item.clone();
    let strip_one = move |_| {
        if item_strip.kind.is_none() {
            return;
        }
        run_strip_queue(vec![item_strip.clone()], keep.get().families());
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
            Some(FileKind::Png) => "image/png",
            Some(FileKind::WebP) => "image/webp",
            Some(FileKind::Pdf) => "application/pdf",
            Some(FileKind::Jpeg) | None => "image/jpeg",
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
        Some(FileKind::Jpeg) => "JPEG",
        Some(FileKind::Png) => "PNG",
        Some(FileKind::WebP) => "WebP",
        Some(FileKind::Pdf) => "PDF",
        None => "unsupported",
    };
    let large_warning = item.kind.and_then(|_| file_warning(item.size));
    let display_name = item.name.clone();
    let thumb_src = item.preview_url.clone();
    let show_thumb = !preview_url.is_empty()
        && matches!(
            item.kind,
            Some(FileKind::Jpeg | FileKind::Png | FileKind::WebP)
        );
    let show_pdf = shows_pdf_placeholder(item.kind);
    let thumb_view = if show_thumb {
        view! { <img class="thumb" src=thumb_src alt=display_name.clone() /> }.into_any()
    } else if show_pdf {
        view! {
            <div class="thumb placeholder pdf" aria-hidden="true">
                "PDF"
            </div>
        }
        .into_any()
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
                    <span class="stamp">{kind_label}</span>
                    {large_warning.map(|_| {
                        view! { <span class="stamp warn">"large"</span> }
                    })}
                    <span class="muted">{format_bytes(item.size)}</span>
                    <StatusPill status=item.status />
                </div>
                <p class="notice" role="status">
                    {large_warning.unwrap_or_default()}
                </p>
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
                Status::Queued => "stamp",
                Status::Stripping => "stamp warn",
                Status::Done => "stamp ok",
                Status::Error | Status::Unsupported => "stamp bad",
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
                report.get().map(|r| {
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
                })
            }}
        </Show>
    }
}

/// PDFs have no raster preview, so the thumbnail shows a document tag instead.
pub(crate) fn shows_pdf_placeholder(kind: Option<FileKind>) -> bool {
    matches!(kind, Some(FileKind::Pdf))
}

pub(crate) fn unique_families(report: &StripReport) -> String {
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

#[cfg(test)]
mod tests {
    use muda_core::{FileKind, RemovedTag, StripReport, TagFamily};

    use super::{shows_pdf_placeholder, unique_families};

    fn report(removed: Vec<RemovedTag>) -> StripReport {
        StripReport {
            original_name: "shot.jpg".into(),
            kind: FileKind::Jpeg,
            output_name: "shot.cleaned.jpg".into(),
            removed,
            warnings: Vec::new(),
            input_bytes: 10,
            output_bytes: 8,
        }
    }

    fn tag(family: TagFamily, label: &str) -> RemovedTag {
        RemovedTag {
            family,
            label: label.into(),
        }
    }

    #[test]
    fn unique_families_empty() {
        assert_eq!(unique_families(&report(vec![])), "no identity tags found");
    }

    #[test]
    fn pdf_gets_placeholder_but_images_do_not() {
        assert!(shows_pdf_placeholder(Some(FileKind::Pdf)));
        assert!(!shows_pdf_placeholder(Some(FileKind::Jpeg)));
        assert!(!shows_pdf_placeholder(Some(FileKind::Png)));
        assert!(!shows_pdf_placeholder(Some(FileKind::WebP)));
        assert!(!shows_pdf_placeholder(None));
    }

    #[test]
    fn unique_families_dedupes_and_keeps_order() {
        let r = report(vec![
            tag(TagFamily::Gps, "GPSLatitude"),
            tag(TagFamily::Camera, "Make"),
            tag(TagFamily::Gps, "GPSLongitude"),
            tag(TagFamily::Comment, "UserComment"),
        ]);
        assert_eq!(unique_families(&r), "GPS, Camera, Comment");
    }
}
