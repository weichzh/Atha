//! Import one constrained EPUB 2 or EPUB 3 rendition into the reader manifest contract.

mod archive;
mod package;

use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesStart, Event},
};
use serde::{Deserialize, Serialize};

use crate::reader::archive::{ArchiveError, MAX_SOURCE_BYTES};

pub const READER_MANIFEST: &str = ".atha-reader.json";

const IMPORT_MARKER: &str = ".atha-epub-import";
const BOOK_METADATA: &str = ".atha-book.json";
use crate::reader::archive::MAX_ENTRIES;
const MAX_SECTIONS: usize = 2_000;
const MAX_TOC_ITEMS: usize = 2_000;
const MAX_IMAGE_SIDE: usize = 8_192;
const MAX_IMAGE_PIXELS: usize = 20_000_000;

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

impl From<ArchiveError> for ImportError {
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::InvalidSource => Self::InvalidSource,
            ArchiveError::SourceTooLarge => Self::SourceTooLarge,
            ArchiveError::InvalidArchive => Self::InvalidArchive,
            ArchiveError::ArchiveTooLarge => Self::ArchiveTooLarge,
            ArchiveError::UnsafePath => Self::UnsafePath,
            ArchiveError::Encrypted => Self::Encrypted,
            ArchiveError::WriteFailed => Self::WriteFailed,
        }
    }
}

pub fn import_epub(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let (source, content_version, source_file) = fingerprinted_source(source.as_ref())?;
    let _import_guard = crate::reader::lock_import();
    let cache_root = cache_root.as_ref();
    fs::create_dir_all(cache_root).map_err(|_| ImportError::WriteFailed)?;
    let target = cache_root.join(&content_version);
    if current_cache(&target, &content_version) {
        return imported_book(target, content_version);
    }

    let staging = cache_root.join(format!(".{content_version}.staging-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|_| ImportError::WriteFailed)?;
    }
    fs::create_dir(&staging).map_err(|_| ImportError::WriteFailed)?;

    let result = build_import(source_file, &source, &staging, &content_version)
        .and_then(|()| publish(&staging, &target, &content_version));
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    imported_book(target, content_version)
}

pub(super) fn source_identity(source: impl AsRef<Path>) -> Result<String, ImportError> {
    fingerprinted_source(source.as_ref()).map(|(_, content_version, _)| content_version)
}

fn fingerprinted_source(source: &Path) -> Result<(PathBuf, String, File), ImportError> {
    let source = fs::canonicalize(source).map_err(|_| ImportError::InvalidSource)?;
    let metadata = source.metadata().map_err(|_| ImportError::InvalidSource)?;
    if !metadata.is_file() {
        return Err(ImportError::InvalidSource);
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err(ImportError::SourceTooLarge);
    }

    let (content_version, source_file) = archive::fingerprint(&source)?;
    Ok((source, content_version, source_file))
}

fn build_import(
    source_file: File,
    source_path: &Path,
    staging: &Path,
    content_version: &str,
) -> Result<(), ImportError> {
    let mut epub = archive::open(source_file)?;
    let index = archive::inspect(&mut epub)?;
    let container = archive::read(&mut epub, &index, "META-INF/container.xml")?;
    let package_path = package::parse_container(&container)?;
    let package_xml = archive::read(&mut epub, &index, &package_path)?;
    let publication = package::parse_package(&package_xml, &package_path)?;
    if index.contains("META-INF/encryption.xml") {
        let encryption = archive::read(&mut epub, &index, "META-INF/encryption.xml")?;
        package::validate_font_obfuscation(&encryption, &publication, &index)?;
    }
    let plan = package::plan_import(&mut epub, &index, publication, content_version)?;

    let mut extracted = 0_u64;
    for path in &plan.files {
        archive::copy(&mut epub, &index, path, staging, &mut extracted)?;
    }
    let mut image_dimensions = HashMap::new();
    for path in plan.section_paths() {
        annotate_section_images(staging, path, &mut image_dimensions)?;
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
        format!("atha-epub-import-v5\n{content_version}\n"),
    )
    .map_err(|_| ImportError::WriteFailed)?;
    if archive::hash_file(source_path)? != content_version {
        return Err(ImportError::SourceChanged);
    }
    Ok(())
}

pub(super) fn complete_cache(path: &Path, content_version: &str) -> bool {
    has_cache_marker(path, content_version)
        && read_metadata(path, content_version).is_ok()
        && super::resources::complete_reader_cache(path, content_version)
}

pub(super) fn has_cache_marker(path: &Path, content_version: &str) -> bool {
    fs::read_to_string(path.join(IMPORT_MARKER)).is_ok_and(|value| {
        value == format!("atha-epub-import-v2\n{content_version}\n")
            || value == format!("atha-epub-import-v3\n{content_version}\n")
            || value == format!("atha-epub-import-v4\n{content_version}\n")
            || value == format!("atha-epub-import-v5\n{content_version}\n")
    })
}

pub(super) fn needs_upgrade(path: &Path, content_version: &str) -> bool {
    fs::read_to_string(path.join(IMPORT_MARKER)).is_ok_and(|value| {
        value == format!("atha-epub-import-v2\n{content_version}\n")
            || value == format!("atha-epub-import-v3\n{content_version}\n")
            || value == format!("atha-epub-import-v4\n{content_version}\n")
    })
}

fn current_cache(path: &Path, content_version: &str) -> bool {
    fs::read_to_string(path.join(IMPORT_MARKER))
        .is_ok_and(|value| value == format!("atha-epub-import-v5\n{content_version}\n"))
        && read_metadata(path, content_version).is_ok()
        && super::resources::complete_reader_cache(path, content_version)
}

fn annotate_section_images(
    staging: &Path,
    section_path: &str,
    dimensions: &mut HashMap<String, Option<(usize, usize)>>,
) -> Result<(), ImportError> {
    let path = staging.join(section_path);
    let bytes = fs::read(&path).map_err(|_| ImportError::WriteFailed)?;
    if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
        return Err(ImportError::UnsupportedEpub);
    }
    if let Some(rewritten) = rewrite_image_dimensions(&bytes, staging, section_path, dimensions) {
        fs::write(path, rewritten).map_err(|_| ImportError::WriteFailed)?;
    }
    Ok(())
}

fn rewrite_image_dimensions(
    source: &[u8],
    staging: &Path,
    section_path: &str,
    dimensions: &mut HashMap<String, Option<(usize, usize)>>,
) -> Option<Vec<u8>> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    let mut changed = false;
    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (event, annotated) =
                    annotate_image(&reader, event, staging, section_path, dimensions)?;
                changed |= annotated;
                writer.write_event(Event::Start(event)).ok()?;
            }
            Event::Empty(event) => {
                let (event, annotated) =
                    annotate_image(&reader, event, staging, section_path, dimensions)?;
                changed |= annotated;
                writer.write_event(Event::Empty(event)).ok()?;
            }
            Event::Eof => break,
            event => writer.write_event(event.into_owned()).ok()?,
        }
    }
    changed.then(|| writer.into_inner())
}

fn annotate_image(
    reader: &Reader<&[u8]>,
    event: BytesStart<'_>,
    staging: &Path,
    section_path: &str,
    dimensions: &mut HashMap<String, Option<(usize, usize)>>,
) -> Option<(BytesStart<'static>, bool)> {
    if !local_name(event.name().as_ref()).eq_ignore_ascii_case(b"img") {
        return Some((event.into_owned(), false));
    }

    let mut src = None;
    let mut has_width = false;
    let mut has_height = false;
    for attribute in event.attributes().with_checks(true) {
        let attribute = attribute.ok()?;
        match local_name(attribute.key.as_ref()) {
            b"src" => {
                src = Some(
                    attribute
                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                        .ok()?
                        .into_owned(),
                );
            }
            b"width" | b"data-atha-intrinsic-width" => has_width = true,
            b"height" | b"data-atha-intrinsic-height" => has_height = true,
            _ => {}
        }
    }
    if has_width || has_height {
        return Some((event.into_owned(), false));
    }
    let Some(src) = src else {
        return Some((event.into_owned(), false));
    };
    let Ok((resource, _)) = archive::resolve_reference(section_path, &src) else {
        return Some((event.into_owned(), false));
    };
    let size = if let Some(size) = dimensions.get(&resource) {
        *size
    } else {
        let size = intrinsic_image_size(&staging.join(&resource));
        dimensions.insert(resource, size);
        size
    };
    let Some((width, height)) = size else {
        return Some((event.into_owned(), false));
    };
    let mut output = event.into_owned();
    let width = width.to_string();
    let height = height.to_string();
    output.push_attribute(("width", width.as_str()));
    output.push_attribute(("height", height.as_str()));
    output.push_attribute(("data-atha-native-size", ""));
    Some((output, true))
}

fn intrinsic_image_size(path: &Path) -> Option<(usize, usize)> {
    let size = imagesize::size(path).ok()?;
    let pixels = size.width.checked_mul(size.height)?;
    if size.width == 0
        || size.height == 0
        || size.width > MAX_IMAGE_SIDE
        || size.height > MAX_IMAGE_SIDE
        || pixels > MAX_IMAGE_PIXELS
    {
        return None;
    }
    let orientation = image_orientation(path)?;
    if matches!(orientation, 5..=8) {
        Some((size.height, size.width))
    } else {
        Some((size.width, size.height))
    }
}

fn image_orientation(path: &Path) -> Option<u32> {
    let mut file = File::open(path).ok()?;
    let mut magic = [0_u8; 8];
    file.read_exact(&mut magic).ok()?;
    let has_orientation = if magic.starts_with(&[0xff, 0xd8]) {
        jpeg_has_exif(path)?
    } else {
        magic == [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] && png_has_exif(path)?
    };
    if !has_orientation {
        return Some(1);
    }
    let file = File::open(path).ok()?;
    match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
        Ok(metadata) => metadata
            .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            .map_or(Some(1), |field| {
                field
                    .value
                    .get_uint(0)
                    .filter(|value| (1..=8).contains(value))
            }),
        Err(exif::Error::NotFound(_)) => Some(1),
        Err(_) => None,
    }
}

fn jpeg_has_exif(path: &Path) -> Option<bool> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let mut marker = [0_u8; 2];
    file.read_exact(&mut marker).ok()?;
    if marker != [0xff, 0xd8] {
        return Some(false);
    }
    loop {
        file.read_exact(&mut marker[..1]).ok()?;
        if marker[0] != 0xff {
            return None;
        }
        loop {
            file.read_exact(&mut marker[1..]).ok()?;
            if marker[1] != 0xff {
                break;
            }
        }
        if matches!(marker[1], 0xd9 | 0xda) {
            return Some(false);
        }
        if matches!(marker[1], 0xc0..=0xcf) && !matches!(marker[1], 0xc4 | 0xc8 | 0xcc) {
            return Some(false);
        }
        if marker[1] == 0x01 || matches!(marker[1], 0xd0..=0xd7) {
            continue;
        }
        let mut length = [0_u8; 2];
        file.read_exact(&mut length).ok()?;
        let payload = u16::from_be_bytes(length).checked_sub(2)? as usize;
        if marker[1] == 0xe1 && payload >= 6 {
            let mut signature = [0_u8; 6];
            file.read_exact(&mut signature).ok()?;
            if signature == *b"Exif\0\0" {
                return Some(true);
            }
            file.seek(SeekFrom::Current((payload - signature.len()) as i64))
                .ok()?;
        } else {
            file.seek(SeekFrom::Current(payload as i64)).ok()?;
        }
    }
}

fn png_has_exif(path: &Path) -> Option<bool> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let mut header = [0_u8; 8];
    file.read_exact(&mut header).ok()?;
    if header != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        return Some(false);
    }
    let mut seen_image_data = false;
    loop {
        let mut chunk = [0_u8; 8];
        file.read_exact(&mut chunk).ok()?;
        let length = u32::from_be_bytes(chunk[..4].try_into().ok()?) as u64;
        let kind = &chunk[4..];
        if kind == b"eXIf" {
            return (!seen_image_data).then_some(true);
        }
        if kind == b"IEND" {
            return Some(false);
        }
        seen_image_data |= kind == b"IDAT";
        file.seek(SeekFrom::Current(
            i64::try_from(length.checked_add(4)?).ok()?,
        ))
        .ok()?;
    }
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
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
    if current_cache(target, content_version) {
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
        Err(_) if current_cache(target, content_version) => {
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
