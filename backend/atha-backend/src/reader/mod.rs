//! Security boundaries shared by the reader host.

use std::sync::{Mutex, MutexGuard, PoisonError};

mod archive;
pub mod cbz;
pub mod dictionary;
pub mod epub;
pub mod fb2;
pub mod kindle;
pub mod library;
pub mod resources;
mod source;
pub mod telemetry;
pub mod text;

pub use source::MAX_SOURCE_BYTES;

pub(super) const MAX_MANIFEST_SECTIONS: usize = 1_000;
pub(super) const MAX_MANIFEST_TOC_ITEMS: usize = 2_000;

static IMPORT_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn lock_import() -> MutexGuard<'static, ()> {
    // ponytail: one process-wide lock; shard by content ID if parallel first opens become measurable.
    IMPORT_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

pub const READER_ORIGIN: &str = "https://atha.localhost";
pub const READER_PAGE: &str = "https://atha.localhost/atha-reader.html";
