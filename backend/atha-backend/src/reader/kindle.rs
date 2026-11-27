//! Bounded MOBI and KF8 projection through the shared reader manifest.

mod guide;
mod huff;
mod projection;
mod structure;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use boko::{Book, ChapterId, Format, Importer, Metadata, SpineEntry, TocEntry};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesStart, BytesText, Event},
};
use serde::{Deserialize, Serialize};

use super::{
    MAX_MANIFEST_SECTIONS, MAX_MANIFEST_TOC_ITEMS,
    classification::{self, SourceClassification, add_label},
    epub::READER_MANIFEST,
    resources::{MAX_READER_RESOURCES, MAX_RESOURCE_BYTES},
    source::{self, SourceError},
};

const IDENTITY_DOMAIN: &[u8] = b"atha/kindle/boko-0.5.0-importer-v1\0";
const IMPORT_MARKER: &str = ".atha-kindle-import";
const IMPORT_MARKER_VERSION: &str = "atha-kindle-import-v1";
const BOOK_METADATA: &str = ".atha-book.json";
const OUTPUT_ROOT: &str = ".atha-kindle";
const SECTION_PREFIX: &str = ".atha-kindle/";
const MAX_KINDLE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RECORD_ZERO_BYTES: u64 = 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 128 * 1024 * 1024;
const LEGACY_HUFF_BUDGET: u64 = 256 * 1024 * 1024;
const MAX_LEGACY_IMAGES: usize = 2_000;
const MAX_RESOURCE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_IMAGE_SIDE: usize = 8_192;
const MAX_IMAGE_PIXELS: usize = 20_000_000;
const MAX_METADATA_CHARS: usize = 512;
const MAX_TOC_LABEL_UTF16_UNITS: usize = 256;
const MAX_AUTHORS: usize = 16;
const NULL_INDEX: u32 = u32::MAX;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedBook {
    pub root: PathBuf,
    pub content_version: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub cover_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportError {
    InvalidSource,
    SourceTooLarge,
    DictionaryUnsupported,
    Encrypted,
    Unsupported,
    InvalidStructure,
    InvalidEncoding,
    TextTooLarge,
    TooManySections,
    TooManyTocItems,
    InvalidMarkup,
    InvalidReference,
    InvalidImage,
    ResourceTooLarge,
    WriteFailed,
    SourceChanged,
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

#[derive(Serialize)]
struct ReaderManifest {
    schema: u8,
    #[serde(rename = "contentVersion")]
    content_version: String,
    sections: Vec<ManifestSection>,
    resources: Vec<String>,
    toc: Vec<ManifestTocItem>,
}

#[derive(Serialize)]
struct ManifestSection {
    id: String,
    href: String,
}

#[derive(Serialize)]
struct ManifestTocItem {
    label: String,
    href: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
}

struct Preflight {
    format: Format,
    huff: Option<huff::Plan>,
    input_bytes: u64,
}

struct BuildStats {
    sections: usize,
    toc_items: usize,
    resources: usize,
    parse_ms: u128,
    render_write_ms: u128,
}

#[derive(Clone, Copy)]
enum ImageKind {
    Gif,
    Jpeg,
    Png,
}

impl ImportError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidSource => "invalid-kindle-source",
            Self::SourceTooLarge => "kindle-source-too-large",
            Self::DictionaryUnsupported => "kindle-dictionary-unsupported",
            Self::Encrypted => "encrypted-kindle",
            Self::Unsupported => "unsupported-kindle",
            Self::InvalidStructure => "invalid-kindle-structure",
            Self::InvalidEncoding => "invalid-kindle-encoding",
            Self::TextTooLarge => "kindle-text-too-large",
            Self::TooManySections => "too-many-kindle-sections",
            Self::TooManyTocItems => "too-many-kindle-toc-items",
            Self::InvalidMarkup => "invalid-kindle-markup",
            Self::InvalidReference => "invalid-kindle-reference",
            Self::InvalidImage => "invalid-kindle-image",
            Self::ResourceTooLarge => "kindle-resource-too-large",
            Self::WriteFailed => "kindle-import-write-failed",
            Self::SourceChanged => "kindle-source-changed",
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for ImportError {}

pub fn import_kindle(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let started = std::time::Instant::now();
    let source = fs::canonicalize(source).map_err(|_| ImportError::InvalidSource)?;
    if !source.is_file() {
        return Err(ImportError::InvalidSource);
    }
    let mut file = File::open(&source).map_err(|_| ImportError::InvalidSource)?;
    preflight(&mut file)?;
    drop(file);
    let _import_guard = super::lock_import();
    let cache_root = cache_root.as_ref();
    fs::create_dir_all(cache_root).map_err(|_| ImportError::WriteFailed)?;
    let staging = cache_root.join(format!(
        ".kindle.staging-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ImportError::WriteFailed)?
            .as_nanos()
    ));
    fs::create_dir(&staging).map_err(|_| ImportError::WriteFailed)?;
    let snapshot = staging.join(".source.kindle");
    let prepared = (|| {
        let mut output = File::create(&snapshot).map_err(|_| ImportError::WriteFailed)?;
        let mut write_failed = false;
        let (content_version, _) = source::fingerprint_with(
            &source,
            IDENTITY_DOMAIN,
            MAX_KINDLE_BYTES,
            |bytes, finished| {
                if !finished && !write_failed && output.write_all(bytes).is_err() {
                    write_failed = true;
                }
            },
        )
        .map_err(source_error)?;
        if write_failed || output.flush().is_err() {
            return Err(ImportError::WriteFailed);
        }
        drop(output);
        let mut snapshot_file = File::open(&snapshot).map_err(|_| ImportError::WriteFailed)?;
        let preflight = preflight(&mut snapshot_file)?;
        Ok((content_version, preflight))
    })();
    let (content_version, preflight) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let load_ms = started.elapsed().as_millis();
    let target = cache_root.join(&content_version);
    if complete_cache(&target, &content_version) {
        if let Err(error) = ensure_source_unchanged(&source, &content_version) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        fs::remove_dir_all(&staging).map_err(|_| ImportError::WriteFailed)?;
        return imported_book(target, content_version);
    }
    let result = (|| {
        let parse_started = std::time::Instant::now();
        let (book_source, mut book, image_records) = if let Some(huff) = preflight.huff {
            let normalized = staging.join(".source.unpacked.kindle");
            let image_records = huff.normalize(&snapshot, &normalized)?;
            fs::remove_file(&snapshot).map_err(|_| ImportError::WriteFailed)?;
            let importer = huff::open_raw(&normalized, preflight.format)?;
            (
                normalized,
                BookView::Raw {
                    importer,
                    toc: None,
                },
                Some(image_records),
            )
        } else {
            let book = Book::open_format(&snapshot, preflight.format)
                .map_err(|_| ImportError::InvalidStructure)?;
            (snapshot, BookView::Existing(book), None)
        };
        book.prepare_toc()?;
        let parse_ms = parse_started.elapsed().as_millis();
        let render_started = std::time::Instant::now();
        let (sections, resources, toc, cover_path) =
            write_book(&book, &staging, image_records.as_ref())?;
        let output_toc_items = toc.len();
        write_json(
            &staging.join(READER_MANIFEST),
            &ReaderManifest {
                schema: 1,
                content_version: content_version.clone(),
                sections,
                resources,
                toc,
            },
        )?;
        let metadata = book.metadata();
        let title = bounded_metadata(&metadata.title);
        let authors = metadata
            .authors
            .iter()
            .filter_map(|value| bounded_metadata(value))
            .take(MAX_AUTHORS)
            .collect::<Vec<_>>();
        let mut classification = SourceClassification::default();
        for subject in &metadata.subjects {
            add_label(&mut classification.subjects, subject);
        }
        write_json(
            &staging.join(BOOK_METADATA),
            &BookMetadata {
                schema: 1,
                content_version: content_version.clone(),
                title,
                authors,
                cover_path,
            },
        )?;
        fs::write(
            staging.join(IMPORT_MARKER),
            format!("{IMPORT_MARKER_VERSION}\n{content_version}\n"),
        )
        .map_err(|_| ImportError::WriteFailed)?;
        ensure_source_unchanged(&source, &content_version)?;
        let stats = BuildStats {
            sections: book.spine().len(),
            toc_items: output_toc_items,
            resources: book
                .list_assets()
                .iter()
                .filter(|path| image_kind(path).is_some())
                .count(),
            parse_ms,
            render_write_ms: render_started.elapsed().as_millis(),
        };
        drop(book);
        fs::remove_file(&book_source).map_err(|_| ImportError::WriteFailed)?;
        publish(&staging, &target, &content_version)?;
        Ok((stats, classification))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let (stats, classification) = result?;
    let _ = classification::write_cache(cache_root, &content_version, classification);
    log::info!(
        target: "atha::reader",
        "operation=import format=kindle outcome=success input_bytes={} sections={} toc_items={} resources={} load_ms={} parse_ms={} render_write_ms={} total_ms={}",
        preflight.input_bytes,
        stats.sections,
        stats.toc_items,
        stats.resources,
        load_ms,
        stats.parse_ms,
        stats.render_write_ms,
        started.elapsed().as_millis()
    );
    imported_book(target, content_version)
}

pub(super) fn source_identity(source: impl AsRef<Path>) -> Result<String, ImportError> {
    let source = fs::canonicalize(source).map_err(|_| ImportError::InvalidSource)?;
    if !source.is_file() {
        return Err(ImportError::InvalidSource);
    }
    let mut file = File::open(&source).map_err(|_| ImportError::InvalidSource)?;
    preflight(&mut file)?;
    source::hash_file(&source, IDENTITY_DOMAIN, MAX_KINDLE_BYTES).map_err(source_error)
}

fn preflight(file: &mut File) -> Result<Preflight, ImportError> {
    let input_bytes = file
        .metadata()
        .map_err(|_| ImportError::InvalidSource)?
        .len();
    if input_bytes > MAX_KINDLE_BYTES {
        return Err(ImportError::SourceTooLarge);
    }
    if input_bytes < 94 {
        return Err(ImportError::InvalidStructure);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ImportError::InvalidSource)?;
    let mut header = [0_u8; 78];
    file.read_exact(&mut header)
        .map_err(|_| ImportError::InvalidStructure)?;
    if &header[60..68] != b"BOOKMOBI" && !header[60..68].eq_ignore_ascii_case(b"TEXTREAD") {
        return Err(ImportError::Unsupported);
    }
    let record_count = usize::from(be_u16(&header, 76)?);
    if record_count < 2 {
        return Err(ImportError::InvalidStructure);
    }
    let table_bytes = record_count
        .checked_mul(8)
        .ok_or(ImportError::InvalidStructure)?;
    let table_end = 78_usize
        .checked_add(table_bytes)
        .ok_or(ImportError::InvalidStructure)?;
    if table_end as u64 > input_bytes {
        return Err(ImportError::InvalidStructure);
    }
    let mut table = vec![0_u8; table_bytes];
    file.read_exact(&mut table)
        .map_err(|_| ImportError::InvalidStructure)?;
    let mut offsets = Vec::with_capacity(record_count);
    for entry in table.chunks_exact(8) {
        offsets.push(u64::from(be_u32(entry, 0)?));
    }
    if offsets[0] < table_end as u64
        || offsets
            .windows(2)
            .any(|pair| pair[0] >= pair[1] || pair[1] > input_bytes)
        || offsets.last().is_none_or(|offset| *offset >= input_bytes)
    {
        return Err(ImportError::InvalidStructure);
    }
    let record_zero_len = offsets[1] - offsets[0];
    if !(16..=MAX_RECORD_ZERO_BYTES).contains(&record_zero_len) {
        return Err(ImportError::InvalidStructure);
    }
    let mut record_zero = vec![0_u8; record_zero_len as usize];
    file.seek(SeekFrom::Start(offsets[0]))
        .and_then(|_| file.read_exact(&mut record_zero))
        .map_err(|_| ImportError::InvalidStructure)?;
    let compression = be_u16(&record_zero, 0)?;
    let declared_text_bytes = u64::from(be_u32(&record_zero, 4)?);
    let text_record_count = usize::from(be_u16(&record_zero, 8)?);
    let encryption = be_u16(&record_zero, 12)?;
    if encryption != 0 {
        return Err(ImportError::Encrypted);
    }
    if text_record_count == 0 || text_record_count >= record_count {
        return Err(ImportError::InvalidStructure);
    }
    let format = if record_zero.len() == 16 {
        if &header[60..68] != b"TEXtREAd" {
            return Err(ImportError::InvalidStructure);
        }
        Format::Mobi
    } else {
        if record_zero.get(16..20) != Some(b"MOBI") {
            return Err(ImportError::InvalidStructure);
        }
        let header_length = usize::try_from(be_u32(&record_zero, 20)?)
            .map_err(|_| ImportError::InvalidStructure)?;
        if header_length < 28
            || 16_usize
                .checked_add(header_length)
                .is_none_or(|end| end > record_zero.len())
        {
            return Err(ImportError::InvalidStructure);
        }
        let encoding = be_u32(&record_zero, 28)?;
        if !matches!(encoding, 1252 | 65001) {
            return Err(ImportError::InvalidEncoding);
        }
        let version = be_u32(&record_zero, 36)?;
        if !(1..=8).contains(&version) {
            return Err(ImportError::Unsupported);
        }
        if version < 8 && be_u32(&record_zero, 40)? != NULL_INDEX {
            return Err(ImportError::DictionaryUnsupported);
        }
        if version == 8 {
            Format::Azw3
        } else {
            Format::Mobi
        }
    };
    if declared_text_bytes == 0 || declared_text_bytes > MAX_TEXT_BYTES {
        return Err(ImportError::TextTooLarge);
    }
    let compressed_text_bytes = offsets
        .get(text_record_count + 1)
        .copied()
        .unwrap_or(input_bytes)
        - offsets[1];
    let huff = match compression {
        1 if compressed_text_bytes <= MAX_TEXT_BYTES => None,
        2 if compressed_text_bytes.saturating_mul(5) <= MAX_TEXT_BYTES => None,
        0x4448 => {
            let huff_index = be_u32(&record_zero, 0x70)? as usize;
            let huff_count = be_u32(&record_zero, 0x74)? as usize;
            let end = huff_index
                .checked_add(huff_count)
                .filter(|end| *end <= record_count)
                .ok_or(ImportError::InvalidStructure)?;
            if huff_count < 2 {
                return Err(ImportError::InvalidStructure);
            }
            // Preserve the pre-P0 path for already admitted books. Only
            // previously rejected HUFF files need the new conservative proof.
            if legacy_huff_admitted(input_bytes, compressed_text_bytes) {
                None
            } else {
                offsets.push(input_bytes);
                Some(huff::Plan {
                    offsets,
                    table,
                    record_zero,
                    text_records: text_record_count,
                    codebook: huff_index..end,
                    is_kf8: matches!(format, Format::Azw3),
                })
            }
        }
        _ => return Err(ImportError::Unsupported),
    };
    Ok(Preflight {
        format,
        input_bytes,
        huff,
    })
}

// The original boko HUFF admission rule; compressed bytes are included
// explicitly to document the old contract.
fn legacy_huff_admitted(file: u64, packed: u64) -> bool {
    [file, packed]
        .into_iter()
        .all(|size| size.saturating_mul(64).saturating_add(4 * 1024 * 1024) <= LEGACY_HUFF_BUDGET)
}

// Keep the established importer for previously admitted books. The extended HUFF path consumes
// only the raw spine, assets and resolved TOC hrefs; it never builds boko IR.
enum BookView {
    Existing(Book),
    Raw {
        importer: Box<dyn Importer>,
        toc: Option<Vec<TocEntry>>,
    },
}

impl BookView {
    fn prepare_toc(&mut self) -> Result<(), ImportError> {
        if self.spine().is_empty() {
            return Err(ImportError::Unsupported);
        }
        if self.spine().len() > MAX_MANIFEST_SECTIONS {
            return Err(ImportError::TooManySections);
        }
        if !self.toc().is_empty() {
            match self {
                Self::Existing(book) => {
                    book.resolve_links()
                        .map_err(|_| ImportError::InvalidStructure)?;
                }
                // Book::resolve_links also fills TOC .target (IR nodes), but
                // ReaderManifest consumes only .href. Resolve those directly.
                Self::Raw { importer, toc } => *toc = importer.resolve_toc(),
            }
        }
        Ok(())
    }

    fn spine(&self) -> &[SpineEntry] {
        match self {
            Self::Existing(book) => book.spine(),
            Self::Raw { importer, .. } => importer.spine(),
        }
    }

    fn metadata(&self) -> &Metadata {
        match self {
            Self::Existing(book) => book.metadata(),
            Self::Raw { importer, .. } => importer.metadata(),
        }
    }

    fn toc(&self) -> &[TocEntry] {
        match self {
            Self::Existing(book) => book.toc(),
            Self::Raw { importer, toc } => toc.as_deref().unwrap_or_else(|| importer.toc()),
        }
    }

    fn source_id(&self, id: ChapterId) -> Option<&str> {
        match self {
            Self::Existing(book) => book.source_id(id),
            Self::Raw { importer, .. } => importer.source_id(id),
        }
    }

    fn load_raw(&self, id: ChapterId) -> boko::Result<Vec<u8>> {
        match self {
            Self::Existing(book) => book.load_raw(id),
            Self::Raw { importer, .. } => importer.load_raw(id),
        }
    }

    fn list_assets(&self) -> &[String] {
        match self {
            Self::Existing(book) => book.list_assets(),
            Self::Raw { importer, .. } => importer.list_assets(),
        }
    }

    fn load_asset(&self, path: &str) -> boko::Result<Vec<u8>> {
        match self {
            Self::Existing(book) => book.load_asset(path),
            Self::Raw { importer, .. } => importer.load_asset(path),
        }
    }
}

type WriteResult = (
    Vec<ManifestSection>,
    Vec<String>,
    Vec<ManifestTocItem>,
    Option<String>,
);

fn write_book(
    book: &BookView,
    staging: &Path,
    image_records: Option<&huff::ImageRecords>,
) -> Result<WriteResult, ImportError> {
    let output_root = staging.join(OUTPUT_ROOT);
    let image_root = output_root.join("images");
    fs::create_dir_all(&image_root).map_err(|_| ImportError::WriteFailed)?;
    let mut asset_map = HashMap::new();
    let mut resources = Vec::new();
    let mut resource_total = 0_u64;
    let image_limit = if image_records.is_some() {
        MAX_READER_RESOURCES
    } else {
        MAX_LEGACY_IMAGES
    };
    for source_path in book.list_assets() {
        let Some(kind) = image_kind(source_path) else {
            continue;
        };
        if resources.len() >= image_limit || asset_map.contains_key(source_path) {
            return Err(ImportError::ResourceTooLarge);
        }
        let expected_bytes = image_records
            .map(|records| records.bytes(source_path))
            .transpose()?;
        if let Some(bytes) = expected_bytes {
            check_image_budget(bytes, resource_total)?;
        }
        let bytes = book
            .load_asset(source_path)
            .map_err(|_| ImportError::InvalidImage)?;
        if expected_bytes.is_some_and(|expected| expected != bytes.len() as u64) {
            return Err(ImportError::InvalidImage);
        }
        validate_image(&bytes, kind)?;
        resource_total = check_image_budget(bytes.len() as u64, resource_total)?;
        let name = format!("image-{:04}.{}", resources.len() + 1, kind.extension());
        let manifest_path = format!("{OUTPUT_ROOT}/images/{name}");
        fs::write(image_root.join(&name), bytes).map_err(|_| ImportError::WriteFailed)?;
        asset_map.insert(source_path.clone(), format!("images/{name}"));
        resources.push(manifest_path);
    }

    let mut source_map = HashMap::new();
    let mut sections = Vec::with_capacity(book.spine().len());
    for (index, entry) in book.spine().iter().enumerate() {
        let source_path = book
            .source_id(entry.id)
            .ok_or(ImportError::InvalidStructure)?;
        let href = format!("{OUTPUT_ROOT}/section-{:04}.xhtml", index + 1);
        if source_map
            .insert(source_path.to_owned(), href.clone())
            .is_some()
        {
            return Err(ImportError::InvalidStructure);
        }
        sections.push(ManifestSection {
            id: format!("section-{:04}", index + 1),
            href,
        });
    }
    let mut text_total = 0_u64;
    let mut guides = guide::Targets::default();
    for (entry, section) in book.spine().iter().zip(&sections) {
        let source_path = book
            .source_id(entry.id)
            .ok_or(ImportError::InvalidStructure)?;
        let raw = book
            .load_raw(entry.id)
            .map_err(|_| ImportError::InvalidStructure)?;
        text_total = text_total
            .checked_add(raw.len() as u64)
            .filter(|total| *total <= MAX_TEXT_BYTES)
            .ok_or(ImportError::TextTooLarge)?;
        let xhtml = sanitize_xhtml(&raw, source_path, &source_map, &asset_map, &mut guides)?;
        if xhtml.len() as u64 > MAX_RESOURCE_BYTES {
            return Err(ImportError::ResourceTooLarge);
        }
        fs::write(staging.join(&section.href), xhtml).map_err(|_| ImportError::WriteFailed)?;
    }
    guides.verify(staging)?;
    let mut toc = Vec::new();
    flatten_toc(book.toc(), &source_map, &mut HashSet::new(), 1, &mut toc)?;
    if toc.is_empty() {
        for (index, section) in sections.iter().enumerate() {
            toc.push(ManifestTocItem {
                label: format!("章节 {}", index + 1),
                href: section.href.clone(),
                level: None,
            });
        }
    }
    if toc.len() > MAX_MANIFEST_TOC_ITEMS {
        return Err(ImportError::TooManyTocItems);
    }
    let cover_path = book
        .metadata()
        .cover_image
        .as_ref()
        .and_then(|path| asset_map.get(path))
        .map(|path| format!("{OUTPUT_ROOT}/{path}"));
    Ok((sections, resources, toc, cover_path))
}

fn check_image_budget(bytes: u64, total: u64) -> Result<u64, ImportError> {
    if bytes == 0 || bytes > MAX_RESOURCE_BYTES {
        return Err(ImportError::ResourceTooLarge);
    }
    total
        .checked_add(bytes)
        .filter(|total| *total <= MAX_RESOURCE_TOTAL_BYTES)
        .ok_or(ImportError::ResourceTooLarge)
}

fn sanitize_xhtml(
    source: &[u8],
    current_source: &str,
    source_map: &HashMap<String, String>,
    asset_map: &HashMap<String, String>,
    guides: &mut guide::Targets,
) -> Result<Vec<u8>, ImportError> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));
    let mut saw_html = false;
    let mut saw_body = false;
    let mut depth = 0_usize;
    let mut root_html = false;
    let mut in_head = false;
    let mut in_guide = false;
    loop {
        match reader
            .read_event()
            .map_err(|_| ImportError::InvalidMarkup)?
        {
            Event::Start(event) => {
                let name = local_name(event.name().as_ref())?;
                if matches!(name.as_str(), "link" | "meta") {
                    return Err(ImportError::InvalidMarkup);
                }
                let output = safe_start(
                    &reader,
                    &event,
                    &name,
                    current_source,
                    source_map,
                    asset_map,
                    None,
                )?;
                if depth == 0 {
                    root_html = event.name().as_ref() == b"html";
                }
                if depth == 1 && root_html && event.name().as_ref() == b"head" {
                    in_head = true;
                }
                if depth == 2 && in_head && event.name().as_ref() == b"guide" {
                    in_guide = true;
                }
                depth += 1;
                saw_html |= name == "html";
                saw_body |= name == "body";
                writer
                    .write_event(Event::Start(output))
                    .map_err(|_| ImportError::InvalidMarkup)?;
            }
            Event::Empty(event) => {
                let name = local_name(event.name().as_ref())?;
                if name == "link" || (name == "meta" && has_http_equiv(&reader, &event)?) {
                    continue;
                }
                let output = safe_start(
                    &reader,
                    &event,
                    &name,
                    current_source,
                    source_map,
                    asset_map,
                    (depth == 3 && in_guide && event.name().as_ref() == b"reference")
                        .then_some(&mut *guides),
                )?;
                writer
                    .write_event(Event::Empty(output))
                    .map_err(|_| ImportError::InvalidMarkup)?;
            }
            Event::End(event) => {
                depth = depth.checked_sub(1).ok_or(ImportError::InvalidMarkup)?;
                if depth == 2 {
                    in_guide = false;
                }
                if depth == 1 {
                    in_head = false;
                }
                writer
                    .write_event(Event::End(event.into_owned()))
                    .map_err(|_| ImportError::InvalidMarkup)?;
            }
            Event::Text(event) => writer
                .write_event(Event::Text(event.into_owned()))
                .map_err(|_| ImportError::InvalidMarkup)?,
            Event::CData(event) => writer
                .write_event(Event::CData(event.into_owned()))
                .map_err(|_| ImportError::InvalidMarkup)?,
            Event::Decl(event) => writer
                .write_event(Event::Decl(event.into_owned()))
                .map_err(|_| ImportError::InvalidMarkup)?,
            Event::GeneralRef(event) if safe_reference(event.as_ref()) => writer
                .write_event(Event::GeneralRef(event.into_owned()))
                .map_err(|_| ImportError::InvalidMarkup)?,
            Event::GeneralRef(event) => {
                let text = html_entity_text(event.as_ref())?;
                writer
                    .write_event(Event::Text(BytesText::new(&text)))
                    .map_err(|_| ImportError::InvalidMarkup)?;
            }
            Event::Comment(_) | Event::DocType(_) => {}
            Event::PI(_) => return Err(ImportError::InvalidMarkup),
            Event::Eof => break,
        }
    }
    if !saw_html || !saw_body {
        return Err(ImportError::InvalidMarkup);
    }
    Ok(writer.into_inner())
}

fn safe_start(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    name: &str,
    current_source: &str,
    source_map: &HashMap<String, String>,
    asset_map: &HashMap<String, String>,
    guides: Option<&mut guide::Targets>,
) -> Result<BytesStart<'static>, ImportError> {
    if matches!(
        name,
        "script"
            | "iframe"
            | "frame"
            | "object"
            | "embed"
            | "form"
            | "input"
            | "button"
            | "select"
            | "textarea"
            | "video"
            | "audio"
            | "source"
            | "track"
            | "base"
            | "style"
    ) {
        return Err(ImportError::InvalidMarkup);
    }
    let mut output = BytesStart::new(
        std::str::from_utf8(event.name().as_ref())
            .map_err(|_| ImportError::InvalidMarkup)?
            .to_owned(),
    );
    let mut guide_href = None;
    let mut is_toc_reference = false;
    for attribute in event.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| ImportError::InvalidMarkup)?;
        let key = std::str::from_utf8(attribute.key.as_ref())
            .map_err(|_| ImportError::InvalidMarkup)?
            .to_owned();
        let lower = key.to_ascii_lowercase();
        if lower.starts_with("on")
            || matches!(
                lower.as_str(),
                "srcset" | "poster" | "action" | "formaction" | "ping" | "target" | "download"
            )
        {
            return Err(ImportError::InvalidMarkup);
        }
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|_| ImportError::InvalidMarkup)?
            .into_owned();
        if value.len() > 4096 {
            return Err(ImportError::InvalidMarkup);
        }
        if guides.is_some() && key == "type" && value == "toc" {
            is_toc_reference = true;
        }
        if guides.is_some() && key == "href" {
            guide_href = Some(value);
            continue;
        }
        if lower == "href"
            && name == "a"
            && value.trim_start_matches("./").starts_with("images/")
            && !asset_map.contains_key(value.trim_start_matches("./"))
        {
            continue;
        }
        let value = match lower.as_str() {
            "href" if name == "a" => {
                rewrite_href(&value, current_source, source_map, Some(asset_map))?
            }
            "href" | "xlink:href" if !value.starts_with('#') => {
                return Err(ImportError::InvalidReference);
            }
            "src" if name == "img" => asset_map
                .get(value.trim_start_matches("./"))
                .cloned()
                .ok_or(ImportError::InvalidReference)?,
            "src" => return Err(ImportError::InvalidReference),
            "style" if unsafe_css(&value) => return Err(ImportError::InvalidMarkup),
            _ => value,
        };
        output.push_attribute((key.as_str(), value.as_str()));
    }
    if let (Some(guides), Some(href)) = (guides, guide_href) {
        if is_toc_reference {
            if output
                .try_get_attribute(guide::TARGET_ATTRIBUTE)
                .map_err(|_| ImportError::InvalidMarkup)?
                .is_some()
            {
                return Err(ImportError::InvalidMarkup);
            }
            let target = guides.project(&href, current_source, source_map)?;
            // Keep metadata inert; the ReaderManifest TOC is separate.
            output.push_attribute((guide::TARGET_ATTRIBUTE, target.as_str()));
        } else if href.starts_with('#') {
            output.push_attribute(("href", href.as_str()));
        } else {
            return Err(ImportError::InvalidReference);
        }
    }
    Ok(output.into_owned())
}

fn has_http_equiv(reader: &Reader<&[u8]>, event: &BytesStart<'_>) -> Result<bool, ImportError> {
    for attribute in event.attributes().with_checks(true) {
        let attribute = attribute.map_err(|_| ImportError::InvalidMarkup)?;
        let key =
            std::str::from_utf8(attribute.key.as_ref()).map_err(|_| ImportError::InvalidMarkup)?;
        if key.eq_ignore_ascii_case("http-equiv") {
            let _ = attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map_err(|_| ImportError::InvalidMarkup)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn rewrite_href(
    value: &str,
    current_source: &str,
    source_map: &HashMap<String, String>,
    asset_map: Option<&HashMap<String, String>>,
) -> Result<String, ImportError> {
    if let Some(fragment) = value.strip_prefix('#') {
        if !valid_fragment(fragment) {
            return Err(ImportError::InvalidReference);
        }
        return Ok(value.to_owned());
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return (!value.contains(['\0', ' ', '\r', '\n']))
            .then(|| value.to_owned())
            .ok_or(ImportError::InvalidReference);
    }
    let (path, fragment) = value
        .split_once('#')
        .map_or((value, None), |(path, fragment)| (path, Some(fragment)));
    let path = path.trim_start_matches("./");
    let target = if path.is_empty() {
        source_map.get(current_source).cloned()
    } else {
        source_map
            .get(path)
            .cloned()
            .or_else(|| asset_map.and_then(|assets| assets.get(path).cloned()))
    }
    .ok_or(ImportError::InvalidReference)?;
    match fragment {
        Some(fragment) if valid_fragment(fragment) => Ok(format!("{target}#{fragment}")),
        Some(_) => Err(ImportError::InvalidReference),
        None => Ok(target),
    }
}

fn flatten_toc(
    entries: &[TocEntry],
    source_map: &HashMap<String, String>,
    seen: &mut HashSet<String>,
    depth: u8,
    output: &mut Vec<ManifestTocItem>,
) -> Result<(), ImportError> {
    for entry in entries {
        if output.len() >= MAX_MANIFEST_TOC_ITEMS {
            return Err(ImportError::TooManyTocItems);
        }
        let label = bounded_toc_label(&entry.title).unwrap_or_else(|| "章节".to_owned());
        if entry.href.starts_with('#')
            || entry.href.starts_with("http://")
            || entry.href.starts_with("https://")
        {
            return Err(ImportError::InvalidReference);
        }
        let href = rewrite_href(&entry.href, "", source_map, None)?;
        if href.encode_utf16().count() > 768 {
            return Err(ImportError::InvalidReference);
        }
        let listed = seen.insert(href.clone());
        if listed {
            output.push(ManifestTocItem {
                label,
                href,
                level: (depth > 1).then(|| depth.min(6)),
            });
        }
        flatten_toc(
            &entry.children,
            source_map,
            seen,
            depth.saturating_add(u8::from(listed)),
            output,
        )?;
    }
    Ok(())
}

fn image_kind(path: &str) -> Option<ImageKind> {
    if !path.starts_with("images/")
        || path.contains(['\0', '\\', ':', '%', '?', '#'])
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return None;
    }
    match Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "gif" => Some(ImageKind::Gif),
        "jpg" | "jpeg" => Some(ImageKind::Jpeg),
        "png" => Some(ImageKind::Png),
        _ => None,
    }
}

impl ImageKind {
    const fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

fn validate_image(bytes: &[u8], kind: ImageKind) -> Result<(), ImportError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_RESOURCE_BYTES {
        return Err(ImportError::ResourceTooLarge);
    }
    let magic = match kind {
        ImageKind::Gif => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        ImageKind::Jpeg => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        ImageKind::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
    };
    if !magic {
        return Err(ImportError::InvalidImage);
    }
    let size = imagesize::blob_size(bytes).map_err(|_| ImportError::InvalidImage)?;
    let pixels = size
        .width
        .checked_mul(size.height)
        .ok_or(ImportError::InvalidImage)?;
    if size.width == 0
        || size.height == 0
        || size.width > MAX_IMAGE_SIDE
        || size.height > MAX_IMAGE_SIDE
        || pixels > MAX_IMAGE_PIXELS
    {
        return Err(ImportError::InvalidImage);
    }
    Ok(())
}

fn unsafe_css(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("@import")
        || lower.contains("url(")
        || lower.contains("src(")
        || lower.contains("image(")
        || lower.contains("image-set(")
        || value.contains('\\')
}

fn safe_reference(value: &[u8]) -> bool {
    matches!(value, b"amp" | b"apos" | b"gt" | b"lt" | b"quot")
        || value
            .strip_prefix(b"#")
            .is_some_and(|digits| !digits.is_empty() && digits.iter().all(u8::is_ascii_digit))
        || value
            .strip_prefix(b"#x")
            .is_some_and(|digits| !digits.is_empty() && digits.iter().all(u8::is_ascii_hexdigit))
}

fn html_entity_text(name: &[u8]) -> Result<String, ImportError> {
    // Look up only a complete standard name. No DTD, custom declarations,
    // attribute expansion or recursive interpretation enters this path.
    let mut key = [0; 33];
    if name.len() >= key.len() || !name.iter().all(u8::is_ascii_alphanumeric) {
        return Err(ImportError::InvalidMarkup);
    }
    key[..name.len()].copy_from_slice(name);
    key[name.len()] = b';';
    let (first, second) = web_atoms::NAMED_ENTITIES
        .get(std::str::from_utf8(&key[..=name.len()]).unwrap())
        .copied()
        .ok_or(ImportError::InvalidMarkup)?;
    let mut text = String::with_capacity(8);
    text.push(char::from_u32(first).ok_or(ImportError::InvalidMarkup)?);
    if second != 0 {
        text.push(char::from_u32(second).ok_or(ImportError::InvalidMarkup)?);
    }
    Ok(text)
}

fn local_name(value: &[u8]) -> Result<String, ImportError> {
    let value = value.rsplit(|byte| *byte == b':').next().unwrap_or(value);
    std::str::from_utf8(value)
        .map(str::to_ascii_lowercase)
        .map_err(|_| ImportError::InvalidMarkup)
}

fn be_u16(bytes: &[u8], offset: usize) -> Result<u16, ImportError> {
    let bytes = bytes
        .get(offset..offset + 2)
        .ok_or(ImportError::InvalidStructure)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn be_u32(bytes: &[u8], offset: usize) -> Result<u32, ImportError> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or(ImportError::InvalidStructure)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn bounded_metadata(value: &str) -> Option<String> {
    let value = value
        .chars()
        .filter(|character| !character.is_control() || character.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_METADATA_CHARS).collect())
}

fn bounded_toc_label(value: &str) -> Option<String> {
    let value = bounded_metadata(value)?;
    let mut units = 0;
    Some(
        value
            .chars()
            .take_while(|character| {
                units += character.len_utf16();
                units <= MAX_TOC_LABEL_UTF16_UNITS
            })
            .collect(),
    )
}

fn valid_fragment(value: &str) -> bool {
    !value.is_empty()
        && value.encode_utf16().count() <= 256
        && !value.contains(['\\', '%', '?', '#', '\0', '\r', '\n'])
        && !value.chars().any(char::is_control)
}

fn source_error(error: SourceError) -> ImportError {
    match error {
        SourceError::InvalidSource => ImportError::InvalidSource,
        SourceError::SourceTooLarge => ImportError::SourceTooLarge,
    }
}

fn ensure_source_unchanged(source_path: &Path, expected: &str) -> Result<(), ImportError> {
    let current =
        source::hash_file(source_path, IDENTITY_DOMAIN, MAX_KINDLE_BYTES).map_err(source_error)?;
    (current == expected)
        .then_some(())
        .ok_or(ImportError::SourceChanged)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ImportError> {
    let mut file = File::create(path).map_err(|_| ImportError::WriteFailed)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(|_| ImportError::WriteFailed)?;
    file.write_all(b"\n").map_err(|_| ImportError::WriteFailed)
}

pub(super) fn complete_cache(path: &Path, content_version: &str) -> bool {
    has_cache_marker(path, content_version)
        && read_metadata(path, content_version).is_ok()
        && super::resources::complete_reader_cache(path, content_version)
}

pub(super) fn has_cache_marker(path: &Path, content_version: &str) -> bool {
    fs::read_to_string(path.join(IMPORT_MARKER))
        .is_ok_and(|value| value == format!("{IMPORT_MARKER_VERSION}\n{content_version}\n"))
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
        || metadata.title.as_ref().is_some_and(String::is_empty)
        || metadata.authors.len() > MAX_AUTHORS
        || metadata.authors.iter().any(String::is_empty)
        || metadata.cover_path.as_ref().is_some_and(|path| {
            path.strip_prefix(&format!("{OUTPUT_ROOT}/"))
                .and_then(image_kind)
                .is_none()
        })
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
                "operation=import stage=publish-rename outcome=failed code=kindle-import-write-failed io_kind={:?}",
                error.kind()
            );
            Err(ImportError::WriteFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_named_characters_are_written_as_xml_text() {
        let render = |raw: &[u8]| {
            sanitize_xhtml(
                raw,
                "chapter.html",
                &HashMap::new(),
                &HashMap::new(),
                &mut guide::Targets::default(),
            )
            .map(|bytes| String::from_utf8(bytes).unwrap())
        };
        let text = render(b"<html><body>&ldquo;A&nbsp;B&rdquo; &LT;em&gt;</body></html>").unwrap();
        assert!(text.contains("“A\u{a0}B” &lt;em&gt;"));
        assert_eq!(
            render(b"<html><body>&unrecognizedName;</body></html>"),
            Err(ImportError::InvalidMarkup)
        );
    }

    #[test]
    fn guide_keeps_only_declared_toc_targets_as_passive_metadata() {
        let spine = HashMap::from([
            (
                "chapter-1.html".into(),
                ".atha-kindle/section-0001.xhtml".into(),
            ),
            (
                "chapter-2.html".into(),
                ".atha-kindle/section-0002.xhtml".into(),
            ),
        ]);
        let sanitize = |fragment: &str| {
            let raw = format!(
                "<html><head><guide>{fragment}</guide></head><body><p>Read</p></body></html>"
            );
            sanitize_xhtml(
                raw.as_bytes(),
                "chapter-1.html",
                &spine,
                &HashMap::new(),
                &mut guide::Targets::default(),
            )
            .map(|bytes| String::from_utf8(bytes).unwrap())
        };
        let output = sanitize(
            r##"<reference title="Contents" href="./chapter-2.html#toc" type="toc"/><reference type="toc" href="#local-toc"/>"##,
        )
        .unwrap();
        assert!(output.contains(r##"<reference title="Contents" type="toc" data-atha-guide-href="section-0002.xhtml#toc"/>"##));
        assert!(output.contains(r##"data-atha-guide-href="#local-toc""##));
        let legacy = sanitize(r##"<reference type="text" href="#legacy"/>"##).unwrap();
        assert!(legacy.contains(r##"<reference type="text" href="#legacy"/>"##));
        assert!(!legacy.contains(guide::TARGET_ATTRIBUTE));
        for reference in [
            r#"<reference type="toc" href="pending.html#toc"/>"#,
            r#"<reference type="text" href="chapter-2.html#toc"/>"#,
            r#"<reference type="toc" href="chapter-2.html"/>"#,
        ] {
            assert_eq!(sanitize(reference), Err(ImportError::InvalidReference));
        }
        let body_reference = br#"<html><head/><body><reference type="toc" href="chapter-2.html#toc"/></body></html>"#;
        assert_eq!(
            sanitize_xhtml(
                body_reference,
                "chapter-1.html",
                &spine,
                &HashMap::new(),
                &mut guide::Targets::default()
            ),
            Err(ImportError::InvalidReference)
        );
    }

    #[test]
    fn toc_keeps_depth_without_turning_duplicate_targets_into_extra_parents() {
        fn entry(href: &str, children: Vec<TocEntry>) -> TocEntry {
            TocEntry {
                title: href.to_owned(),
                href: href.to_owned(),
                children,
                play_order: None,
                target: None,
            }
        }
        let root = entry(
            "one.xhtml",
            vec![entry("one.xhtml", vec![entry("two.xhtml", vec![])])],
        );
        let mut toc = Vec::new();
        let source_map = HashMap::from([
            ("one.xhtml".into(), "part.xhtml".into()),
            ("two.xhtml".into(), "chapter.xhtml".into()),
        ]);
        flatten_toc(&[root], &source_map, &mut HashSet::new(), 1, &mut toc).unwrap();
        assert_eq!(
            toc.iter()
                .map(|item| (item.href.as_str(), item.level))
                .collect::<Vec<_>>(),
            [("part.xhtml", None), ("chapter.xhtml", Some(2))]
        );
    }

    #[test]
    fn toc_requires_a_section_path() {
        let entry = TocEntry {
            title: "Fragment only".to_owned(),
            href: "#target".to_owned(),
            children: Vec::new(),
            play_order: None,
            target: None,
        };
        assert_eq!(
            flatten_toc(
                &[entry],
                &HashMap::new(),
                &mut HashSet::new(),
                1,
                &mut Vec::new()
            ),
            Err(ImportError::InvalidReference)
        );
    }

    #[test]
    fn changed_source_fails_the_final_identity_check() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
        fs::create_dir_all(&root).expect("create repository temporary root");
        let path = root.join(format!("atha-kindle-source-check-{}", std::process::id()));
        fs::write(&path, b"before").expect("write source");
        let expected =
            source::hash_file(&path, IDENTITY_DOMAIN, MAX_KINDLE_BYTES).expect("hash source");
        fs::write(&path, b"after").expect("change source");
        assert_eq!(
            ensure_source_unchanged(&path, &expected),
            Err(ImportError::SourceChanged)
        );
        fs::remove_file(path).expect("remove source");
    }
}
