//! Bounded FB2 and FBZ projection into the shared reader manifest.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesEnd, BytesRef, BytesStart, BytesText, Event},
};
use serde::{Deserialize, Serialize};

use super::{
    MAX_MANIFEST_SECTIONS, MAX_MANIFEST_TOC_ITEMS,
    archive::{self, ArchiveError},
    epub::READER_MANIFEST,
    resources::MAX_RESOURCE_BYTES,
    source::{self, SourceDigest, SourceError},
};

const IDENTITY_DOMAIN: &[u8] = b"atha/fb2/importer-v1\0";
const IMPORT_MARKER: &str = ".atha-fb2-import";
const IMPORT_MARKER_VERSION: &str = "atha-fb2-import-v1";
const BOOK_METADATA: &str = ".atha-book.json";
const OUTPUT_ROOT: &str = ".atha-fb2";
const MAX_FB2_BYTES: u64 = 64 * 1024 * 1024;
const MAX_XML_DEPTH: usize = 128;
const MAX_XML_ELEMENTS: usize = 1_000_000;
const MAX_METADATA_CHARS: usize = 512;
const MAX_AUTHORS: usize = 16;
const MAX_IDS: usize = 20_000;
const MAX_IMAGES: usize = 1_000;
const MAX_IMAGE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_IMAGE_SIDE: usize = 8_192;
const MAX_IMAGE_PIXELS: usize = 20_000_000;

const XHTML_PREFIX: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
    "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head>",
    "<meta charset=\"utf-8\" /><style>",
    "body&gt;img,section&gt;img{display:block;max-width:100%;height:auto;margin:auto}",
    ".title{text-align:center;margin-block:2em}",
    "p{margin-block:.35em}",
    ".stanza{margin-block:1em}",
    ".stanza-line{margin:0}",
    ".text-author,.date{text-align:end}",
    "table{border-collapse:collapse}th,td{padding:.25em;border:1px solid currentColor}",
    "a[role=doc-noteref]{font-size:.75em;vertical-align:super}",
    "</style></head><body>\n",
);
const XHTML_SUFFIX: &[u8] = b"</body></html>\n";

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
    InvalidArchive,
    ArchiveTooLarge,
    UnsafePath,
    Encrypted,
    UnsupportedFb2,
    InvalidXml,
    TooManySections,
    TooManyTocItems,
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
    toc: Vec<TocItem>,
}

#[derive(Serialize)]
struct ManifestSection {
    id: String,
    href: String,
}

#[derive(Clone, Serialize)]
struct TocItem {
    label: String,
    href: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SegmentKey {
    MainPreamble,
    MainSection(usize),
    AuxiliaryBody(usize),
}

struct SegmentPlan {
    id: String,
    href: String,
}

struct BinarySource {
    content_type: String,
    encoded: String,
}

struct ImageResource {
    path: String,
}

struct BuildStats {
    sections: usize,
    toc_items: usize,
    resources: usize,
    parse_ms: u128,
    render_write_ms: u128,
}

#[derive(Default)]
struct AuthorParts {
    first: String,
    middle: String,
    last: String,
    nickname: String,
}

struct TocCandidate {
    section_depth: usize,
    title_depth: Option<usize>,
    key: SegmentKey,
    anchor: Option<String>,
    label: String,
    emitted: bool,
}

#[derive(Default)]
struct Plan {
    title: String,
    authors: Vec<String>,
    cover_id: Option<String>,
    binaries: HashMap<String, BinarySource>,
    referenced_images: Vec<String>,
    referenced_image_set: HashSet<String>,
    segments: Vec<SegmentPlan>,
    segment_indexes: HashMap<SegmentKey, usize>,
    id_hrefs: HashMap<String, String>,
    toc: Vec<TocItem>,
    toc_hrefs: HashSet<String>,
    images: Vec<ImageResource>,
    image_paths: HashMap<String, String>,
}

#[derive(Default)]
struct XmlCursor {
    depth: usize,
    path: Vec<String>,
    body_index: Option<usize>,
    body_depth: Option<usize>,
    body_count: usize,
    main_section_index: usize,
    main_section_depth: Option<usize>,
    current_author: Option<AuthorParts>,
    binary: Option<(usize, String, String, String)>,
    toc: Vec<TocCandidate>,
    elements: usize,
    root_seen: bool,
    root_closed: bool,
    open_void_depth: Option<usize>,
}

impl ImportError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidSource => "invalid-fb2-source",
            Self::SourceTooLarge => "fb2-source-too-large",
            Self::InvalidArchive => "invalid-fbz-archive",
            Self::ArchiveTooLarge => "fbz-archive-too-large",
            Self::UnsafePath => "unsafe-fbz-path",
            Self::Encrypted => "encrypted-fbz",
            Self::UnsupportedFb2 => "unsupported-fb2",
            Self::InvalidXml => "invalid-fb2-xml",
            Self::TooManySections => "too-many-fb2-sections",
            Self::TooManyTocItems => "too-many-fb2-toc-items",
            Self::InvalidReference => "invalid-fb2-reference",
            Self::InvalidImage => "invalid-fb2-image",
            Self::ResourceTooLarge => "fb2-resource-too-large",
            Self::WriteFailed => "fb2-import-write-failed",
            Self::SourceChanged => "fb2-source-changed",
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

pub fn import_fb2(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let started = std::time::Instant::now();
    let source = fs::canonicalize(source).map_err(|_| ImportError::InvalidSource)?;
    if !source.is_file() {
        return Err(ImportError::InvalidSource);
    }
    let load_started = std::time::Instant::now();
    let (content_version, xml, format) = load_source(&source)?;
    let input_bytes = xml.len();
    let load_ms = load_started.elapsed().as_millis();
    let _import_guard = super::lock_import();
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
    let result = (|| {
        let parse_started = std::time::Instant::now();
        let mut plan = discover(&xml)?;
        plan.prepare_images(&staging)?;
        let parse_ms = parse_started.elapsed().as_millis();
        let render_started = std::time::Instant::now();
        render_and_write(&xml, &mut plan, &staging, &content_version)?;
        let render_write_ms = render_started.elapsed().as_millis();
        let (current, _, _) = load_source(&source)?;
        if current != content_version {
            return Err(ImportError::SourceChanged);
        }
        let stats = BuildStats {
            sections: plan.segments.len(),
            toc_items: plan.toc.len(),
            resources: plan.images.len(),
            parse_ms,
            render_write_ms,
        };
        publish(&staging, &target, &content_version)?;
        Ok(stats)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let stats = result?;
    log::info!(
        target: "atha::reader",
        "operation=import format={} outcome=success input_bytes={} sections={} toc_items={} resources={} load_ms={} parse_ms={} render_write_ms={} total_ms={}",
        format,
        input_bytes,
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
    load_source(&source).map(|(content_version, _, _)| content_version)
}

fn load_source(source_path: &Path) -> Result<(String, Vec<u8>, &'static str), ImportError> {
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("fb2") {
        let (content_version, file) =
            source::fingerprint(source_path, IDENTITY_DOMAIN, MAX_FB2_BYTES)
                .map_err(source_error)?;
        let xml = read_limited(file, MAX_FB2_BYTES)?;
        return Ok((content_version, xml, "fb2"));
    }
    if !extension.eq_ignore_ascii_case("fbz") {
        return Err(ImportError::UnsupportedFb2);
    }
    let (_, source_file) = archive::fingerprint(source_path)?;
    let mut archive = archive::open_fingerprinted(source_file)?;
    let index = archive::inspect(&mut archive)?;
    let mut paths = index.paths();
    let path = paths.next().ok_or(ImportError::UnsupportedFb2)?;
    if paths.next().is_some()
        || path.contains('/')
        || !path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("fb2"))
    {
        return Err(ImportError::UnsupportedFb2);
    }
    let xml = archive::read(&mut archive, &index, path)?;
    let content_version = digest(&xml)?;
    Ok((content_version, xml, "fbz"))
}

fn read_limited(file: File, max_bytes: u64) -> Result<Vec<u8>, ImportError> {
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ImportError::InvalidSource)?;
    if bytes.is_empty() {
        return Err(ImportError::InvalidSource);
    }
    if bytes.len() as u64 > max_bytes {
        return Err(ImportError::SourceTooLarge);
    }
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> Result<String, ImportError> {
    let mut digest = SourceDigest::new(IDENTITY_DOMAIN, MAX_FB2_BYTES);
    digest.update(bytes).map_err(source_error)?;
    Ok(digest.finish())
}

fn source_error(error: SourceError) -> ImportError {
    match error {
        SourceError::InvalidSource => ImportError::InvalidSource,
        SourceError::SourceTooLarge => ImportError::SourceTooLarge,
    }
}

fn discover(xml: &[u8]) -> Result<Plan, ImportError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut cursor = XmlCursor::default();
    let mut plan = Plan::default();
    loop {
        match reader.read_event().map_err(|_| ImportError::InvalidXml)? {
            Event::Start(event) => discover_start(&reader, &event, false, &mut cursor, &mut plan)?,
            Event::Empty(event) => {
                discover_start(&reader, &event, true, &mut cursor, &mut plan)?;
                discover_end(event.local_name().as_ref(), &mut cursor, &mut plan)?;
            }
            Event::End(event) => discover_end(event.local_name().as_ref(), &mut cursor, &mut plan)?,
            Event::Text(text) => {
                let text = text.decode().map_err(|_| ImportError::InvalidXml)?;
                discover_text(&text, &mut cursor, &mut plan)?;
            }
            Event::CData(text) => {
                let text = text.decode().map_err(|_| ImportError::InvalidXml)?;
                discover_text(&text, &mut cursor, &mut plan)?;
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::GeneralRef(value) => {
                let text = xml_reference(&value)?;
                discover_text(&text, &mut cursor, &mut plan)?;
            }
            Event::DocType(_) | Event::PI(_) => {
                return Err(ImportError::InvalidXml);
            }
            Event::Eof if cursor.depth == 0 => break,
            Event::Eof => return Err(ImportError::InvalidXml),
        }
    }
    if !cursor.root_seen
        || !cursor.root_closed
        || cursor.body_count == 0
        || plan.segments.is_empty()
    {
        return Err(ImportError::UnsupportedFb2);
    }
    if !cursor.toc.is_empty() || cursor.current_author.is_some() || cursor.binary.is_some() {
        return Err(ImportError::InvalidXml);
    }
    plan.title = normalize(&plan.title);
    plan.authors.truncate(MAX_AUTHORS);
    Ok(plan)
}

fn discover_start(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    empty: bool,
    cursor: &mut XmlCursor,
    plan: &mut Plan,
) -> Result<(), ImportError> {
    cursor.depth = cursor
        .depth
        .checked_add(1)
        .filter(|depth| *depth <= MAX_XML_DEPTH)
        .ok_or(ImportError::InvalidXml)?;
    cursor.elements = cursor
        .elements
        .checked_add(1)
        .filter(|count| *count <= MAX_XML_ELEMENTS)
        .ok_or(ImportError::InvalidXml)?;
    let name = decode_name(event.local_name().as_ref())?;
    cursor.path.push(name.clone());
    if cursor
        .open_void_depth
        .is_some_and(|void_depth| cursor.depth > void_depth)
    {
        return Err(ImportError::InvalidXml);
    }
    if cursor.depth == 1 {
        if cursor.root_seen || name != "FictionBook" {
            return Err(ImportError::UnsupportedFb2);
        }
        cursor.root_seen = true;
    } else if cursor.root_closed {
        return Err(ImportError::InvalidXml);
    }

    if name == "body" && cursor.depth == 2 {
        cursor.body_index = Some(cursor.body_count);
        cursor.body_count += 1;
        cursor.body_depth = Some(cursor.depth);
        cursor.main_section_index = 0;
        cursor.main_section_depth = None;
    }
    if cursor.body_index == Some(0)
        && name == "section"
        && cursor.body_depth == Some(cursor.depth.saturating_sub(1))
    {
        cursor.main_section_index += 1;
        cursor.main_section_depth = Some(cursor.depth);
    }
    if cursor.body_index == Some(0)
        && cursor.body_depth == Some(cursor.depth.saturating_sub(1))
        && cursor.main_section_index > 0
        && name != "section"
    {
        return Err(ImportError::UnsupportedFb2);
    }
    let key = current_key(cursor);
    if in_body(cursor) && renderable(&name) {
        let key = key.ok_or(ImportError::UnsupportedFb2)?;
        plan.ensure_segment(key)?;
        if let Some(id) = attribute(reader, event, "id")? {
            plan.insert_id(id, key)?;
        }
        if name == "image" {
            let href = local_reference(reader, event)?;
            plan.reference_image(href)?;
        } else if name == "a" {
            local_reference(reader, event)?;
        }
        if !empty && matches!(name.as_str(), "image" | "empty-line") {
            cursor.open_void_depth = Some(cursor.depth);
        }
        if name == "section" {
            cursor.toc.push(TocCandidate {
                section_depth: cursor.depth,
                title_depth: None,
                key,
                anchor: attribute(reader, event, "id")?,
                label: String::new(),
                emitted: false,
            });
            if cursor.toc.len() > MAX_XML_DEPTH {
                return Err(ImportError::InvalidXml);
            }
        } else if name == "title"
            && cursor
                .toc
                .last()
                .is_some_and(|candidate| candidate.section_depth + 1 == cursor.depth)
        {
            cursor.toc.last_mut().expect("checked TOC").title_depth = Some(cursor.depth);
        }
    } else if in_body(cursor) && name != "body" {
        return Err(ImportError::UnsupportedFb2);
    }

    if path_ends(&cursor.path, &["description", "title-info", "author"]) {
        if cursor.current_author.is_some() {
            return Err(ImportError::InvalidXml);
        }
        cursor.current_author = Some(AuthorParts::default());
    }
    if path_ends(
        &cursor.path,
        &["description", "title-info", "coverpage", "image"],
    ) {
        plan.cover_id = Some(local_reference(reader, event)?);
    }
    if cursor.depth == 2 && name == "binary" {
        let id = required_attribute(reader, event, "id")?;
        let content_type = required_attribute(reader, event, "content-type")?;
        if image_kind(&content_type).is_none() {
            return Err(ImportError::InvalidImage);
        }
        if plan.binaries.contains_key(&id) || cursor.binary.is_some() {
            return Err(ImportError::InvalidXml);
        }
        validate_id(&id)?;
        cursor.binary = Some((cursor.depth, id, content_type, String::new()));
    }
    Ok(())
}

fn discover_text(text: &str, cursor: &mut XmlCursor, plan: &mut Plan) -> Result<(), ImportError> {
    if cursor.open_void_depth.is_some() {
        return if xml_space(text) {
            Ok(())
        } else {
            Err(ImportError::InvalidXml)
        };
    }
    if let Some((_, _, _, encoded)) = cursor.binary.as_mut() {
        if encoded.len().saturating_add(text.len()) > MAX_FB2_BYTES as usize {
            return Err(ImportError::ResourceTooLarge);
        }
        encoded.push_str(text);
        return Ok(());
    }
    if path_ends(&cursor.path, &["description", "title-info", "book-title"]) {
        push_bounded(&mut plan.title, text, MAX_METADATA_CHARS)?;
    }
    if let Some(author) = cursor.current_author.as_mut() {
        match cursor.path.last().map(String::as_str) {
            Some("first-name") => push_bounded(&mut author.first, text, MAX_METADATA_CHARS)?,
            Some("middle-name") => push_bounded(&mut author.middle, text, MAX_METADATA_CHARS)?,
            Some("last-name") => push_bounded(&mut author.last, text, MAX_METADATA_CHARS)?,
            Some("nickname") => push_bounded(&mut author.nickname, text, MAX_METADATA_CHARS)?,
            _ => {}
        }
    }
    for candidate in cursor.toc.iter_mut().filter(|candidate| {
        candidate
            .title_depth
            .is_some_and(|depth| cursor.depth >= depth)
    }) {
        push_bounded(&mut candidate.label, text, MAX_METADATA_CHARS)?;
    }
    if in_body(cursor) && cursor.path.last().is_some_and(|name| name == "body") && !xml_space(text)
    {
        return Err(ImportError::UnsupportedFb2);
    }
    Ok(())
}

fn discover_end(name: &[u8], cursor: &mut XmlCursor, plan: &mut Plan) -> Result<(), ImportError> {
    let name = decode_name(name)?;
    if cursor.path.last() != Some(&name) {
        return Err(ImportError::InvalidXml);
    }
    if name == "title"
        && let Some(candidate) = cursor
            .toc
            .last_mut()
            .filter(|candidate| candidate.title_depth == Some(cursor.depth))
    {
        candidate.title_depth = None;
        let label = normalize(&candidate.label);
        if !label.is_empty() && !candidate.emitted {
            let href = candidate
                .anchor
                .as_ref()
                .and_then(|anchor| plan.id_hrefs.get(anchor))
                .cloned()
                .unwrap_or_else(|| plan.segment(candidate.key).href.clone());
            candidate.emitted = true;
            if plan.toc_hrefs.insert(href.clone()) {
                if plan.toc.len() >= MAX_MANIFEST_TOC_ITEMS {
                    return Err(ImportError::TooManyTocItems);
                }
                plan.toc.push(TocItem { label, href });
            }
        }
    }
    if name == "section"
        && cursor
            .toc
            .last()
            .is_some_and(|candidate| candidate.section_depth == cursor.depth)
    {
        cursor.toc.pop().expect("checked TOC");
    }
    if path_ends(&cursor.path, &["description", "title-info", "author"]) {
        let author = cursor
            .current_author
            .take()
            .ok_or(ImportError::InvalidXml)?;
        let nickname = normalize(&author.nickname);
        let name = if nickname.is_empty() {
            [author.first, author.middle, author.last]
                .iter()
                .map(|part| normalize(part))
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            nickname
        };
        if !name.is_empty() && plan.authors.len() < MAX_AUTHORS {
            plan.authors.push(name);
        }
    }
    if let Some((binary_depth, _, _, _)) = cursor.binary.as_ref()
        && *binary_depth == cursor.depth
        && name == "binary"
    {
        let (_, id, content_type, encoded) = cursor.binary.take().ok_or(ImportError::InvalidXml)?;
        plan.binaries.insert(
            id,
            BinarySource {
                content_type,
                encoded,
            },
        );
    }
    if cursor.main_section_depth == Some(cursor.depth) && name == "section" {
        cursor.main_section_depth = None;
    }
    if cursor.open_void_depth == Some(cursor.depth) {
        cursor.open_void_depth = None;
    }
    if cursor.body_depth == Some(cursor.depth) && name == "body" {
        cursor.body_index = None;
        cursor.body_depth = None;
        cursor.main_section_depth = None;
    }
    if cursor.depth == 1 && name == "FictionBook" {
        cursor.root_closed = true;
    }
    cursor.path.pop();
    cursor.depth = cursor.depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
    Ok(())
}

impl Plan {
    fn ensure_segment(&mut self, key: SegmentKey) -> Result<&SegmentPlan, ImportError> {
        if let Some(index) = self.segment_indexes.get(&key).copied() {
            return Ok(&self.segments[index]);
        }
        if self.segments.len() >= MAX_MANIFEST_SECTIONS {
            return Err(ImportError::TooManySections);
        }
        let number = self.segments.len() + 1;
        let segment = SegmentPlan {
            id: format!("section-{number:04}"),
            href: format!("{OUTPUT_ROOT}/section-{number:04}.xhtml"),
        };
        self.segments.push(segment);
        self.segment_indexes.insert(key, self.segments.len() - 1);
        Ok(self.segments.last().expect("inserted segment"))
    }

    fn segment(&self, key: SegmentKey) -> &SegmentPlan {
        &self.segments[*self.segment_indexes.get(&key).expect("known segment")]
    }

    fn insert_id(&mut self, id: String, key: SegmentKey) -> Result<(), ImportError> {
        validate_id(&id)?;
        if self.id_hrefs.len() >= MAX_IDS || self.id_hrefs.contains_key(&id) {
            return Err(ImportError::InvalidReference);
        }
        let href = format!("{}#{id}", self.segment(key).href);
        self.id_hrefs.insert(id, href);
        Ok(())
    }

    fn reference_image(&mut self, id: String) -> Result<(), ImportError> {
        if self.referenced_image_set.insert(id.clone()) {
            if self.referenced_images.len() >= MAX_IMAGES {
                return Err(ImportError::ResourceTooLarge);
            }
            self.referenced_images.push(id);
        }
        Ok(())
    }

    fn prepare_images(&mut self, staging: &Path) -> Result<(), ImportError> {
        if let Some(id) = self.cover_id.clone() {
            self.reference_image(id)?;
        }
        fs::create_dir_all(staging.join(OUTPUT_ROOT).join("images"))
            .map_err(|_| ImportError::WriteFailed)?;
        let mut total = 0_u64;
        for (offset, id) in self.referenced_images.clone().into_iter().enumerate() {
            let source = self
                .binaries
                .remove(&id)
                .ok_or(ImportError::InvalidReference)?;
            let kind = image_kind(&source.content_type).ok_or(ImportError::InvalidImage)?;
            let mut encoded = source.encoded.into_bytes();
            encoded.retain(|byte| !byte.is_ascii_whitespace());
            let bytes = BASE64
                .decode(encoded)
                .map_err(|_| ImportError::InvalidImage)?;
            if bytes.is_empty() || bytes.len() as u64 > MAX_RESOURCE_BYTES {
                return Err(ImportError::ResourceTooLarge);
            }
            validate_image(&bytes, kind)?;
            total = total
                .checked_add(bytes.len() as u64)
                .filter(|total| *total <= MAX_IMAGE_TOTAL_BYTES)
                .ok_or(ImportError::ResourceTooLarge)?;
            let path = format!(
                "{OUTPUT_ROOT}/images/image-{:04}.{}",
                offset + 1,
                kind.extension()
            );
            fs::write(staging.join(&path), bytes).map_err(|_| ImportError::WriteFailed)?;
            self.image_paths.insert(id, path.clone());
            self.images.push(ImageResource { path });
        }
        self.binaries.clear();
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ImageKind {
    Jpeg,
    Png,
}

impl ImageKind {
    const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

fn image_kind(content_type: &str) -> Option<ImageKind> {
    match content_type.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some(ImageKind::Jpeg),
        "image/png" => Some(ImageKind::Png),
        _ => None,
    }
}

fn validate_image(bytes: &[u8], kind: ImageKind) -> Result<(), ImportError> {
    let magic = match kind {
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

struct SegmentOutput {
    bytes: Vec<u8>,
}

fn render_and_write(
    xml: &[u8],
    plan: &mut Plan,
    staging: &Path,
    content_version: &str,
) -> Result<(), ImportError> {
    let mut outputs = plan
        .segments
        .iter()
        .map(|_| SegmentOutput {
            bytes: XHTML_PREFIX.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut cursor = XmlCursor::default();
    let mut suppressed_void_depths = HashSet::new();
    loop {
        match reader.read_event().map_err(|_| ImportError::InvalidXml)? {
            Event::Start(event) => {
                render_start(
                    &reader,
                    &event,
                    false,
                    &mut cursor,
                    plan,
                    &mut outputs,
                    &mut suppressed_void_depths,
                )?;
            }
            Event::Empty(event) => {
                render_start(
                    &reader,
                    &event,
                    true,
                    &mut cursor,
                    plan,
                    &mut outputs,
                    &mut suppressed_void_depths,
                )?;
                render_end(
                    event.local_name().as_ref(),
                    true,
                    &mut cursor,
                    plan,
                    &mut outputs,
                    &mut suppressed_void_depths,
                )?;
            }
            Event::End(event) => render_end(
                event.local_name().as_ref(),
                false,
                &mut cursor,
                plan,
                &mut outputs,
                &mut suppressed_void_depths,
            )?,
            Event::Text(text) => {
                let text = text.decode().map_err(|_| ImportError::InvalidXml)?;
                render_text(&text, &cursor, plan, &mut outputs, &suppressed_void_depths)?;
            }
            Event::CData(text) => {
                let text = text.decode().map_err(|_| ImportError::InvalidXml)?;
                render_text(&text, &cursor, plan, &mut outputs, &suppressed_void_depths)?;
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::GeneralRef(value) => {
                let text = xml_reference(&value)?;
                render_text(&text, &cursor, plan, &mut outputs, &suppressed_void_depths)?;
            }
            Event::DocType(_) | Event::PI(_) => {
                return Err(ImportError::InvalidXml);
            }
            Event::Eof if cursor.depth == 0 => break,
            Event::Eof => return Err(ImportError::InvalidXml),
        }
    }
    if !suppressed_void_depths.is_empty() {
        return Err(ImportError::InvalidXml);
    }
    let output_root = staging.join(OUTPUT_ROOT);
    fs::create_dir_all(output_root).map_err(|_| ImportError::WriteFailed)?;
    for (segment, output) in plan.segments.iter().zip(outputs.iter_mut()) {
        output.bytes.extend_from_slice(XHTML_SUFFIX);
        if output.bytes.len() as u64 > MAX_RESOURCE_BYTES {
            return Err(ImportError::ResourceTooLarge);
        }
        fs::write(staging.join(&segment.href), &output.bytes)
            .map_err(|_| ImportError::WriteFailed)?;
    }
    let sections = plan
        .segments
        .iter()
        .map(|segment| ManifestSection {
            id: segment.id.clone(),
            href: segment.href.clone(),
        })
        .collect();
    let resources = plan.images.iter().map(|image| image.path.clone()).collect();
    let cover_path = plan
        .cover_id
        .as_ref()
        .and_then(|id| plan.image_paths.get(id))
        .cloned();
    write_json(
        &staging.join(READER_MANIFEST),
        &ReaderManifest {
            schema: 1,
            content_version: content_version.to_owned(),
            sections,
            resources,
            toc: plan.toc.clone(),
        },
    )?;
    write_json(
        &staging.join(BOOK_METADATA),
        &BookMetadata {
            schema: 1,
            content_version: content_version.to_owned(),
            title: (!plan.title.is_empty()).then(|| plan.title.clone()),
            authors: plan.authors.clone(),
            cover_path,
        },
    )?;
    fs::write(
        staging.join(IMPORT_MARKER),
        format!("{IMPORT_MARKER_VERSION}\n{content_version}\n"),
    )
    .map_err(|_| ImportError::WriteFailed)
}

fn render_start(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    empty: bool,
    cursor: &mut XmlCursor,
    plan: &Plan,
    outputs: &mut [SegmentOutput],
    suppressed: &mut HashSet<usize>,
) -> Result<(), ImportError> {
    cursor.depth = cursor.depth.checked_add(1).ok_or(ImportError::InvalidXml)?;
    if suppressed
        .iter()
        .any(|void_depth| cursor.depth > *void_depth)
    {
        return Err(ImportError::InvalidXml);
    }
    let name = decode_name(event.local_name().as_ref())?;
    cursor.path.push(name.clone());
    if name == "body" && cursor.depth == 2 {
        cursor.body_index = Some(cursor.body_count);
        cursor.body_count += 1;
        cursor.body_depth = Some(cursor.depth);
        cursor.main_section_index = 0;
        cursor.main_section_depth = None;
    }
    if cursor.body_index == Some(0)
        && name == "section"
        && cursor.body_depth == Some(cursor.depth.saturating_sub(1))
    {
        cursor.main_section_index += 1;
        cursor.main_section_depth = Some(cursor.depth);
    }
    if !in_body(cursor) || name == "body" {
        return Ok(());
    }
    let key = current_key(cursor).ok_or(ImportError::UnsupportedFb2)?;
    let Some(index) = plan.segment_indexes.get(&key).copied() else {
        return Err(ImportError::UnsupportedFb2);
    };
    let mapped = mapped_element(&name).ok_or(ImportError::UnsupportedFb2)?;
    let is_void = matches!(name.as_str(), "image" | "empty-line");
    let mut output = BytesStart::new(mapped);
    output.push_attribute(("class", class_name(&name)));
    if let Some(id) = attribute(reader, event, "id")? {
        validate_id(&id)?;
        output.push_attribute(("id", id.as_str()));
    }
    if let Some(lang) = attribute(reader, event, "lang")?
        && valid_short_attribute(&lang)
    {
        output.push_attribute(("lang", lang.as_str()));
    }
    if let Some(title) = attribute(reader, event, "title")?
        && valid_short_attribute(&title)
    {
        output.push_attribute(("title", title.as_str()));
    }
    if name == "image" {
        let id = local_reference(reader, event)?;
        let path = plan
            .image_paths
            .get(&id)
            .ok_or(ImportError::InvalidReference)?;
        let relative = path
            .strip_prefix(&format!("{OUTPUT_ROOT}/"))
            .ok_or(ImportError::InvalidReference)?;
        output.push_attribute(("src", relative));
        if let Some(alt) = attribute(reader, event, "alt")?
            && valid_short_attribute(&alt)
        {
            output.push_attribute(("alt", alt.as_str()));
        }
    } else if name == "a" {
        let id = local_reference(reader, event)?;
        let href = plan
            .id_hrefs
            .get(&id)
            .ok_or(ImportError::InvalidReference)?;
        let current = &plan.segments[index].href;
        let local = href
            .strip_prefix(current)
            .filter(|suffix| suffix.starts_with('#'))
            .or_else(|| href.strip_prefix(&format!("{OUTPUT_ROOT}/")))
            .ok_or(ImportError::InvalidReference)?;
        output.push_attribute(("href", local));
        if attribute(reader, event, "type")?.as_deref() == Some("note") {
            output.push_attribute(("role", "doc-noteref"));
        }
    } else if matches!(name.as_str(), "th" | "td") {
        for attr in ["colspan", "rowspan"] {
            if let Some(value) = attribute(reader, event, attr)?
                && value
                    .parse::<u16>()
                    .is_ok_and(|value| (1..=100).contains(&value))
            {
                output.push_attribute((attr, value.as_str()));
            }
        }
    }
    let event = if empty || is_void {
        Event::Empty(output)
    } else {
        Event::Start(output)
    };
    write_output(&mut outputs[index], event)?;
    if is_void && !empty {
        suppressed.insert(cursor.depth);
    }
    Ok(())
}

fn render_end(
    name: &[u8],
    empty: bool,
    cursor: &mut XmlCursor,
    plan: &Plan,
    outputs: &mut [SegmentOutput],
    suppressed: &mut HashSet<usize>,
) -> Result<(), ImportError> {
    let name = decode_name(name)?;
    if cursor.path.last() != Some(&name) {
        return Err(ImportError::InvalidXml);
    }
    if in_body(cursor) && name != "body" && !empty {
        let key = current_key(cursor).ok_or(ImportError::UnsupportedFb2)?;
        let index = *plan
            .segment_indexes
            .get(&key)
            .ok_or(ImportError::UnsupportedFb2)?;
        if suppressed.remove(&cursor.depth) {
            // The matching void start was already emitted as an empty XHTML element.
        } else {
            let mapped = mapped_element(&name).ok_or(ImportError::UnsupportedFb2)?;
            write_output(&mut outputs[index], Event::End(BytesEnd::new(mapped)))?;
        }
    }
    if cursor.main_section_depth == Some(cursor.depth) && name == "section" {
        cursor.main_section_depth = None;
    }
    if cursor.body_depth == Some(cursor.depth) && name == "body" {
        cursor.body_index = None;
        cursor.body_depth = None;
        cursor.main_section_depth = None;
    }
    cursor.path.pop();
    cursor.depth = cursor.depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
    Ok(())
}

fn render_text(
    text: &str,
    cursor: &XmlCursor,
    plan: &Plan,
    outputs: &mut [SegmentOutput],
    suppressed: &HashSet<usize>,
) -> Result<(), ImportError> {
    if suppressed
        .iter()
        .any(|void_depth| cursor.depth >= *void_depth)
    {
        return if xml_space(text) {
            Ok(())
        } else {
            Err(ImportError::InvalidXml)
        };
    }
    if !in_body(cursor) || cursor.path.last().is_some_and(|name| name == "body") {
        return Ok(());
    }
    let key = current_key(cursor).ok_or(ImportError::UnsupportedFb2)?;
    let index = *plan
        .segment_indexes
        .get(&key)
        .ok_or(ImportError::UnsupportedFb2)?;
    write_output(&mut outputs[index], Event::Text(BytesText::new(text)))
}

fn write_output(output: &mut SegmentOutput, event: Event<'_>) -> Result<(), ImportError> {
    Writer::new(&mut output.bytes)
        .write_event(event)
        .map_err(|_| ImportError::WriteFailed)?;
    if output.bytes.len() as u64 > MAX_RESOURCE_BYTES {
        return Err(ImportError::ResourceTooLarge);
    }
    Ok(())
}

fn mapped_element(name: &str) -> Option<&'static str> {
    match name {
        "section" => Some("section"),
        "title" => Some("header"),
        "p" => Some("p"),
        "subtitle" => Some("h2"),
        "epigraph" | "cite" | "poem" => Some("blockquote"),
        "annotation" => Some("aside"),
        "image" => Some("img"),
        "a" => Some("a"),
        "strong" => Some("strong"),
        "emphasis" => Some("em"),
        "style" => Some("span"),
        "strikethrough" => Some("s"),
        "sub" => Some("sub"),
        "sup" => Some("sup"),
        "code" => Some("code"),
        "stanza" => Some("div"),
        "v" => Some("div"),
        "text-author" | "date" => Some("p"),
        "empty-line" => Some("br"),
        "table" => Some("table"),
        "tr" => Some("tr"),
        "th" => Some("th"),
        "td" => Some("td"),
        _ => None,
    }
}

fn class_name(name: &str) -> &'static str {
    match name {
        "section" => "section",
        "title" => "title",
        "p" => "paragraph",
        "subtitle" => "subtitle",
        "epigraph" => "epigraph",
        "cite" => "cite",
        "poem" => "poem",
        "annotation" => "annotation",
        "image" => "image",
        "a" => "link",
        "strong" => "strong",
        "emphasis" => "emphasis",
        "style" => "style",
        "strikethrough" => "strikethrough",
        "sub" => "sub",
        "sup" => "sup",
        "code" => "code",
        "stanza" => "stanza",
        "v" => "stanza-line",
        "text-author" => "text-author",
        "date" => "date",
        "empty-line" => "empty-line",
        "table" => "table",
        "tr" => "table-row",
        "th" => "table-header",
        "td" => "table-cell",
        _ => "fb2",
    }
}

fn renderable(name: &str) -> bool {
    mapped_element(name).is_some()
}

fn current_key(cursor: &XmlCursor) -> Option<SegmentKey> {
    match cursor.body_index? {
        0 => cursor
            .main_section_depth
            .map_or(Some(SegmentKey::MainPreamble), |_| {
                Some(SegmentKey::MainSection(cursor.main_section_index))
            }),
        index => Some(SegmentKey::AuxiliaryBody(index)),
    }
}

fn in_body(cursor: &XmlCursor) -> bool {
    cursor.body_index.is_some()
        && cursor
            .body_depth
            .is_some_and(|body_depth| cursor.depth >= body_depth)
}

fn local_reference(reader: &Reader<&[u8]>, event: &BytesStart<'_>) -> Result<String, ImportError> {
    let href = required_attribute(reader, event, "href")?;
    let id = href
        .strip_prefix('#')
        .filter(|id| !id.is_empty())
        .ok_or(ImportError::InvalidReference)?;
    validate_id(id)?;
    Ok(id.to_owned())
}

fn required_attribute(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    name: &str,
) -> Result<String, ImportError> {
    attribute(reader, event, name)?.ok_or(ImportError::InvalidXml)
}

fn attribute(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    name: &str,
) -> Result<Option<String>, ImportError> {
    let mut value = None;
    for attribute in event.attributes() {
        let attribute = attribute.map_err(|_| ImportError::InvalidXml)?;
        if attribute.key.local_name().as_ref() != name.as_bytes() {
            continue;
        }
        if value.is_some() {
            return Err(ImportError::InvalidXml);
        }
        value = Some(
            attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map_err(|_| ImportError::InvalidXml)?
                .into_owned(),
        );
    }
    Ok(value)
}

fn validate_id(value: &str) -> Result<(), ImportError> {
    if value.is_empty()
        || value.chars().count() > 256
        || value.chars().any(|character| character.is_control())
        || value.contains(['#', '/', '\\', '?', '%'])
    {
        return Err(ImportError::InvalidReference);
    }
    Ok(())
}

fn valid_short_attribute(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn xml_reference(value: &BytesRef<'_>) -> Result<String, ImportError> {
    let value = if let Some(value) = value
        .resolve_char_ref()
        .map_err(|_| ImportError::InvalidXml)?
    {
        value.to_string()
    } else {
        let name = value.decode().map_err(|_| ImportError::InvalidXml)?;
        quick_xml::escape::resolve_predefined_entity(&name)
            .map(str::to_owned)
            .ok_or(ImportError::InvalidXml)?
    };
    if value.chars().all(valid_xml_char) {
        Ok(value)
    } else {
        Err(ImportError::InvalidXml)
    }
}

fn valid_xml_char(value: char) -> bool {
    matches!(value, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn decode_name(name: &[u8]) -> Result<String, ImportError> {
    std::str::from_utf8(name)
        .map(str::to_owned)
        .map_err(|_| ImportError::InvalidXml)
}

fn path_ends(path: &[String], suffix: &[&str]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

fn push_bounded(target: &mut String, value: &str, max_chars: usize) -> Result<(), ImportError> {
    if target.chars().count().saturating_add(value.chars().count()) > max_chars {
        return Err(ImportError::UnsupportedFb2);
    }
    target.push_str(value);
    Ok(())
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn xml_space(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
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
        || metadata.authors.iter().any(String::is_empty)
        || metadata.cover_path.as_ref().is_some_and(|path| {
            !path.starts_with(&format!("{OUTPUT_ROOT}/images/"))
                || !matches!(
                    Path::new(path).extension().and_then(|value| value.to_str()),
                    Some("jpg" | "png")
                )
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
                "operation=import stage=publish-rename outcome=failed code=fb2-import-write-failed io_kind={:?}",
                error.kind()
            );
            Err(ImportError::WriteFailed)
        }
    }
}
