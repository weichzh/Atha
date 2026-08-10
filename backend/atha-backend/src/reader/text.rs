//! Bounded Markdown and TXT projection into the shared reader manifest.

use std::{
    error::Error,
    fmt, fs,
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
    thread,
    time::Duration,
};

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Decoder, DecoderResult, Encoding, UTF_8, UTF_16BE, UTF_16LE};
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{
    MAX_MANIFEST_SECTIONS, MAX_MANIFEST_TOC_ITEMS,
    epub::READER_MANIFEST,
    resources::MAX_RESOURCE_BYTES,
    source::{self, SourceDigest, SourceError},
};

const MARKDOWN_IDENTITY_DOMAIN: &[u8] = b"atha/markdown/importer-v1\0";
const TXT_IDENTITY_DOMAIN: &[u8] = b"atha/txt/importer-v1\0";
const TEXT_ROOT: &str = ".atha-text";
const IMPORT_MARKER: &str = ".atha-text-import";
const BOOK_METADATA: &str = ".atha-book.json";
const MARKDOWN_MARKER_VERSION: &str = "atha-markdown-import-v2";
const TXT_MARKER_VERSION: &str = "atha-txt-import-v1";
const MAX_METADATA_BYTES: u64 = 64 * 1024;
const MARKDOWN_STYLE: &str = concat!(
    "pre { white-space: pre-wrap; overflow-wrap: break-word; }\n",
    "pre, code { font-family: monospace; }\n",
    "table { border-collapse: collapse; }\n",
    "th, td { border: 1px solid currentColor; padding: 0.2em 0.5em; }\n",
    "blockquote { margin-inline: 1em; }\n",
);
const MAX_LABEL_UTF16_UNITS: usize = 256;
const MAX_CHAPTER_LINE_CHARS: usize = 80;
const TXT_SECTION_TARGET_BYTES: usize = 1024 * 1024;
const XHTML_FOOTER: &[u8] = b"</body></html>\n";

#[derive(Clone, Copy)]
enum TextFormat {
    Markdown,
    Txt,
}

impl TextFormat {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Txt => "txt",
        }
    }

    const fn write_failed(self) -> ImportError {
        match self {
            Self::Markdown => ImportError::MarkdownWriteFailed,
            Self::Txt => ImportError::TxtWriteFailed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ImportedBook {
    pub root: PathBuf,
    pub content_version: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub cover_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportError {
    InvalidMarkdownSource,
    MarkdownSourceTooLarge,
    InvalidMarkdownEncoding,
    TooManyMarkdownSections,
    TooManyMarkdownTocItems,
    MarkdownSectionTooLarge,
    MarkdownWriteFailed,
    MarkdownSourceChanged,
    InvalidTxtSource,
    TxtSourceTooLarge,
    InvalidTxtEncoding,
    TooManyTxtSections,
    TxtSectionTooLarge,
    TxtWriteFailed,
    TxtSourceChanged,
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
    sections: Vec<Section>,
    resources: Vec<String>,
    toc: Vec<TocItem>,
}

#[derive(Serialize)]
struct Section {
    id: String,
    href: String,
}

#[derive(Serialize)]
struct TocItem {
    label: String,
    href: String,
}

struct MarkdownBuildStats {
    input_bytes: u64,
    sections: usize,
    toc_items: usize,
    decode_ms: u128,
    parse_ms: u128,
    render_write_ms: u128,
}

struct TxtBuildStats {
    sections: usize,
    toc_items: usize,
    chapter_scan_ms: u128,
    render_write_ms: u128,
}

pub(super) fn import_markdown(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let started = std::time::Instant::now();
    let source = source.as_ref();
    let cache_root = cache_root.as_ref();
    fs::create_dir_all(cache_root).map_err(|_| ImportError::MarkdownWriteFailed)?;
    let fingerprint_started = std::time::Instant::now();
    let (content_version, mut source_file) =
        source::fingerprint(source, MARKDOWN_IDENTITY_DOMAIN, MAX_RESOURCE_BYTES)
            .map_err(markdown_source_error)?;
    let fingerprint_ms = fingerprint_started.elapsed().as_millis();
    let _import_guard = super::lock_import();
    let target = cache_root.join(&content_version);
    if complete_import(&target, MARKDOWN_MARKER_VERSION, &content_version) {
        return imported_book(
            target,
            content_version,
            MARKDOWN_MARKER_VERSION,
            TextFormat::Markdown,
        );
    }
    let staging = cache_root.join(format!(".{content_version}.staging-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|_| ImportError::MarkdownWriteFailed)?;
    }
    fs::create_dir(&staging).map_err(|_| ImportError::MarkdownWriteFailed)?;
    let result = build_markdown_import(
        &mut source_file,
        source,
        &staging,
        &content_version,
    )
    .and_then(|stats| {
        let publish_started = std::time::Instant::now();
        publish(
            &staging,
            &target,
            &content_version,
            MARKDOWN_MARKER_VERSION,
            TextFormat::Markdown,
        )?;
        let publish_ms = publish_started.elapsed().as_millis();
        log::info!(
            target: "atha::reader",
            "operation=import format=markdown outcome=success input_bytes={} sections={} toc_items={} fingerprint_ms={} decode_ms={} markdown_parse_ms={} render_write_ms={} publish_ms={} total_ms={}",
            stats.input_bytes,
            stats.sections,
            stats.toc_items,
            fingerprint_ms,
            stats.decode_ms,
            stats.parse_ms,
            stats.render_write_ms,
            publish_ms,
            started.elapsed().as_millis()
        );
        Ok(())
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    imported_book(
        target,
        content_version,
        MARKDOWN_MARKER_VERSION,
        TextFormat::Markdown,
    )
}

pub(super) fn import_txt(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let started = std::time::Instant::now();
    let source = source.as_ref();
    let cache_root = cache_root.as_ref();
    fs::create_dir_all(cache_root).map_err(|_| ImportError::TxtWriteFailed)?;
    let mut probe = TxtProbe::new();
    let detect_started = std::time::Instant::now();
    let (content_version, _) = source::fingerprint_with(
        source,
        TXT_IDENTITY_DOMAIN,
        super::MAX_SOURCE_BYTES,
        |bytes, last| probe.feed(bytes, last),
    )
    .map_err(txt_source_error)?;
    let input_bytes = probe.input_bytes;
    let encoding = probe.finish()?;
    let detect_ms = detect_started.elapsed().as_millis();
    let _import_guard = super::lock_import();
    let target = cache_root.join(&content_version);
    if complete_import(&target, TXT_MARKER_VERSION, &content_version) {
        return imported_book(target, content_version, TXT_MARKER_VERSION, TextFormat::Txt);
    }
    let staging = cache_root.join(format!(".{content_version}.staging-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|_| ImportError::TxtWriteFailed)?;
    }
    fs::create_dir(&staging).map_err(|_| ImportError::TxtWriteFailed)?;
    let result = build_txt_import(source, &staging, &content_version, encoding).and_then(
        |stats| {
            let publish_started = std::time::Instant::now();
            publish(
                &staging,
                &target,
                &content_version,
                TXT_MARKER_VERSION,
                TextFormat::Txt,
            )?;
            let publish_ms = publish_started.elapsed().as_millis();
            log::info!(
                target: "atha::reader",
                "operation=import format=txt outcome=success input_bytes={} encoding={} sections={} toc_items={} detect_ms={} chapter_scan_ms={} render_write_ms={} publish_ms={} total_ms={}",
                input_bytes,
                encoding.log_label,
                stats.sections,
                stats.toc_items,
                detect_ms,
                stats.chapter_scan_ms,
                stats.render_write_ms,
                publish_ms,
                started.elapsed().as_millis()
            );
            Ok(())
        },
    );
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    imported_book(target, content_version, TXT_MARKER_VERSION, TextFormat::Txt)
}

pub(super) fn markdown_source_identity(source: impl AsRef<Path>) -> Result<String, ImportError> {
    source::fingerprint(
        source.as_ref(),
        MARKDOWN_IDENTITY_DOMAIN,
        MAX_RESOURCE_BYTES,
    )
    .map(|(content_version, _)| content_version)
    .map_err(markdown_source_error)
}

pub(super) fn txt_source_identity(source: impl AsRef<Path>) -> Result<String, ImportError> {
    let mut probe = TxtProbe::new();
    let (content_version, _) = source::fingerprint_with(
        source.as_ref(),
        TXT_IDENTITY_DOMAIN,
        super::MAX_SOURCE_BYTES,
        |bytes, last| probe.feed(bytes, last),
    )
    .map_err(txt_source_error)?;
    probe.finish()?;
    Ok(content_version)
}

impl ImportError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidMarkdownSource => "invalid-markdown-source",
            Self::MarkdownSourceTooLarge => "markdown-source-too-large",
            Self::InvalidMarkdownEncoding => "invalid-markdown-encoding",
            Self::TooManyMarkdownSections => "too-many-markdown-sections",
            Self::TooManyMarkdownTocItems => "too-many-markdown-toc-items",
            Self::MarkdownSectionTooLarge => "markdown-section-too-large",
            Self::MarkdownWriteFailed => "markdown-import-write-failed",
            Self::MarkdownSourceChanged => "markdown-source-changed",
            Self::InvalidTxtSource => "invalid-txt-source",
            Self::TxtSourceTooLarge => "txt-source-too-large",
            Self::InvalidTxtEncoding => "invalid-txt-encoding",
            Self::TooManyTxtSections => "too-many-txt-sections",
            Self::TxtSectionTooLarge => "txt-section-too-large",
            Self::TxtWriteFailed => "txt-import-write-failed",
            Self::TxtSourceChanged => "txt-source-changed",
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for ImportError {}

#[derive(Clone, Copy)]
struct TxtEncoding {
    encoding: &'static Encoding,
    bom_bytes: u64,
    log_label: &'static str,
}

struct TxtProbe {
    detector: EncodingDetector,
    utf8_decoder: Decoder,
    utf8_valid: bool,
    scratch: String,
    prefix: Vec<u8>,
    saw_nul: bool,
    input_bytes: u64,
}

impl TxtProbe {
    fn new() -> Self {
        Self {
            detector: EncodingDetector::new(Iso2022JpDetection::Allow),
            utf8_decoder: UTF_8.new_decoder_without_bom_handling(),
            utf8_valid: true,
            scratch: String::new(),
            prefix: Vec::with_capacity(3),
            saw_nul: false,
            input_bytes: 0,
        }
    }

    fn feed(&mut self, bytes: &[u8], last: bool) {
        self.input_bytes = self.input_bytes.saturating_add(bytes.len() as u64);
        if self.prefix.len() < 3 {
            let remaining = 3 - self.prefix.len();
            self.prefix
                .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
        }
        self.saw_nul |= bytes.contains(&0);
        self.detector.feed(bytes, last);
        if !self.utf8_valid {
            return;
        }
        self.scratch.clear();
        let Some(capacity) = self
            .utf8_decoder
            .max_utf8_buffer_length_without_replacement(bytes.len())
        else {
            self.utf8_valid = false;
            return;
        };
        self.scratch.reserve(capacity);
        let (result, read) =
            self.utf8_decoder
                .decode_to_string_without_replacement(bytes, &mut self.scratch, last);
        self.utf8_valid = result == DecoderResult::InputEmpty && read == bytes.len();
    }

    fn finish(self) -> Result<TxtEncoding, ImportError> {
        if self.prefix.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return Ok(TxtEncoding {
                encoding: UTF_8,
                bom_bytes: 3,
                log_label: "utf-8-bom",
            });
        }
        if self.prefix.starts_with(&[0xFF, 0xFE]) {
            return Ok(TxtEncoding {
                encoding: UTF_16LE,
                bom_bytes: 2,
                log_label: "utf-16le-bom",
            });
        }
        if self.prefix.starts_with(&[0xFE, 0xFF]) {
            return Ok(TxtEncoding {
                encoding: UTF_16BE,
                bom_bytes: 2,
                log_label: "utf-16be-bom",
            });
        }
        if self.saw_nul {
            return Err(ImportError::InvalidTxtEncoding);
        }
        if self.utf8_valid {
            return Ok(TxtEncoding {
                encoding: UTF_8,
                bom_bytes: 0,
                log_label: "utf-8",
            });
        }
        let encoding = self.detector.guess(None, Utf8Detection::Deny);
        Ok(TxtEncoding {
            encoding,
            bom_bytes: 0,
            log_label: encoding.name(),
        })
    }
}

fn build_txt_import(
    source: &Path,
    staging: &Path,
    content_version: &str,
    encoding: TxtEncoding,
) -> Result<TxtBuildStats, ImportError> {
    let chapter_scan_started = std::time::Instant::now();
    let mut main_chapter_candidates = 0_usize;
    let mut count_digest = SourceDigest::new(TXT_IDENTITY_DOMAIN, super::MAX_SOURCE_BYTES);
    decode_txt_lines(source, encoding, Some(&mut count_digest), |line| {
        if matches!(chapter_line(line), Some((ChapterKind::Main, _))) {
            main_chapter_candidates = main_chapter_candidates.saturating_add(1);
        }
        Ok(())
    })?;
    if count_digest.finish() != content_version {
        return Err(ImportError::TxtSourceChanged);
    }
    let chapter_scan_ms = chapter_scan_started.elapsed().as_millis();
    if main_chapter_candidates > MAX_MANIFEST_TOC_ITEMS {
        return Err(ImportError::TooManyTxtSections);
    }

    let text_root = staging.join(TEXT_ROOT);
    fs::create_dir(&text_root).map_err(|_| ImportError::TxtWriteFailed)?;
    let render_started = std::time::Instant::now();
    let mut projection = TxtProjection::new(staging, main_chapter_candidates >= 2);
    let mut final_digest = SourceDigest::new(TXT_IDENTITY_DOMAIN, super::MAX_SOURCE_BYTES);
    decode_txt_lines(source, encoding, Some(&mut final_digest), |line| {
        projection.push_line(line)
    })?;
    let (sections, toc) = projection.finish()?;
    let section_count = sections.len();
    let toc_count = toc.len();
    let manifest = ReaderManifest {
        schema: 1,
        content_version: content_version.to_owned(),
        sections,
        resources: Vec::new(),
        toc,
    };
    write_json(
        &staging.join(READER_MANIFEST),
        &manifest,
        ImportError::TxtWriteFailed,
    )?;
    write_json(
        &staging.join(BOOK_METADATA),
        &BookMetadata {
            schema: 1,
            content_version: content_version.to_owned(),
            title: None,
            authors: Vec::new(),
            cover_path: None,
        },
        ImportError::TxtWriteFailed,
    )?;
    write_bytes(
        &staging.join(IMPORT_MARKER),
        marker(TXT_MARKER_VERSION, content_version).as_bytes(),
        ImportError::TxtWriteFailed,
    )?;
    if final_digest.finish() != content_version {
        return Err(ImportError::TxtSourceChanged);
    }
    Ok(TxtBuildStats {
        sections: section_count,
        toc_items: toc_count,
        chapter_scan_ms,
        render_write_ms: render_started.elapsed().as_millis(),
    })
}

fn decode_txt_lines(
    source: &Path,
    encoding: TxtEncoding,
    mut digest: Option<&mut SourceDigest>,
    mut receive: impl FnMut(&str) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    let file = File::open(source).map_err(|_| ImportError::InvalidTxtSource)?;
    let mut reader = BufReader::new(file);
    let mut bom = [0_u8; 3];
    if encoding.bom_bytes != 0 {
        let bom_length =
            usize::try_from(encoding.bom_bytes).map_err(|_| ImportError::InvalidTxtSource)?;
        reader
            .read_exact(&mut bom[..bom_length])
            .map_err(|_| ImportError::InvalidTxtEncoding)?;
        if let Some(digest) = digest.as_deref_mut() {
            digest
                .update(&bom[..bom_length])
                .map_err(txt_source_error)?;
        }
    }
    let mut decoder = encoding.encoding.new_decoder_without_bom_handling();
    let mut splitter = LineSplitter::new();
    let mut input = [0_u8; 64 * 1024];
    let mut output = String::new();
    let mut total = encoding.bom_bytes;
    loop {
        let read = reader
            .read(&mut input)
            .map_err(|_| ImportError::InvalidTxtSource)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ImportError::TxtSourceTooLarge)?;
        if total > super::MAX_SOURCE_BYTES {
            return Err(ImportError::TxtSourceTooLarge);
        }
        if let Some(digest) = digest.as_deref_mut() {
            digest.update(&input[..read]).map_err(txt_source_error)?;
        }
        decode_txt_chunk(
            &mut decoder,
            &input[..read],
            false,
            &mut output,
            &mut splitter,
            &mut receive,
        )?;
    }
    decode_txt_chunk(
        &mut decoder,
        &[],
        true,
        &mut output,
        &mut splitter,
        &mut receive,
    )?;
    splitter.finish(&mut receive)
}

fn decode_txt_chunk(
    decoder: &mut Decoder,
    input: &[u8],
    last: bool,
    output: &mut String,
    splitter: &mut LineSplitter,
    receive: &mut impl FnMut(&str) -> Result<(), ImportError>,
) -> Result<(), ImportError> {
    output.clear();
    let capacity = decoder
        .max_utf8_buffer_length_without_replacement(input.len())
        .ok_or(ImportError::TxtSourceTooLarge)?;
    output.reserve(capacity);
    let (result, read) = decoder.decode_to_string_without_replacement(input, output, last);
    if result != DecoderResult::InputEmpty || read != input.len() {
        return Err(ImportError::InvalidTxtEncoding);
    }
    splitter.feed(output, receive)
}

struct LineSplitter {
    current: String,
    pending_cr: bool,
}

impl LineSplitter {
    fn new() -> Self {
        Self {
            current: String::new(),
            pending_cr: false,
        }
    }

    fn feed(
        &mut self,
        value: &str,
        receive: &mut impl FnMut(&str) -> Result<(), ImportError>,
    ) -> Result<(), ImportError> {
        for character in value.chars() {
            if self.pending_cr {
                receive(&self.current)?;
                self.current.clear();
                self.pending_cr = false;
                if character == '\n' {
                    continue;
                }
            }
            match character {
                '\r' => self.pending_cr = true,
                '\n' => {
                    receive(&self.current)?;
                    self.current.clear();
                }
                _ => {
                    if self.current.len().saturating_add(character.len_utf8())
                        > MAX_RESOURCE_BYTES as usize
                    {
                        return Err(ImportError::TxtSectionTooLarge);
                    }
                    self.current.push(character);
                }
            }
        }
        Ok(())
    }

    fn finish(
        &mut self,
        receive: &mut impl FnMut(&str) -> Result<(), ImportError>,
    ) -> Result<(), ImportError> {
        if self.pending_cr || !self.current.is_empty() {
            receive(&self.current)?;
            self.current.clear();
            self.pending_cr = false;
        }
        Ok(())
    }
}

struct TxtProjection<'a> {
    staging: &'a Path,
    chapter_mode: bool,
    current: Option<TxtSectionWriter>,
    sections: Vec<Section>,
    toc: Vec<TocItem>,
    split_allowed: bool,
    fragment_sequence: usize,
    chapter_sequence: usize,
}

impl<'a> TxtProjection<'a> {
    fn new(staging: &'a Path, chapter_mode: bool) -> Self {
        Self {
            staging,
            chapter_mode,
            current: None,
            sections: Vec::new(),
            toc: Vec::new(),
            split_allowed: false,
            fragment_sequence: 0,
            chapter_sequence: 0,
        }
    }

    fn push_line(&mut self, line: &str) -> Result<(), ImportError> {
        let line = line.trim();
        if line.is_empty() {
            self.split_allowed = true;
            return Ok(());
        }
        if self.chapter_mode {
            if let Some((_, label)) = chapter_line(line) {
                let start_group = self.chapter_sequence == 0
                    || self
                        .current
                        .as_ref()
                        .is_some_and(|section| section.written >= TXT_SECTION_TARGET_BYTES);
                if start_group {
                    self.finish_current()?;
                    self.start_section(&label, None)?;
                }
                if self.toc.len() >= MAX_MANIFEST_TOC_ITEMS {
                    return Err(ImportError::TooManyTxtSections);
                }
                self.chapter_sequence = self.chapter_sequence.saturating_add(1);
                let fragment = format!("chapter-{:04}", self.chapter_sequence);
                let section = self.current.as_mut().ok_or(ImportError::TxtWriteFailed)?;
                section.write_heading(&fragment, &label)?;
                self.toc.push(TocItem {
                    label,
                    href: format!("{}#{fragment}", section.href),
                });
                self.split_allowed = false;
                return Ok(());
            }
            if self.current.is_none() {
                self.start_section("前言", None)?;
            }
            self.write_paragraph(line, false)?;
        } else {
            if self.current.is_none() {
                self.start_fragment()?;
            }
            self.write_paragraph(line, true)?;
        }
        self.split_allowed = false;
        Ok(())
    }

    fn write_paragraph(&mut self, value: &str, allow_split: bool) -> Result<(), ImportError> {
        let required = escaped_xml_len(value)
            .and_then(|length| length.checked_add(b"<p></p>\n".len()))
            .ok_or(ImportError::TxtSectionTooLarge)?;
        let fits = self
            .current
            .as_ref()
            .is_some_and(|section| section.can_fit(required));
        if !fits && allow_split && self.split_allowed {
            self.finish_current()?;
            self.start_fragment()?;
        }
        let section = self.current.as_mut().ok_or(ImportError::TxtWriteFailed)?;
        if !section.can_fit(required) {
            return Err(ImportError::TxtSectionTooLarge);
        }
        section.write_text_element("p", value)
    }

    fn start_fragment(&mut self) -> Result<(), ImportError> {
        self.fragment_sequence = self.fragment_sequence.saturating_add(1);
        let label = format!("正文片段 {}", self.fragment_sequence);
        self.start_section(&label, Some(label.clone()))
    }

    fn start_section(&mut self, title: &str, toc_label: Option<String>) -> Result<(), ImportError> {
        if self.current.is_some() {
            return Err(ImportError::TxtWriteFailed);
        }
        let index = self.sections.len().saturating_add(1);
        if index > MAX_MANIFEST_SECTIONS {
            return Err(ImportError::TooManyTxtSections);
        }
        let id = format!("section-{index:04}");
        let href = format!("{TEXT_ROOT}/{id}.xhtml");
        let writer = TxtSectionWriter::new(self.staging.join(&href), id, href.clone(), title)?;
        if let Some(label) = toc_label {
            if self.toc.len() >= MAX_MANIFEST_TOC_ITEMS {
                return Err(ImportError::TooManyTxtSections);
            }
            self.toc.push(TocItem { label, href });
        }
        self.current = Some(writer);
        Ok(())
    }

    fn finish_current(&mut self) -> Result<(), ImportError> {
        if let Some(current) = self.current.take() {
            self.sections.push(current.finish()?);
        }
        Ok(())
    }

    fn finish(mut self) -> Result<(Vec<Section>, Vec<TocItem>), ImportError> {
        if self.current.is_none() && self.sections.is_empty() {
            if self.chapter_mode {
                self.start_section("正文", None)?;
            } else {
                self.start_fragment()?;
            }
        }
        self.finish_current()?;
        Ok((self.sections, self.toc))
    }
}

struct TxtSectionWriter {
    file: File,
    id: String,
    href: String,
    written: usize,
}

impl TxtSectionWriter {
    fn new(path: PathBuf, id: String, href: String, title: &str) -> Result<Self, ImportError> {
        let mut header = String::new();
        header.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        header.push_str(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><meta charset=\"utf-8\" /><title>",
        );
        push_escaped(&mut header, title);
        header.push_str("</title></head><body class=\"atha-text atha-txt\">");
        let mut file = File::create(path).map_err(|_| ImportError::TxtWriteFailed)?;
        file.write_all(header.as_bytes())
            .map_err(|_| ImportError::TxtWriteFailed)?;
        Ok(Self {
            file,
            id,
            href,
            written: header.len(),
        })
    }

    fn can_fit(&self, additional: usize) -> bool {
        self.written
            .checked_add(additional)
            .and_then(|value| value.checked_add(XHTML_FOOTER.len()))
            .is_some_and(|value| value as u64 <= MAX_RESOURCE_BYTES)
    }

    fn write_text_element(&mut self, element: &str, value: &str) -> Result<(), ImportError> {
        let mut markup = String::new();
        markup.push('<');
        markup.push_str(element);
        markup.push('>');
        push_escaped(&mut markup, value);
        markup.push_str("</");
        markup.push_str(element);
        markup.push_str(">\n");
        self.write_markup(&markup)
    }

    fn write_heading(&mut self, id: &str, value: &str) -> Result<(), ImportError> {
        let mut markup = format!("<h1 id=\"{id}\">");
        push_escaped(&mut markup, value);
        markup.push_str("</h1>\n");
        self.write_markup(&markup)
    }

    fn write_markup(&mut self, markup: &str) -> Result<(), ImportError> {
        if !self.can_fit(markup.len()) {
            return Err(ImportError::TxtSectionTooLarge);
        }
        self.file
            .write_all(markup.as_bytes())
            .map_err(|_| ImportError::TxtWriteFailed)?;
        self.written = self.written.saturating_add(markup.len());
        Ok(())
    }

    fn finish(mut self) -> Result<Section, ImportError> {
        if !self.can_fit(0) {
            return Err(ImportError::TxtSectionTooLarge);
        }
        self.file
            .write_all(XHTML_FOOTER)
            .and_then(|()| self.file.sync_all())
            .map_err(|_| ImportError::TxtWriteFailed)?;
        Ok(Section {
            id: self.id,
            href: self.href,
        })
    }
}

#[derive(Clone, Copy)]
enum ChapterKind {
    Main,
    Special,
}

fn chapter_line(line: &str) -> Option<(ChapterKind, String)> {
    let line = line.trim();
    if line.is_empty() || line.chars().count() > MAX_CHAPTER_LINE_CHARS {
        return None;
    }
    static MAIN_CHAPTER: OnceLock<Regex> = OnceLock::new();
    let main_chapter = MAIN_CHAPTER.get_or_init(|| {
        Regex::new(r"^第[0-9０-９零〇○一二三四五六七八九十百千万两]+[章节回篇].{0,64}$")
            .expect("main chapter regex is valid")
    });
    if main_chapter.is_match(line) {
        return Some((ChapterKind::Main, normalize_label(line)));
    }
    static SPECIAL_CHAPTER: OnceLock<Regex> = OnceLock::new();
    let special_chapter = SPECIAL_CHAPTER.get_or_init(|| {
        Regex::new(r"^(?:序章|楔子|引子|前言|后记|尾声|终章|番外.{0,64})$")
            .expect("special chapter regex is valid")
    });
    special_chapter
        .is_match(line)
        .then(|| (ChapterKind::Special, normalize_label(line)))
}

fn escaped_xml_len(value: &str) -> Option<usize> {
    value.chars().try_fold(0_usize, |length, character| {
        let bytes = match character {
            '&' => 5,
            '<' | '>' => 4,
            '"' | '\'' => 6,
            character if character.is_control() && !character.is_whitespace() => 0,
            character if character.is_control() => 1,
            character if !valid_xml_character(character) => 0,
            character => character.len_utf8(),
        };
        length.checked_add(bytes)
    })
}

fn txt_source_error(error: SourceError) -> ImportError {
    match error {
        SourceError::InvalidSource => ImportError::InvalidTxtSource,
        SourceError::SourceTooLarge => ImportError::TxtSourceTooLarge,
    }
}

fn build_markdown_import(
    source_file: &mut File,
    source: &Path,
    staging: &Path,
    content_version: &str,
) -> Result<MarkdownBuildStats, ImportError> {
    let decode_started = std::time::Instant::now();
    let mut bytes = Vec::new();
    source_file
        .take(MAX_RESOURCE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ImportError::InvalidMarkdownSource)?;
    if bytes.len() as u64 > MAX_RESOURCE_BYTES {
        return Err(ImportError::MarkdownSourceTooLarge);
    }
    if bytes.contains(&0) {
        return Err(ImportError::InvalidMarkdownEncoding);
    }
    let markdown = std::str::from_utf8(&bytes).map_err(|_| ImportError::InvalidMarkdownEncoding)?;
    if markdown
        .chars()
        .any(|character| !valid_xml_character(character))
    {
        return Err(ImportError::InvalidMarkdownEncoding);
    }
    let decode_ms = decode_started.elapsed().as_millis();
    let parse_started = std::time::Instant::now();
    let groups = markdown_groups(markdown);
    let parse_ms = parse_started.elapsed().as_millis();
    if groups.len() > MAX_MANIFEST_SECTIONS {
        return Err(ImportError::TooManyMarkdownSections);
    }

    let render_started = std::time::Instant::now();
    let text_root = staging.join(TEXT_ROOT);
    fs::create_dir(&text_root).map_err(|_| ImportError::MarkdownWriteFailed)?;
    let mut sections = Vec::with_capacity(groups.len());
    let mut toc = Vec::new();
    let mut first_h1 = None;
    let mut heading_sequence = 0_usize;
    for (index, events) in groups.into_iter().enumerate() {
        let id = format!("section-{:04}", index + 1);
        let href = format!("{TEXT_ROOT}/{id}.xhtml");
        let (xhtml, mut items, title) =
            render_markdown_section(events, &href, &mut heading_sequence)?;
        if first_h1.is_none() {
            first_h1 = title;
        }
        if toc.len().saturating_add(items.len()) > MAX_MANIFEST_TOC_ITEMS {
            return Err(ImportError::TooManyMarkdownTocItems);
        }
        toc.append(&mut items);
        write_bytes(
            &staging.join(&href),
            xhtml.as_bytes(),
            ImportError::MarkdownWriteFailed,
        )?;
        sections.push(Section { id, href });
    }
    let manifest = ReaderManifest {
        schema: 1,
        content_version: content_version.to_owned(),
        sections,
        resources: Vec::new(),
        toc,
    };
    write_json(
        &staging.join(READER_MANIFEST),
        &manifest,
        ImportError::MarkdownWriteFailed,
    )?;
    write_json(
        &staging.join(BOOK_METADATA),
        &BookMetadata {
            schema: 1,
            content_version: content_version.to_owned(),
            title: first_h1,
            authors: Vec::new(),
            cover_path: None,
        },
        ImportError::MarkdownWriteFailed,
    )?;
    write_bytes(
        &staging.join(IMPORT_MARKER),
        marker(MARKDOWN_MARKER_VERSION, content_version).as_bytes(),
        ImportError::MarkdownWriteFailed,
    )?;
    if source::hash_file(source, MARKDOWN_IDENTITY_DOMAIN, MAX_RESOURCE_BYTES)
        .map_err(markdown_source_error)?
        != content_version
    {
        return Err(ImportError::MarkdownSourceChanged);
    }
    Ok(MarkdownBuildStats {
        input_bytes: bytes.len() as u64,
        sections: manifest.sections.len(),
        toc_items: manifest.toc.len(),
        decode_ms,
        parse_ms,
        render_write_ms: render_started.elapsed().as_millis(),
    })
}

fn markdown_groups(markdown: &str) -> Vec<Vec<Event<'static>>> {
    let mut options = Options::empty();
    options.insert(
        Options::ENABLE_GFM
            | Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    );
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0_usize;
    let mut metadata_depth = 0_usize;
    let mut saw_h1 = false;
    for event in Parser::new_ext(markdown, options) {
        if matches!(event, Event::Start(Tag::MetadataBlock(_))) {
            metadata_depth += 1;
            continue;
        }
        if matches!(event, Event::End(TagEnd::MetadataBlock(_))) {
            metadata_depth = metadata_depth.saturating_sub(1);
            continue;
        }
        if metadata_depth != 0 {
            continue;
        }
        let starts_top_h1 = matches!(
            &event,
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) if depth == 0
        );
        if starts_top_h1 {
            if saw_h1 || visible_events(&current) {
                groups.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            saw_h1 = true;
        }
        match event {
            Event::Start(_) => depth = depth.saturating_add(1),
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        current.push(event.into_static());
    }
    if saw_h1 || visible_events(&current) || groups.is_empty() {
        groups.push(current);
    }
    groups
}

fn visible_events(events: &[Event<'_>]) -> bool {
    events.iter().any(|event| match event {
        Event::Text(value)
        | Event::Code(value)
        | Event::Html(value)
        | Event::InlineHtml(value)
        | Event::InlineMath(value)
        | Event::DisplayMath(value) => !value.trim().is_empty(),
        Event::Rule | Event::TaskListMarker(_) | Event::FootnoteReference(_) => true,
        _ => false,
    })
}

fn render_markdown_section(
    events: Vec<Event<'static>>,
    href: &str,
    heading_sequence: &mut usize,
) -> Result<(String, Vec<TocItem>, Option<String>), ImportError> {
    let mut rendered = Vec::with_capacity(events.len());
    let mut toc = Vec::new();
    let mut heading: Option<(HeadingLevel, String, String)> = None;
    let mut first_h1 = None;
    for event in events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                *heading_sequence = heading_sequence.saturating_add(1);
                let anchor = format!("heading-{heading_sequence:04}");
                heading = Some((level, anchor.clone(), String::new()));
                rendered.push(Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::from(anchor)),
                    classes: Vec::new(),
                    attrs: Vec::new(),
                }));
            }
            Event::End(TagEnd::Heading(level)) => {
                rendered.push(Event::End(TagEnd::Heading(level)));
                if let Some((captured_level, anchor, label)) = heading.take() {
                    let label = normalize_label(&label);
                    if !label.is_empty() {
                        if captured_level == HeadingLevel::H1 && first_h1.is_none() {
                            first_h1 = Some(label.clone());
                        }
                        toc.push(TocItem {
                            label,
                            href: if captured_level == HeadingLevel::H1 {
                                href.to_owned()
                            } else {
                                format!("{href}#{anchor}")
                            },
                        });
                    }
                }
            }
            Event::Start(Tag::Link { .. } | Tag::Image { .. })
            | Event::End(TagEnd::Link | TagEnd::Image) => {}
            Event::Html(value) | Event::InlineHtml(value) => {
                append_heading_text(&mut heading, &value);
                rendered.push(Event::Text(value));
            }
            Event::FootnoteReference(value) => {
                let value = CowStr::from(format!("[{value}]"));
                append_heading_text(&mut heading, &value);
                rendered.push(Event::Text(value));
            }
            Event::TaskListMarker(checked) => {
                let value = CowStr::from(if checked { "[x] " } else { "[ ] " });
                append_heading_text(&mut heading, &value);
                rendered.push(Event::Text(value));
            }
            Event::Text(value) => {
                append_heading_text(&mut heading, &value);
                rendered.push(Event::Text(value));
            }
            Event::Code(value) => {
                append_heading_text(&mut heading, &value);
                rendered.push(Event::Code(value));
            }
            Event::InlineMath(value) => {
                append_heading_text(&mut heading, &value);
                rendered.push(Event::InlineMath(value));
            }
            Event::DisplayMath(value) => {
                append_heading_text(&mut heading, &value);
                rendered.push(Event::DisplayMath(value));
            }
            Event::SoftBreak | Event::HardBreak => {
                append_heading_text(&mut heading, " ");
                rendered.push(event);
            }
            other => rendered.push(other),
        }
    }
    let mut body = String::new();
    html::push_html(&mut body, rendered.into_iter());
    let title = first_h1.clone().unwrap_or_else(|| "正文".to_owned());
    let mut xhtml = String::with_capacity(body.len().saturating_add(256));
    xhtml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xhtml.push_str(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><meta charset=\"utf-8\" /><title>",
    );
    push_escaped(&mut xhtml, &title);
    xhtml.push_str("</title><style>");
    xhtml.push_str(MARKDOWN_STYLE);
    xhtml.push_str("</style></head><body class=\"atha-text atha-markdown\">");
    xhtml.push_str(&body);
    xhtml.push_str("</body></html>\n");
    if xhtml.len() as u64 > MAX_RESOURCE_BYTES {
        return Err(ImportError::MarkdownSectionTooLarge);
    }
    Ok((xhtml, toc, first_h1))
}

fn append_heading_text(heading: &mut Option<(HeadingLevel, String, String)>, value: &str) {
    if let Some((_, _, label)) = heading {
        label.push_str(value);
    }
}

fn normalize_label(value: &str) -> String {
    let normalized = value
        .chars()
        .filter_map(|character| {
            if character.is_whitespace() {
                Some(' ')
            } else if character.is_control() || !valid_xml_character(character) {
                None
            } else {
                Some(character)
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut utf16_units = 0_usize;
    normalized
        .chars()
        .take_while(|character| {
            let next = utf16_units.saturating_add(character.len_utf16());
            if next > MAX_LABEL_UTF16_UNITS {
                return false;
            }
            utf16_units = next;
            true
        })
        .collect()
}

fn push_escaped(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            character if character.is_control() && character.is_whitespace() => output.push(' '),
            character if character.is_control() => {}
            character if !valid_xml_character(character) => {}
            _ => output.push(character),
        }
    }
}

fn valid_xml_character(character: char) -> bool {
    matches!(
        character,
        '\u{9}'
            | '\u{A}'
            | '\u{D}'
            | '\u{20}'..='\u{D7FF}'
            | '\u{E000}'..='\u{FFFD}'
            | '\u{10000}'..='\u{10FFFF}'
    )
}

fn markdown_source_error(error: SourceError) -> ImportError {
    match error {
        SourceError::InvalidSource => ImportError::InvalidMarkdownSource,
        SourceError::SourceTooLarge => ImportError::MarkdownSourceTooLarge,
    }
}

fn marker(version: &str, content_version: &str) -> String {
    format!("{version}\n{content_version}\n")
}

fn complete_import(path: &Path, version: &str, content_version: &str) -> bool {
    has_marker(path, version, content_version)
        && read_metadata(path, content_version).is_some()
        && super::resources::complete_reader_cache(path, content_version)
}

fn has_marker(path: &Path, version: &str, content_version: &str) -> bool {
    fs::read_to_string(path.join(IMPORT_MARKER))
        .is_ok_and(|value| value == marker(version, content_version))
}

pub(super) fn has_cache_marker(path: &Path, content_version: &str) -> bool {
    has_marker(path, MARKDOWN_MARKER_VERSION, content_version)
        || has_marker(path, TXT_MARKER_VERSION, content_version)
}

pub(super) fn complete_cache(path: &Path, content_version: &str, extension: &str) -> bool {
    match extension {
        "md" | "markdown" => complete_import(path, MARKDOWN_MARKER_VERSION, content_version),
        "txt" => complete_import(path, TXT_MARKER_VERSION, content_version),
        _ => false,
    }
}

fn imported_book(
    root: PathBuf,
    content_version: String,
    marker_version: &str,
    format: TextFormat,
) -> Result<ImportedBook, ImportError> {
    let write_failed = format.write_failed();
    if !has_marker(&root, marker_version, &content_version) {
        return Err(write_failed);
    }
    let metadata = read_metadata(&root, &content_version).ok_or(write_failed)?;
    if !super::resources::complete_reader_cache(&root, &content_version) {
        return Err(write_failed);
    }
    Ok(ImportedBook {
        root,
        content_version,
        title: metadata.title,
        authors: metadata.authors,
        cover_path: metadata.cover_path,
    })
}

fn read_metadata(path: &Path, content_version: &str) -> Option<BookMetadata> {
    let path = path.join(BOOK_METADATA);
    if path.metadata().ok()?.len() > MAX_METADATA_BYTES {
        return None;
    }
    let metadata: BookMetadata = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if metadata.schema != 1
        || metadata.content_version != content_version
        || metadata.title.as_ref().is_some_and(|title| {
            title.is_empty() || title.encode_utf16().count() > MAX_LABEL_UTF16_UNITS
        })
        || !metadata.authors.is_empty()
        || metadata.cover_path.is_some()
    {
        return None;
    }
    Some(metadata)
}

fn write_json(path: &Path, value: &impl Serialize, error: ImportError) -> Result<(), ImportError> {
    let mut file = File::create(path).map_err(|_| error)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(|_| error)?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|_| error)
}

fn write_bytes(path: &Path, bytes: &[u8], error: ImportError) -> Result<(), ImportError> {
    let mut file = File::create(path).map_err(|_| error)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| error)
}

fn publish(
    staging: &Path,
    target: &Path,
    content_version: &str,
    marker_version: &str,
    format: TextFormat,
) -> Result<(), ImportError> {
    let error = format.write_failed();
    if complete_import(target, marker_version, content_version) {
        fs::remove_dir_all(staging).map_err(|_| error)?;
        return Ok(());
    }
    if target.exists() {
        fs::remove_dir_all(target).map_err(|_| error)?;
    }
    let mut renamed = fs::rename(staging, target);
    for _ in 0..4 {
        if !matches!(&renamed, Err(io_error) if io_error.kind() == std::io::ErrorKind::PermissionDenied)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
        renamed = fs::rename(staging, target);
    }
    match renamed {
        Ok(()) => Ok(()),
        Err(_) if complete_import(target, marker_version, content_version) => {
            fs::remove_dir_all(staging).map_err(|_| error)
        }
        Err(io_error) => {
            log::warn!(
                target: "atha::reader",
                "operation=import format={} stage=publish-rename outcome=failed code={} io_kind={:?}",
                format.as_str(),
                error.code(),
                io_error.kind()
            );
            Err(error)
        }
    }
}
