//! Import one constrained EPUB 3 rendition into the reader manifest contract.

mod archive;
mod package;

use std::{
    error::Error,
    fmt, fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

pub const READER_MANIFEST: &str = ".atha-reader.json";

const IMPORT_MARKER: &str = ".atha-epub-import";
const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_SECTIONS: usize = 1_000;
const MAX_TOC_ITEMS: usize = 2_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedBook {
    pub root: PathBuf,
    pub content_version: String,
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
        return Ok(ImportedBook {
            root: target,
            content_version,
        });
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
    Ok(ImportedBook {
        root: target,
        content_version,
    })
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
    let mut manifest =
        File::create(staging.join(READER_MANIFEST)).map_err(|_| ImportError::WriteFailed)?;
    serde_json::to_writer_pretty(&mut manifest, &plan.manifest)
        .map_err(|_| ImportError::WriteFailed)?;
    manifest
        .write_all(b"\n")
        .map_err(|_| ImportError::WriteFailed)?;
    fs::write(
        staging.join(IMPORT_MARKER),
        format!("atha-epub-import-v1\n{content_version}\n"),
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
            .is_ok_and(|value| value == format!("atha-epub-import-v1\n{content_version}\n"))
}

fn publish(staging: &Path, target: &Path, content_version: &str) -> Result<(), ImportError> {
    if complete_cache(target, content_version) {
        fs::remove_dir_all(staging).map_err(|_| ImportError::WriteFailed)?;
        return Ok(());
    }
    if target.exists() {
        fs::remove_dir_all(target).map_err(|_| ImportError::WriteFailed)?;
    }
    match fs::rename(staging, target) {
        Ok(()) => Ok(()),
        Err(_) if complete_cache(target, content_version) => {
            fs::remove_dir_all(staging).map_err(|_| ImportError::WriteFailed)
        }
        Err(_) => Err(ImportError::WriteFailed),
    }
}
