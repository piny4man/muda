use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::*;
use muda_core::strip_image;

use super::model::{FileItem, Status, STRIP_CONCURRENCY};

pub(crate) fn run_strip_queue(items: Vec<FileItem>) {
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
