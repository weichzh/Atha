//! Import one constrained EPUB 2 or EPUB 3 rendition into the reader manifest contract.

mod archive;
mod package;

use std::{
    error::Error,
    fmt, fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const READER_MANIFEST: &str = ".atha-reader.json";
pub const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;

const IMPORT_MARKER: &str = ".atha-epub-import";
const BOOK_METADATA: &str = ".atha-book.json";
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_SECTIONS: usize = 1_000;
const MAX_TOC_ITEMS: usize = 2_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedBook {
    pub root: PathBuf,
    pub content_version: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub cover_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BookMetadata {
    schema: u8,
    #[serde(rename = "contentVersion")]
    content_version: String,
    title: Option<String>,
    authors: Vec<String>,
    #[serde(rename = "coverPath")]
    cover_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportError {
    InvalidSource,
    SourceTooLarge,
    InvalidArchive,
    ArchiveTooLarge,
    UnsafePath,
    Encrypted,
    InvalidXml,
    UnsupportedEpub,
    TooManySections,
    TooManyTocItems,
    WriteFailed,
    SourceChanged,
}

impl ImportError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidSource => "invalid-epub-source",
            Self::SourceTooLarge => "epub-source-too-large",
            Self::InvalidArchive => "invalid-epub-archive",
            Self::ArchiveTooLarge => "epub-archive-too-large",
            Self::UnsafePath => "unsafe-epub-path",
            Self::Encrypted => "encrypted-epub",
            Self::InvalidXml => "invalid-epub-xml",
            Self::UnsupportedEpub => "unsupported-epub",
            Self::TooManySections => "too-many-epub-sections",
            Self::TooManyTocItems => "too-many-epub-toc-items",
            Self::WriteFailed => "epub-import-write-failed",
            Self::SourceChanged => "epub-source-changed",
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for ImportError {}

pub fn import_epub(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let source = fs::canonicalize(source).map_err(|_| ImportError::InvalidSource)?;
    let metadata = source.metadata().map_err(|_| ImportError::InvalidSource)?;
    if !metadata.is_file() {
        return Err(ImportError::InvalidSource);
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err(ImportError::SourceTooLarge);
    }

    let content_version = archive::hash_file(&source)?;
    let cache_root = cache_root.as_ref();
    fs::create_dir_all(cache_root).map_err(|_| ImportError::WriteFailed)?;
    let target = cache_root.join(&content_version);
    if complete_cache(&target, &content_version) {
        return imported_book(target, content_version);
    }

    let staging = cache_root.join(format!(".{content_version}.staging-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|_| ImportError::WriteFailed)?;
    }
    fs::create_dir(&staging).map_err(|_| ImportError::WriteFailed)?;

    let result = build_import(&source, &staging, &content_version)
        .and_then(|()| publish(&staging, &target, &content_version));
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    imported_book(target, content_version)
}

fn build_import(source: &Path, staging: &Path, content_version: &str) -> Result<(), ImportError> {
    let mut epub = archive::open(source)?;
    let index = archive::inspect(&mut epub)?;
    archive::verify_mimetype(&mut epub, &index)?;
    if index.contains("META-INF/encryption.xml") {
        return Err(ImportError::Encrypted);
    }

    let container = archive::read(&mut epub, &index, "META-INF/container.xml")?;
    let package_path = package::parse_container(&container)?;
    let package_xml = archive::read(&mut epub, &index, &package_path)?;
    let publication = package::parse_package(&package_xml, &package_path)?;
    let plan = package::plan_import(&mut epub, &index, publication, content_version)?;

    let mut extracted = 0_u64;
    for path in &plan.files {
        archive::copy(&mut epub, &index, path, staging, &mut extracted)?;
    }
    for path in plan.section_paths() {
        let bytes = fs::read(staging.join(path)).map_err(|_| ImportError::WriteFailed)?;
        if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
            return Err(ImportError::UnsupportedEpub);
        }
    }
    let mut manifest =
        File::create(staging.join(READER_MANIFEST)).map_err(|_| ImportError::WriteFailed)?;
    serde_json::to_writer_pretty(&mut manifest, &plan.manifest)
        .map_err(|_| ImportError::WriteFailed)?;
    manifest
        .write_all(b"\n")
        .map_err(|_| ImportError::WriteFailed)?;
    let metadata = BookMetadata {
        schema: 1,
        content_version: content_version.to_owned(),
        title: plan.title,
        authors: plan.authors,
        cover_path: plan.cover_path,
    };
    let mut metadata_file =
        File::create(staging.join(BOOK_METADATA)).map_err(|_| ImportError::WriteFailed)?;
    serde_json::to_writer_pretty(&mut metadata_file, &metadata)
        .map_err(|_| ImportError::WriteFailed)?;
    metadata_file
        .write_all(b"\n")
        .map_err(|_| ImportError::WriteFailed)?;
    fs::write(
        staging.join(IMPORT_MARKER),
        format!("atha-epub-import-v2\n{content_version}\n"),
    )
    .map_err(|_| ImportError::WriteFailed)?;
    if archive::hash_file(source)? != content_version {
        return Err(ImportError::SourceChanged);
    }
    Ok(())
}

fn complete_cache(path: &Path, content_version: &str) -> bool {
    path.join(READER_MANIFEST).is_file()
        && fs::read_to_string(path.join(IMPORT_MARKER))
            .is_ok_and(|value| value == format!("atha-epub-import-v2\n{content_version}\n"))
        && read_metadata(path, content_version).is_ok()
}

fn imported_book(root: PathBuf, content_version: String) -> Result<ImportedBook, ImportError> {
    let metadata = read_metadata(&root, &content_version)?;
    Ok(ImportedBook {
        root,
        content_version,
        title: metadata.title,
        authors: metadata.authors,
        cover_path: metadata.cover_path,
    })
}

fn read_metadata(path: &Path, content_version: &str) -> Result<BookMetadata, ImportError> {
    let metadata: BookMetadata = serde_json::from_slice(
        &fs::read(path.join(BOOK_METADATA)).map_err(|_| ImportError::WriteFailed)?,
    )
    .map_err(|_| ImportError::WriteFailed)?;
    if metadata.schema != 1
        || metadata.content_version != content_version
        || metadata
            .title
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > package::MAX_METADATA_TEXT)
        || metadata.authors.len() > package::MAX_AUTHORS
        || metadata
            .authors
            .iter()
            .any(|value| value.is_empty() || value.len() > package::MAX_METADATA_TEXT)
        || metadata
            .cover_path
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 512)
    {
        return Err(ImportError::WriteFailed);
    }
    Ok(metadata)
}

fn publish(staging: &Path, target: &Path, content_version: &str) -> Result<(), ImportError> {
    if complete_cache(target, content_version) {
        fs::remove_dir_all(staging).map_err(|_| ImportError::WriteFailed)?;
        return Ok(());
    }
    if target.exists() {
        fs::remove_dir_all(target).map_err(|_| ImportError::WriteFailed)?;
    }
    let mut renamed = fs::rename(staging, target);
    // ponytail: bounded retry covers transient Windows file sharing; use a native move API only if 40 ms proves insufficient.
    for _ in 0..4 {
        if !matches!(&renamed, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        renamed = fs::rename(staging, target);
    }
    match renamed {
        Ok(()) => Ok(()),
        Err(_) if complete_cache(target, content_version) => {
            fs::remove_dir_all(staging).map_err(|_| ImportError::WriteFailed)
        }
        Err(error) => {
            log::warn!(
                target: "atha::reader",
                "operation=import stage=publish-rename outcome=failed code=epub-import-write-failed io_kind={:?}",
                error.kind()
            );
            Err(ImportError::WriteFailed)
        }
    }
}
