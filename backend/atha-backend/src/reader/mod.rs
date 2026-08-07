//! Security boundaries shared by the reader host.

mod archive;
pub use archive::MAX_SOURCE_BYTES;
pub mod cbz;
pub mod epub;
pub mod library;
pub mod resources;
pub mod telemetry;

pub const READER_ORIGIN: &str = "https://atha.localhost";
pub const READER_PAGE: &str = "https://atha.localhost/atha-reader.html";
