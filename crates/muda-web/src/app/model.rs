use std::sync::Arc;

use leptos::prelude::*;
use muda_core::{ImageKind, StripReport};

pub(crate) const STRIP_CONCURRENCY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Queued,
    Stripping,
    Done,
    Error,
    Unsupported,
}

#[derive(Clone)]
pub(crate) struct FileItem {
    pub id: u64,
    pub name: String,
    pub size: usize,
    pub kind: Option<ImageKind>,
    pub preview_url: String,
    pub original: Arc<[u8]>,
    pub status: RwSignal<Status>,
    pub report: RwSignal<Option<StripReport>>,
    pub output: RwSignal<Option<Vec<u8>>>,
    pub error: RwSignal<Option<String>>,
}
