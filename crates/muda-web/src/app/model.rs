use std::sync::Arc;

use leptos::prelude::*;
use muda_core::{FileKind, StripReport, TagFamily};

pub(crate) const STRIP_CONCURRENCY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Queued,
    Stripping,
    Done,
    Error,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeepSelection {
    pub gps: bool,
    pub camera: bool,
    pub software: bool,
    pub comment: bool,
}

impl KeepSelection {
    pub(crate) fn families(self) -> Vec<TagFamily> {
        let mut families = Vec::new();
        if self.gps {
            families.push(TagFamily::Gps);
        }
        if self.camera {
            families.push(TagFamily::Camera);
        }
        if self.software {
            families.push(TagFamily::Software);
        }
        if self.comment {
            families.push(TagFamily::Comment);
        }
        families
    }
}

#[derive(Clone)]
pub(crate) struct FileItem {
    pub id: u64,
    pub name: String,
    pub size: usize,
    pub kind: Option<FileKind>,
    pub preview_url: String,
    pub original: Arc<[u8]>,
    pub status: RwSignal<Status>,
    pub report: RwSignal<Option<StripReport>>,
    pub output: RwSignal<Option<Vec<u8>>>,
    pub error: RwSignal<Option<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keep_is_empty() {
        assert!(KeepSelection::default().families().is_empty());
    }

    #[test]
    fn families_follow_flags() {
        let keep = KeepSelection {
            gps: true,
            comment: true,
            ..KeepSelection::default()
        };
        assert_eq!(keep.families(), vec![TagFamily::Gps, TagFamily::Comment]);
    }
}
