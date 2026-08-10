//! Import one constrained CBZ archive into the reader manifest contract.

use std::{
    cmp::Ordering,
    error::Error,
    fmt, fs,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use imagesize::ImageType;
use quick_xml::{
    Decoder, NsReader, XmlVersion,
    events::{BytesStart, Event},
};
use serde::{Deserialize, Serialize};

use super::{
    archive::{self, ArchiveError},
    epub::READER_MANIFEST,
};

const IMPORT_MARKER: &str = ".atha-cbz-import";
const BOOK_METADATA: &str = ".atha-book.json";
const OUTPUT_ROOT: &str = ".atha-cbz";
const MAX_PAGES: usize = 1_000;
const MAX_IMAGE_SIDE: usize = 8_192;
const MAX_IMAGE_PIXELS: usize = 20_000_000;
const MAX_COMIC_INFO_BYTES: u64 = 1024 * 1024;
const MAX_COMIC_INFO_DEPTH: usize = 64;
const MAX_COMIC_INFO_TEXT_BYTES: usize = 512;

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
    UnsupportedCbz,
    InvalidImage,
    TooManyPages,
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

#[derive(Default)]
struct ComicInfo {
    title: Option<String>,
    writer: Option<String>,
    cover_index: Option<usize>,
}

#[derive(Clone, Copy)]
enum ComicInfoField {
    Title,
    Writer,
}

#[derive(Clone, Copy)]
enum PageKind {
    Jpeg,
    Png,
}

impl PageKind {
    const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }

    const fn image_type(self) -> ImageType {
        match self {
            Self::Jpeg => ImageType::Jpeg,
            Self::Png => ImageType::Png,
        }
    }
}

impl ImportError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidSource => "invalid-cbz-source",
            Self::SourceTooLarge => "cbz-source-too-large",
            Self::InvalidArchive => "invalid-cbz-archive",
            Self::ArchiveTooLarge => "cbz-archive-too-large",
            Self::UnsafePath => "unsafe-cbz-path",
            Self::Encrypted => "encrypted-cbz",
            Self::UnsupportedCbz => "unsupported-cbz",
            Self::InvalidImage => "invalid-cbz-image",
            Self::TooManyPages => "too-many-cbz-pages",
            Self::WriteFailed => "cbz-import-write-failed",
            Self::SourceChanged => "cbz-source-changed",
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

pub fn import_cbz(
    source: impl AsRef<Path>,
    cache_root: impl AsRef<Path>,
) -> Result<ImportedBook, ImportError> {
    let (source, content_version, source_file) = fingerprinted_source(source.as_ref())?;
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
    if metadata.len() > archive::MAX_SOURCE_BYTES {
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
    let mut archive = archive::open_fingerprinted(source_file)?;
    let index = archive::inspect(&mut archive)?;
    if index.contains("mimetype") || index.contains("META-INF/container.xml") {
        return Err(ImportError::UnsupportedCbz);
    }
    let mut pages = index
        .paths()
        .filter(|path| !ignored(path))
        .filter_map(|path| page_kind(path).map(|kind| (path.to_owned(), kind)))
        .collect::<Vec<_>>();
    if pages.is_empty() {
        return Err(ImportError::UnsupportedCbz);
    }
    if pages.len() > MAX_PAGES {
        return Err(ImportError::TooManyPages);
    }
    pages.sort_by(|left, right| natural_cmp(&left.0, &right.0));
    let mut extracted = 0_u64;
    let comic_info =
        read_comic_info(&mut archive, &index, pages.len(), &mut extracted)?.unwrap_or_default();

    let image_root = staging.join(OUTPUT_ROOT).join("images");
    fs::create_dir_all(&image_root).map_err(|_| ImportError::WriteFailed)?;
    let mut sections = Vec::with_capacity(pages.len());
    let mut resources = Vec::with_capacity(pages.len());
    let mut toc = Vec::with_capacity(pages.len());
    for (offset, (source_path, kind)) in pages.iter().enumerate() {
        let page = offset + 1;
        let bytes = archive::read(&mut archive, &index, source_path)?;
        archive::add_extracted(
            &mut extracted,
            u64::try_from(bytes.len()).map_err(|_| ImportError::ArchiveTooLarge)?,
        )?;
        validate_image(&bytes, *kind)?;
        let image_name = format!("page-{page:04}.{}", kind.extension());
        let image_path = format!("{OUTPUT_ROOT}/images/{image_name}");
        fs::write(staging.join(&image_path), bytes).map_err(|_| ImportError::WriteFailed)?;
        let id = format!("page-{page:04}");
        let href = format!("{OUTPUT_ROOT}/{id}.xhtml");
        fs::write(staging.join(&href), page_xhtml(page, &image_name))
            .map_err(|_| ImportError::WriteFailed)?;
        sections.push(Section {
            id,
            href: href.clone(),
        });
        resources.push(image_path);
        toc.push(TocItem {
            label: source_path.clone(),
            href,
        });
    }

    let cover_path = comic_info
        .cover_index
        .and_then(|index| resources.get(index))
        .cloned()
        .or_else(|| resources.first().cloned());
    write_json(
        &staging.join(READER_MANIFEST),
        &ReaderManifest {
            schema: 1,
            content_version: content_version.to_owned(),
            sections,
            resources,
            toc,
        },
    )?;
    write_json(
        &staging.join(BOOK_METADATA),
        &BookMetadata {
            schema: 1,
            content_version: content_version.to_owned(),
            title: comic_info.title,
            authors: comic_info.writer.into_iter().collect(),
            cover_path,
        },
    )?;
    fs::write(
        staging.join(IMPORT_MARKER),
        format!("atha-cbz-import-v1\n{content_version}\n"),
    )
    .map_err(|_| ImportError::WriteFailed)?;
    if archive::hash_file(source_path)? != content_version {
        return Err(ImportError::SourceChanged);
    }
    Ok(())
}

fn read_comic_info(
    archive: &mut archive::Archive,
    index: &archive::ArchiveIndex,
    page_count: usize,
    extracted: &mut u64,
) -> Result<Option<ComicInfo>, ImportError> {
    let path = if index.contains("ComicInfo.xml") {
        "ComicInfo.xml"
    } else {
        let mut matches = index
            .paths()
            .filter(|path| !ignored(path))
            .filter(|path| path.rsplit('/').next() == Some("ComicInfo.xml"));
        let Some(path) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Ok(None);
        }
        path
    };
    let Ok(entry) = archive.by_name(path) else {
        return Ok(None);
    };
    if entry.size() > MAX_COMIC_INFO_BYTES {
        return Ok(None);
    }
    let Ok(capacity) = usize::try_from(entry.size()) else {
        return Ok(None);
    };
    let mut xml = Vec::with_capacity(capacity);
    if entry
        .take(MAX_COMIC_INFO_BYTES + 1)
        .read_to_end(&mut xml)
        .is_err()
    {
        return Ok(None);
    }
    if xml.len() as u64 > MAX_COMIC_INFO_BYTES {
        return Ok(None);
    }
    archive::add_extracted(
        extracted,
        u64::try_from(xml.len()).map_err(|_| ImportError::ArchiveTooLarge)?,
    )?;
    let Ok(info) = parse_comic_info(&xml) else {
        return Ok(None);
    };
    if info.cover_index.is_some_and(|value| value >= page_count) {
        return Ok(None);
    }
    Ok(Some(info))
}

fn parse_comic_info(xml: &[u8]) -> Result<ComicInfo, ()> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut pages_depth = None;
    let mut field = None::<(ComicInfoField, usize, String)>;
    let mut info = ComicInfo::default();
    loop {
        let event = reader.read_event().map_err(|_| ())?;
        match event {
            Event::Start(event) => {
                depth = depth
                    .checked_add(1)
                    .filter(|depth| *depth <= MAX_COMIC_INFO_DEPTH)
                    .ok_or(())?;
                let name = event.local_name();
                if depth == 1 {
                    if root_seen || name.as_ref() != b"ComicInfo" {
                        return Err(());
                    }
                    root_seen = true;
                } else if !root_seen || root_closed {
                    return Err(());
                } else if depth == 2 && name.as_ref() == b"Pages" {
                    if pages_depth.is_some() {
                        return Err(());
                    }
                    pages_depth = Some(depth);
                } else if depth == 2 && matches!(name.as_ref(), b"Title" | b"Writer") {
                    let kind = if name.as_ref() == b"Title" {
                        ComicInfoField::Title
                    } else {
                        ComicInfoField::Writer
                    };
                    if field.is_some()
                        || matches!(kind, ComicInfoField::Title) && info.title.is_some()
                        || matches!(kind, ComicInfoField::Writer) && info.writer.is_some()
                    {
                        return Err(());
                    }
                    field = Some((kind, depth, String::new()));
                } else if pages_depth == Some(depth.saturating_sub(1)) && name.as_ref() == b"Page" {
                    set_front_cover(&reader, &event, &mut info.cover_index)?;
                } else if field.is_some() {
                    return Err(());
                }
            }
            Event::Empty(event) => {
                let name = event.local_name();
                if pages_depth == Some(depth) && name.as_ref() == b"Page" {
                    set_front_cover(&reader, &event, &mut info.cover_index)?;
                } else if depth == 0 || field.is_some() {
                    return Err(());
                }
            }
            Event::End(event) => {
                if field
                    .as_ref()
                    .is_some_and(|(_, field_depth, _)| *field_depth == depth)
                {
                    let (kind, _, value) = field.take().ok_or(())?;
                    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !value.is_empty() {
                        match kind {
                            ComicInfoField::Title => info.title = Some(value),
                            ComicInfoField::Writer => info.writer = Some(value),
                        }
                    }
                }
                if event.local_name().as_ref() == b"Pages" && pages_depth == Some(depth) {
                    pages_depth = None;
                }
                if depth == 1 {
                    if event.local_name().as_ref() != b"ComicInfo" || !root_seen {
                        return Err(());
                    }
                    root_closed = true;
                }
                depth = depth.checked_sub(1).ok_or(())?;
            }
            Event::Text(text) if field.is_some() => {
                let decoded = text.decode().map_err(|_| ())?;
                let value = quick_xml::escape::unescape(&decoded).map_err(|_| ())?;
                let target = &mut field.as_mut().ok_or(())?.2;
                if target.len().saturating_add(value.len()) > MAX_COMIC_INFO_TEXT_BYTES {
                    return Err(());
                }
                target.push_str(&value);
            }
            Event::CData(text) if field.is_some() => {
                let value = text.decode().map_err(|_| ())?;
                let target = &mut field.as_mut().ok_or(())?.2;
                if target.len().saturating_add(value.len()) > MAX_COMIC_INFO_TEXT_BYTES {
                    return Err(());
                }
                target.push_str(&value);
            }
            Event::DocType(_) | Event::GeneralRef(_) => return Err(()),
            Event::Eof if depth == 0 && root_seen && root_closed && field.is_none() => break,
            Event::Eof => return Err(()),
            _ => {}
        }
    }
    Ok(info)
}

fn set_front_cover(
    reader: &NsReader<&[u8]>,
    event: &BytesStart<'_>,
    cover_index: &mut Option<usize>,
) -> Result<(), ()> {
    let page_type = xml_attribute(reader.decoder(), event, b"Type")?;
    if page_type.as_deref() != Some("FrontCover") {
        return Ok(());
    }
    if cover_index.is_some() {
        return Err(());
    }
    *cover_index = Some(
        xml_attribute(reader.decoder(), event, b"Image")?
            .ok_or(())?
            .parse()
            .map_err(|_| ())?,
    );
    Ok(())
}

fn xml_attribute(
    decoder: Decoder,
    event: &BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, ()> {
    let mut found = None;
    for value in event.attributes().with_checks(true) {
        let value = value.map_err(|_| ())?;
        if value.key.as_ref() == name {
            if found.is_some() {
                return Err(());
            }
            found = Some(
                value
                    .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                    .map_err(|_| ())?
                    .into_owned(),
            );
        }
    }
    Ok(found)
}

fn validate_image(bytes: &[u8], kind: PageKind) -> Result<(), ImportError> {
    if imagesize::image_type(bytes).ok() != Some(kind.image_type()) {
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

fn page_kind(path: &str) -> Option<PageKind> {
    match Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => Some(PageKind::Jpeg),
        "png" => Some(PageKind::Png),
        _ => None,
    }
}

fn ignored(path: &str) -> bool {
    path.split('/').any(|part| {
        part.starts_with('.') || part.eq_ignore_ascii_case("__MACOSX") || part.starts_with("._")
    })
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left_parts = left.split('/');
    let mut right_parts = right.split('/');
    loop {
        match (left_parts.next(), right_parts.next()) {
            (Some(left), Some(right)) => match natural_segment_cmp(left, right)
                .then_with(|| left.as_bytes().cmp(right.as_bytes()))
            {
                Ordering::Equal => {}
                order => return order,
            },
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => return Ordering::Equal,
        }
    }
}

fn natural_segment_cmp(left: &str, right: &str) -> Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let (mut left_at, mut right_at) = (0, 0);
    while left_at < left.len() && right_at < right.len() {
        if left[left_at].is_ascii_digit() && right[right_at].is_ascii_digit() {
            let left_end = digit_end(left, left_at);
            let right_end = digit_end(right, right_at);
            let left_number = trim_zeroes(&left[left_at..left_end]);
            let right_number = trim_zeroes(&right[right_at..right_end]);
            match left_number
                .len()
                .cmp(&right_number.len())
                .then_with(|| left_number.cmp(right_number))
            {
                Ordering::Equal => {
                    left_at = left_end;
                    right_at = right_end;
                    continue;
                }
                order => return order,
            }
        }
        match left[left_at]
            .to_ascii_lowercase()
            .cmp(&right[right_at].to_ascii_lowercase())
        {
            Ordering::Equal => {
                left_at += 1;
                right_at += 1;
            }
            order => return order,
        }
    }
    (left.len() - left_at).cmp(&(right.len() - right_at))
}

fn digit_end(value: &[u8], start: usize) -> usize {
    value[start..]
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .map_or(value.len(), |offset| start + offset)
}

fn trim_zeroes(value: &[u8]) -> &[u8] {
    let first = value
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(value.len());
    &value[first..]
}

fn page_xhtml(page: usize, image_name: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><meta charset=\"utf-8\" /><title>第 {page} 页</title></head><body><main class=\"atha-cbz-page\"><img src=\"images/{image_name}\" alt=\"第 {page} 页\" /></main></body></html>\n"
    )
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
        .is_ok_and(|value| value == format!("atha-cbz-import-v1\n{content_version}\n"))
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
        || metadata
            .cover_path
            .as_ref()
            .is_none_or(|path| !path.starts_with(".atha-cbz/images/") || page_kind(path).is_none())
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
                "operation=import stage=publish-rename outcome=failed code=cbz-import-write-failed io_kind={:?}",
                error.kind()
            );
            Err(ImportError::WriteFailed)
        }
    }
}
