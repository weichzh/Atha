//! Local message facts, revisions, source captures, relationships, and export.

mod export;
mod legacy;
mod model;
mod query;
mod schema;
mod store;
mod util;
mod write;

pub use model::*;
pub use store::MessageStore;
