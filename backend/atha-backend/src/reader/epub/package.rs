use std::collections::{HashMap, HashSet};

use quick_xml::{
    Decoder, NsReader, XmlVersion,
    events::{BytesRef, BytesStart, Event},
    name::ResolveResult,
};
use serde::Serialize;

use super::{ImportError, MAX_ENTRIES, MAX_SECTIONS, MAX_TOC_ITEMS, archive};

const CONTAINER_NS: &[u8] = b"urn:oasis:names:tc:opendocument:xmlns:container";
const OPF_NS: &[u8] = b"http://www.idpf.org/2007/opf";
const DC_NS: &[u8] = b"http://purl.org/dc/elements/1.1/";
const XHTML_NS: &[u8] = b"http://www.w3.org/1999/xhtml";
const EPUB_NS: &[u8] = b"http://www.idpf.org/2007/ops";
const NCX_NS: &[u8] = b"http://www.daisy.org/z3986/2005/ncx/";
const NCX_MEDIA_TYPE: &str = "application/x-dtbncx+xml";
const NCX_DOCTYPE: &[u8] =
    br#"ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd""#;
const MAX_XML_DEPTH: usize = 256;
const MAX_TOC_LABEL_BYTES: usize = 1024;
pub(super) const MAX_METADATA_TEXT: usize = 512;
pub(super) const MAX_AUTHORS: usize = 16;

#[derive(Debug)]
struct PackageItem {
    id: String,
    path: String,
    media_type: String,
    properties: String,
}

#[derive(Debug)]
pub(super) struct Package {
    items: HashMap<String, PackageItem>,
    spine: Vec<String>,
    navigation: Navigation,
    title: Option<String>,
    authors: Vec<String>,
    cover_path: Option<String>,
}

#[derive(Debug)]
enum Navigation {
    Xhtml(String),
    Ncx(String),
}

#[derive(Clone, Copy)]
enum PackageVersion {
    Epub2,
    Epub3,
}

#[derive(Debug, Serialize)]
pub(super) struct ReaderManifest {
    schema: u8,
    #[serde(rename = "contentVersion")]
    content_version: String,
    sections: Vec<Section>,
    resources: Vec<String>,
    toc: Vec<TocItem>,
}

#[derive(Debug, Serialize)]
struct Section {
    id: String,
    href: String,
}

#[derive(Debug, Serialize)]
struct TocItem {
    label: String,
    href: String,
}

pub(super) struct ImportPlan {
    pub(super) manifest: ReaderManifest,
    pub(super) files: Vec<String>,
    pub(super) title: Option<String>,
    pub(super) authors: Vec<String>,
    pub(super) cover_path: Option<String>,
}

impl ImportPlan {
    pub(super) fn section_paths(&self) -> impl Iterator<Item = &str> {
        self.manifest
            .sections
            .iter()
            .map(|section| section.href.as_str())
    }
}

#[derive(Clone, Copy)]
enum MetadataField {
    Title,
    Creator,
}

pub(super) fn parse_container(xml: &[u8]) -> Result<String, ImportError> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut container_seen = false;
    let mut rootfiles_depth = None;
    let mut rootfiles_count = 0_u8;
    let mut rootfiles = Vec::new();
    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ImportError::InvalidXml)?;
        match event {
            Event::Start(event) => {
                depth = next_xml_depth(depth)?;
                let name = event.local_name();
                match name.as_ref() {
                    b"container" if depth == 1 && !container_seen => {
                        require_namespace(&namespace, CONTAINER_NS)?;
                        if attribute(reader.decoder(), &event, b"version")?.as_deref()
                            != Some("1.0")
                        {
                            return Err(ImportError::UnsupportedEpub);
                        }
                        container_seen = true;
                    }
                    b"rootfiles" => {
                        require_namespace(&namespace, CONTAINER_NS)?;
                        if depth != 2 || rootfiles_depth.is_some() {
                            return Err(ImportError::InvalidXml);
                        }
                        rootfiles_count = rootfiles_count.saturating_add(1);
                        rootfiles_depth = Some(depth);
                    }
                    b"rootfile" => {
                        require_namespace(&namespace, CONTAINER_NS)?;
                        if rootfiles_depth != Some(depth - 1) {
                            return Err(ImportError::InvalidXml);
                        }
                        rootfiles.push(container_rootfile(reader.decoder(), &event)?);
                    }
                    _ if depth == 1 => return Err(ImportError::InvalidXml),
                    _ if depth == 0 => return Err(ImportError::InvalidXml),
                    _ => {}
                }
            }
            Event::Empty(event) => {
                next_xml_depth(depth)?;
                let name = event.local_name();
                match name.as_ref() {
                    b"rootfiles" => {
                        require_namespace(&namespace, CONTAINER_NS)?;
                        if depth != 1 {
                            return Err(ImportError::InvalidXml);
                        }
                        rootfiles_count = rootfiles_count.saturating_add(1);
                    }
                    b"rootfile" => {
                        require_namespace(&namespace, CONTAINER_NS)?;
                        if rootfiles_depth != Some(depth) {
                            return Err(ImportError::InvalidXml);
                        }
                        rootfiles.push(container_rootfile(reader.decoder(), &event)?);
                    }
                    b"container" if depth == 0 => return Err(ImportError::UnsupportedEpub),
                    _ if depth == 0 => return Err(ImportError::InvalidXml),
                    _ => {}
                }
            }
            Event::End(event) => {
                let name = event.local_name();
                if name.as_ref() == b"rootfiles" && rootfiles_depth == Some(depth) {
                    require_namespace(&namespace, CONTAINER_NS)?;
                    rootfiles_depth = None;
                }
                depth = depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
            }
            Event::DocType(_) => return Err(ImportError::InvalidXml),
            Event::Text(text) if depth == 0 && !xml_whitespace(text.as_ref()) => {
                return Err(ImportError::InvalidXml);
            }
            Event::CData(_) if depth == 0 => return Err(ImportError::InvalidXml),
            Event::Eof if depth == 0 && rootfiles_depth.is_none() => break,
            Event::Eof => return Err(ImportError::InvalidXml),
            _ => {}
        }
    }
    if !container_seen || rootfiles_count != 1 || rootfiles.len() != 1 {
        return Err(ImportError::UnsupportedEpub);
    }
    let (path, media_type) = rootfiles.remove(0);
    if media_type != "application/oebps-package+xml" {
        return Err(ImportError::UnsupportedEpub);
    }
    Ok(path)
}

fn container_rootfile(
    decoder: Decoder,
    event: &BytesStart<'_>,
) -> Result<(String, String), ImportError> {
    let path = attribute(decoder, event, b"full-path")?.ok_or(ImportError::InvalidXml)?;
    let media_type = attribute(decoder, event, b"media-type")?.ok_or(ImportError::InvalidXml)?;
    Ok((archive::safe_path(&path)?, media_type))
}

pub(super) fn parse_package(xml: &[u8], package_path: &str) -> Result<Package, ImportError> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut package_seen = false;
    let mut version = None;
    let mut items = HashMap::new();
    let mut paths = HashSet::new();
    let mut spine = Vec::new();
    let mut nav_path = None;
    let mut spine_toc = None;
    let mut cover_id = None;
    let mut title = None;
    let mut authors = Vec::new();
    let mut metadata_depth = None;
    let mut metadata_field: Option<(MetadataField, usize, String)> = None;
    let mut manifest_depth = None;
    let mut spine_depth = None;
    let mut manifest_count = 0_u8;
    let mut spine_count = 0_u8;
    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ImportError::InvalidXml)?;
        match event {
            Event::Start(event) => {
                depth = next_xml_depth(depth)?;
                let name = event.local_name();
                match name.as_ref() {
                    b"package" if depth == 1 && !package_seen => {
                        require_namespace(&namespace, OPF_NS)?;
                        package_seen = true;
                        version = attribute(reader.decoder(), &event, b"version")?;
                    }
                    b"metadata" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if depth != 2 || metadata_depth.is_some() {
                            return Err(ImportError::InvalidXml);
                        }
                        metadata_depth = Some(depth);
                    }
                    b"title" if metadata_depth == Some(depth - 1) => {
                        require_namespace(&namespace, DC_NS)?;
                        if metadata_field.is_some() {
                            return Err(ImportError::InvalidXml);
                        }
                        metadata_field = Some((MetadataField::Title, depth, String::new()));
                    }
                    b"creator" if metadata_depth == Some(depth - 1) => {
                        require_namespace(&namespace, DC_NS)?;
                        if metadata_field.is_some() {
                            return Err(ImportError::InvalidXml);
                        }
                        metadata_field = Some((MetadataField::Creator, depth, String::new()));
                    }
                    b"meta" if metadata_depth == Some(depth - 1) => {
                        require_namespace(&namespace, OPF_NS)?;
                        if version.as_deref() == Some("2.0") {
                            capture_cover_meta(reader.decoder(), &event, &mut cover_id)?;
                        }
                    }
                    b"manifest" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if depth != 2 || manifest_depth.is_some() {
                            return Err(ImportError::InvalidXml);
                        }
                        manifest_count = manifest_count.saturating_add(1);
                        manifest_depth = Some(depth);
                    }
                    b"spine" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if depth != 2 || spine_depth.is_some() {
                            return Err(ImportError::InvalidXml);
                        }
                        spine_count = spine_count.saturating_add(1);
                        capture_spine_toc(reader.decoder(), &event, &mut spine_toc)?;
                        spine_depth = Some(depth);
                    }
                    b"item" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if manifest_depth != Some(depth - 1) {
                            return Err(ImportError::InvalidXml);
                        }
                        insert_item(
                            reader.decoder(),
                            &event,
                            package_path,
                            &mut items,
                            &mut paths,
                            &mut nav_path,
                        )?;
                    }
                    b"itemref" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if spine_depth != Some(depth - 1) {
                            return Err(ImportError::InvalidXml);
                        }
                        push_spine(reader.decoder(), &event, &mut spine)?;
                    }
                    _ if depth == 1 => return Err(ImportError::InvalidXml),
                    _ if depth == 0 => return Err(ImportError::InvalidXml),
                    _ => {}
                }
            }
            Event::Empty(event) => {
                next_xml_depth(depth)?;
                let name = event.local_name();
                match name.as_ref() {
                    b"manifest" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if depth != 1 {
                            return Err(ImportError::InvalidXml);
                        }
                        manifest_count = manifest_count.saturating_add(1);
                    }
                    b"spine" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if depth != 1 {
                            return Err(ImportError::InvalidXml);
                        }
                        spine_count = spine_count.saturating_add(1);
                        capture_spine_toc(reader.decoder(), &event, &mut spine_toc)?;
                    }
                    b"meta" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if metadata_depth != Some(depth) {
                            return Err(ImportError::InvalidXml);
                        }
                        if version.as_deref() == Some("2.0") {
                            capture_cover_meta(reader.decoder(), &event, &mut cover_id)?;
                        }
                    }
                    b"item" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if manifest_depth != Some(depth) {
                            return Err(ImportError::InvalidXml);
                        }
                        insert_item(
                            reader.decoder(),
                            &event,
                            package_path,
                            &mut items,
                            &mut paths,
                            &mut nav_path,
                        )?;
                    }
                    b"itemref" => {
                        require_namespace(&namespace, OPF_NS)?;
                        if spine_depth != Some(depth) {
                            return Err(ImportError::InvalidXml);
                        }
                        push_spine(reader.decoder(), &event, &mut spine)?;
                    }
                    b"package" if depth == 0 => return Err(ImportError::UnsupportedEpub),
                    _ if depth == 0 => return Err(ImportError::InvalidXml),
                    _ => {}
                }
            }
            Event::End(event) => {
                let name = event.local_name();
                if metadata_field
                    .as_ref()
                    .is_some_and(|(_, field_depth, _)| *field_depth == depth)
                {
                    let (field, _, value) = metadata_field.take().expect("metadata field");
                    let value = normalize_metadata(&value);
                    if !value.is_empty() {
                        match field {
                            MetadataField::Title if title.is_none() => title = Some(value),
                            MetadataField::Creator if authors.len() < MAX_AUTHORS => {
                                authors.push(value);
                            }
                            _ => {}
                        }
                    }
                } else if name.as_ref() == b"metadata" && metadata_depth == Some(depth) {
                    require_namespace(&namespace, OPF_NS)?;
                    metadata_depth = None;
                } else if name.as_ref() == b"manifest" && manifest_depth == Some(depth) {
                    require_namespace(&namespace, OPF_NS)?;
                    manifest_depth = None;
                } else if name.as_ref() == b"spine" && spine_depth == Some(depth) {
                    require_namespace(&namespace, OPF_NS)?;
                    spine_depth = None;
                }
                depth = depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
            }
            Event::Text(text) if metadata_field.is_some() => {
                let decoded = text.decode().map_err(|_| ImportError::InvalidXml)?;
                let value =
                    quick_xml::escape::unescape(&decoded).map_err(|_| ImportError::InvalidXml)?;
                let target = &mut metadata_field.as_mut().expect("metadata field").2;
                if target.len().saturating_add(value.len()) > MAX_METADATA_TEXT {
                    return Err(ImportError::InvalidXml);
                }
                target.push_str(&value);
            }
            Event::GeneralRef(value) if metadata_field.is_some() => {
                let target = &mut metadata_field.as_mut().expect("metadata field").2;
                push_xml_reference(target, &value, MAX_METADATA_TEXT)?;
            }
            Event::DocType(_) => return Err(ImportError::InvalidXml),
            Event::Text(text) if depth == 0 && !xml_whitespace(text.as_ref()) => {
                return Err(ImportError::InvalidXml);
            }
            Event::CData(_) if depth == 0 => return Err(ImportError::InvalidXml),
            Event::Eof
                if depth == 0
                    && metadata_depth.is_none()
                    && metadata_field.is_none()
                    && manifest_depth.is_none()
                    && spine_depth.is_none() =>
            {
                break;
            }
            Event::Eof => return Err(ImportError::InvalidXml),
            _ => {}
        }
    }
    let version = match version.as_deref() {
        Some("2.0") => PackageVersion::Epub2,
        Some("3.0") => PackageVersion::Epub3,
        _ => return Err(ImportError::UnsupportedEpub),
    };
    if !package_seen
        || manifest_count != 1
        || spine_count != 1
        || items.is_empty()
        || spine.is_empty()
    {
        return Err(ImportError::UnsupportedEpub);
    }
    let navigation = match version {
        PackageVersion::Epub2 => {
            let toc = spine_toc.ok_or(ImportError::UnsupportedEpub)?;
            let item = items.get(&toc).ok_or(ImportError::UnsupportedEpub)?;
            if item.media_type != NCX_MEDIA_TYPE {
                return Err(ImportError::UnsupportedEpub);
            }
            Navigation::Ncx(item.path.clone())
        }
        PackageVersion::Epub3 => Navigation::Xhtml(nav_path.ok_or(ImportError::UnsupportedEpub)?),
    };
    let cover_path = match version {
        PackageVersion::Epub2 => match cover_id {
            Some(id) => {
                let item = items.get(&id).ok_or(ImportError::InvalidXml)?;
                if !item.media_type.starts_with("image/")
                    || !resource_type(&item.media_type, &item.path)
                {
                    return Err(ImportError::InvalidXml);
                }
                Some(item.path.clone())
            }
            None => None,
        },
        PackageVersion::Epub3 => {
            let mut paths = items
                .values()
                .filter(|item| {
                    item.properties
                        .split_ascii_whitespace()
                        .any(|value| value == "cover-image")
                })
                .map(|item| item.path.clone());
            let path = paths.next();
            if paths.next().is_some() {
                return Err(ImportError::InvalidXml);
            }
            path
        }
    };
    let cover_path = cover_path.filter(|path| {
        items
            .values()
            .find(|item| item.path == *path)
            .is_some_and(|item| resource_type(&item.media_type, path))
    });
    Ok(Package {
        items,
        spine,
        navigation,
        title,
        authors,
        cover_path,
    })
}

fn package_item(
    decoder: Decoder,
    event: &BytesStart<'_>,
    package_path: &str,
) -> Result<PackageItem, ImportError> {
    let id = attribute(decoder, event, b"id")?.ok_or(ImportError::InvalidXml)?;
    let href = attribute(decoder, event, b"href")?.ok_or(ImportError::InvalidXml)?;
    let media_type = attribute(decoder, event, b"media-type")?.ok_or(ImportError::InvalidXml)?;
    let properties = attribute(decoder, event, b"properties")?.unwrap_or_default();
    Ok(PackageItem {
        id,
        path: archive::resolve_reference(package_path, &href)?.0,
        media_type,
        properties,
    })
}

fn capture_cover_meta(
    decoder: Decoder,
    event: &BytesStart<'_>,
    cover_id: &mut Option<String>,
) -> Result<(), ImportError> {
    if attribute(decoder, event, b"name")?.as_deref() != Some("cover") {
        return Ok(());
    }
    let id = attribute(decoder, event, b"content")?.ok_or(ImportError::InvalidXml)?;
    if id.is_empty() || cover_id.replace(id).is_some() {
        return Err(ImportError::InvalidXml);
    }
    Ok(())
}

fn capture_spine_toc(
    decoder: Decoder,
    event: &BytesStart<'_>,
    spine_toc: &mut Option<String>,
) -> Result<(), ImportError> {
    let Some(id) = attribute(decoder, event, b"toc")? else {
        return Ok(());
    };
    if id.is_empty() || spine_toc.replace(id).is_some() {
        return Err(ImportError::InvalidXml);
    }
    Ok(())
}

fn insert_item(
    decoder: Decoder,
    event: &BytesStart<'_>,
    package_path: &str,
    items: &mut HashMap<String, PackageItem>,
    paths: &mut HashSet<String>,
    nav_path: &mut Option<String>,
) -> Result<(), ImportError> {
    let item = package_item(decoder, event, package_path)?;
    if items.len() >= MAX_ENTRIES
        || items.contains_key(&item.id)
        || !paths.insert(item.path.clone())
    {
        return Err(ImportError::InvalidXml);
    }
    if item
        .properties
        .split_ascii_whitespace()
        .any(|value| value == "nav")
        && nav_path.replace(item.path.clone()).is_some()
    {
        return Err(ImportError::UnsupportedEpub);
    }
    items.insert(item.id.clone(), item);
    Ok(())
}

fn push_spine(
    decoder: Decoder,
    event: &BytesStart<'_>,
    spine: &mut Vec<String>,
) -> Result<(), ImportError> {
    if spine.len() >= MAX_SECTIONS {
        return Err(ImportError::TooManySections);
    }
    spine.push(attribute(decoder, event, b"idref")?.ok_or(ImportError::InvalidXml)?);
    Ok(())
}

pub(super) fn plan_import(
    archive: &mut archive::EpubArchive,
    index: &archive::ArchiveIndex,
    package: Package,
    content_version: &str,
) -> Result<ImportPlan, ImportError> {
    let title = package.title.clone();
    let authors = package.authors.clone();
    let cover_path = package.cover_path.clone();
    let (nav_path, ncx) = match &package.navigation {
        Navigation::Xhtml(path) => (path, false),
        Navigation::Ncx(path) => (path, true),
    };
    let nav_item = package
        .items
        .values()
        .find(|item| item.path == *nav_path)
        .ok_or(ImportError::InvalidXml)?;
    let expected_media_type = if ncx {
        NCX_MEDIA_TYPE
    } else {
        "application/xhtml+xml"
    };
    if nav_item.media_type != expected_media_type {
        return Err(ImportError::UnsupportedEpub);
    }
    let mut sections = Vec::with_capacity(package.spine.len());
    let mut section_paths = HashSet::with_capacity(package.spine.len());
    for (position, idref) in package.spine.iter().enumerate() {
        let item = package.items.get(idref).ok_or(ImportError::InvalidXml)?;
        if item.media_type != "application/xhtml+xml" {
            return Err(ImportError::UnsupportedEpub);
        }
        archive::require(index, &item.path)?;
        if !section_paths.insert(item.path.clone()) {
            return Err(ImportError::InvalidXml);
        }
        sections.push(Section {
            id: format!("section-{}", position + 1),
            href: item.path.clone(),
        });
    }

    let nav = archive::read(archive, index, nav_path)?;
    let toc = if ncx {
        parse_ncx(&nav, nav_path, &section_paths)?
    } else {
        parse_navigation(&nav, nav_path, &section_paths)?
    };
    let mut resources = package
        .items
        .values()
        .filter(|item| resource_type(&item.media_type, &item.path))
        .map(|item| item.path.clone())
        .filter(|path| !section_paths.contains(path))
        .collect::<Vec<_>>();
    resources.sort();
    resources.dedup();
    for resource in &resources {
        archive::require(index, resource)?;
    }
    let mut files = sections
        .iter()
        .map(|section| section.href.clone())
        .chain(resources.iter().cloned())
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    Ok(ImportPlan {
        manifest: ReaderManifest {
            schema: 1,
            content_version: content_version.to_owned(),
            sections,
            resources,
            toc,
        },
        files,
        title,
        authors,
        cover_path,
    })
}

fn normalize_metadata(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_navigation(
    xml: &[u8],
    nav_path: &str,
    sections: &HashSet<String>,
) -> Result<Vec<TocItem>, ImportError> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0_usize;
    let mut html_seen = false;
    let mut doctype_seen = false;
    let mut body_depth = None;
    let mut body_count = 0_u8;
    let mut toc_depth = None;
    let mut toc_count = 0_u8;
    let mut link: Option<(usize, String, String)> = None;
    let mut result = Vec::new();
    let mut hrefs = HashSet::new();
    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ImportError::InvalidXml)?;
        match event {
            Event::Start(event) => {
                depth = next_xml_depth(depth)?;
                let name = event.local_name();
                if depth == 1 {
                    if name.as_ref() != b"html" || html_seen {
                        return Err(ImportError::InvalidXml);
                    }
                    require_namespace(&namespace, XHTML_NS)?;
                    html_seen = true;
                }
                let xhtml = namespace_is(&namespace, XHTML_NS);
                if xhtml && name.as_ref() == b"body" {
                    if depth != 2 || body_depth.is_some() {
                        return Err(ImportError::InvalidXml);
                    }
                    body_count = body_count.saturating_add(1);
                    body_depth = Some(depth);
                } else if xhtml && name.as_ref() == b"nav" && is_toc_nav(&reader, &event)? {
                    if body_depth.is_none() || toc_depth.is_some() || toc_count != 0 {
                        return Err(ImportError::UnsupportedEpub);
                    }
                    toc_count += 1;
                    toc_depth = Some(depth);
                } else if xhtml
                    && name.as_ref() == b"a"
                    && toc_depth.is_some()
                    && link.is_none()
                    && let Some(href) = attribute(reader.decoder(), &event, b"href")?
                {
                    link = Some((depth, href, String::new()));
                }
            }
            Event::Text(text) if link.is_some() => {
                let decoded = text.decode().map_err(|_| ImportError::InvalidXml)?;
                let value =
                    quick_xml::escape::unescape(&decoded).map_err(|_| ImportError::InvalidXml)?;
                push_ncx_label(&mut link.as_mut().expect("checked link").2, &value)?;
            }
            Event::CData(text) if link.is_some() => {
                let value = text.decode().map_err(|_| ImportError::InvalidXml)?;
                push_ncx_label(&mut link.as_mut().expect("checked link").2, &value)?;
            }
            Event::GeneralRef(value) if link.is_some() => {
                push_xml_reference(
                    &mut link.as_mut().expect("checked link").2,
                    &value,
                    MAX_TOC_LABEL_BYTES,
                )?;
            }
            Event::End(event) => {
                let name = event.local_name();
                let xhtml = namespace_is(&namespace, XHTML_NS);
                if xhtml
                    && name.as_ref() == b"a"
                    && link.as_ref().is_some_and(|value| value.0 == depth)
                {
                    let (_, raw_href, raw_label) = link.take().expect("checked link");
                    let (path, fragment) = archive::resolve_reference(nav_path, &raw_href)?;
                    if !sections.contains(&path) {
                        return Err(ImportError::UnsupportedEpub);
                    }
                    let label = raw_label.split_whitespace().collect::<Vec<_>>().join(" ");
                    if label.is_empty() || label.encode_utf16().count() > 256 {
                        return Err(ImportError::InvalidXml);
                    }
                    let href =
                        fragment.map_or_else(|| path.clone(), |value| format!("{path}#{value}"));
                    if !hrefs.insert(href.clone()) {
                        return Err(ImportError::InvalidXml);
                    }
                    if result.len() >= MAX_TOC_ITEMS {
                        return Err(ImportError::TooManyTocItems);
                    }
                    result.push(TocItem { label, href });
                }
                if xhtml && name.as_ref() == b"nav" && toc_depth == Some(depth) {
                    toc_depth = None;
                }
                if xhtml && name.as_ref() == b"body" && body_depth == Some(depth) {
                    body_depth = None;
                }
                depth = depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
            }
            Event::Empty(_) => {
                if next_xml_depth(depth)? == 1 {
                    return Err(ImportError::UnsupportedEpub);
                }
            }
            Event::DocType(value) => {
                let value: &[u8] = value.as_ref();
                if depth != 0 || html_seen || doctype_seen || value != b"html" {
                    return Err(ImportError::InvalidXml);
                }
                doctype_seen = true;
            }
            Event::Text(text) if depth == 0 && !xml_whitespace(text.as_ref()) => {
                return Err(ImportError::InvalidXml);
            }
            Event::CData(_) if depth == 0 => return Err(ImportError::InvalidXml),
            Event::Eof
                if depth == 0 && body_depth.is_none() && toc_depth.is_none() && link.is_none() =>
            {
                break;
            }
            Event::Eof => return Err(ImportError::InvalidXml),
            _ => {}
        }
    }
    if !html_seen || body_count != 1 || toc_count != 1 || result.is_empty() {
        return Err(ImportError::UnsupportedEpub);
    }
    Ok(result)
}

struct NcxPoint {
    depth: usize,
    slot: usize,
    label_depth: Option<usize>,
    text_depth: Option<usize>,
    content_depth: Option<usize>,
    text_seen: bool,
    label: Option<String>,
    label_candidate: String,
    source: Option<String>,
}

fn parse_ncx(
    xml: &[u8],
    nav_path: &str,
    sections: &HashSet<String>,
) -> Result<Vec<TocItem>, ImportError> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0_usize;
    let mut ncx_seen = false;
    let mut doctype_seen = false;
    let mut head_seen = false;
    let mut doc_title_seen = false;
    let mut doc_author_seen = false;
    let mut nav_map_depth = None;
    let mut ignored_depth = None;
    let mut nav_map_count = 0_u8;
    let mut page_list_seen = false;
    let mut nav_list_seen = false;
    let mut points = Vec::<NcxPoint>::new();
    let mut point_ids = HashSet::new();
    let mut play_orders = HashSet::new();
    let mut hrefs = HashSet::new();
    let mut result = Vec::new();
    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| ImportError::InvalidXml)?;
        match event {
            Event::Start(event) => {
                depth = next_xml_depth(depth)?;
                let name = event.local_name();
                if matches!(name.as_ref(), b"audio" | b"img") {
                    return Err(ImportError::InvalidXml);
                }
                if ignored_depth.is_some() {
                    continue;
                }
                if depth == 1 {
                    if name.as_ref() != b"ncx" || ncx_seen {
                        return Err(ImportError::InvalidXml);
                    }
                    require_namespace(&namespace, NCX_NS)?;
                    if attribute(reader.decoder(), &event, b"version")?.as_deref() != Some("2005-1")
                    {
                        return Err(ImportError::UnsupportedEpub);
                    }
                    ncx_seen = true;
                } else if depth == 2 {
                    require_namespace(&namespace, NCX_NS)?;
                    match name.as_ref() {
                        b"head" => {
                            if head_seen || doc_title_seen || nav_map_count != 0 {
                                return Err(ImportError::InvalidXml);
                            }
                            head_seen = true;
                            ignored_depth = Some(depth);
                        }
                        b"docTitle" => {
                            if !head_seen || doc_title_seen || nav_map_count != 0 {
                                return Err(ImportError::InvalidXml);
                            }
                            doc_title_seen = true;
                            ignored_depth = Some(depth);
                        }
                        b"docAuthor" => {
                            if !head_seen
                                || !doc_title_seen
                                || doc_author_seen
                                || nav_map_count != 0
                            {
                                return Err(ImportError::InvalidXml);
                            }
                            doc_author_seen = true;
                            ignored_depth = Some(depth);
                        }
                        b"navMap" => {
                            if !head_seen || !doc_title_seen || nav_map_count != 0 {
                                return Err(ImportError::InvalidXml);
                            }
                            nav_map_count = 1;
                            nav_map_depth = Some(depth);
                        }
                        b"pageList" => {
                            if nav_map_count != 1
                                || nav_map_depth.is_some()
                                || page_list_seen
                                || nav_list_seen
                            {
                                return Err(ImportError::InvalidXml);
                            }
                            page_list_seen = true;
                            ignored_depth = Some(depth);
                        }
                        b"navList" => {
                            if nav_map_count != 1 || nav_map_depth.is_some() {
                                return Err(ImportError::InvalidXml);
                            }
                            nav_list_seen = true;
                            ignored_depth = Some(depth);
                        }
                        _ => return Err(ImportError::InvalidXml),
                    }
                } else if namespace_is(&namespace, NCX_NS) && name.as_ref() == b"navPoint" {
                    let expected_depth = points.last().map_or_else(
                        || nav_map_depth.map(|value| value + 1),
                        |point| Some(point.depth + 1),
                    );
                    if expected_depth != Some(depth)
                        || points.last().is_some_and(|point| {
                            point.label_depth.is_some()
                                || point.content_depth.is_some()
                                || point.source.is_none()
                        })
                    {
                        return Err(ImportError::InvalidXml);
                    }
                    if result.len() >= MAX_TOC_ITEMS {
                        return Err(ImportError::TooManyTocItems);
                    }
                    let id = attribute(reader.decoder(), &event, b"id")?
                        .ok_or(ImportError::InvalidXml)?;
                    if id.is_empty()
                        || id.len() > 512
                        || id.chars().any(char::is_control)
                        || !point_ids.insert(id)
                    {
                        return Err(ImportError::InvalidXml);
                    }
                    match attribute(reader.decoder(), &event, b"playOrder")? {
                        Some(value) => {
                            let play_order =
                                value.parse::<u32>().map_err(|_| ImportError::InvalidXml)?;
                            if play_order == 0 || !play_orders.insert(play_order) {
                                return Err(ImportError::InvalidXml);
                            }
                        }
                        None if doctype_seen => return Err(ImportError::InvalidXml),
                        None => {}
                    }
                    let slot = result.len();
                    result.push(TocItem {
                        label: String::new(),
                        href: String::new(),
                    });
                    points.push(NcxPoint {
                        depth,
                        slot,
                        label_depth: None,
                        text_depth: None,
                        content_depth: None,
                        text_seen: false,
                        label: None,
                        label_candidate: String::new(),
                        source: None,
                    });
                } else if namespace_is(&namespace, NCX_NS)
                    && matches!(name.as_ref(), b"navInfo" | b"navLabel")
                    && nav_map_depth == Some(depth - 1)
                    && points.is_empty()
                    && result.is_empty()
                {
                    ignored_depth = Some(depth);
                } else if namespace_is(&namespace, NCX_NS)
                    && name.as_ref() == b"navLabel"
                    && nav_map_depth.is_some()
                {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if depth != point.depth + 1
                        || point.label_depth.is_some()
                        || point.content_depth.is_some()
                        || point.source.is_some()
                    {
                        return Err(ImportError::InvalidXml);
                    }
                    if point.label.is_some() {
                        ignored_depth = Some(depth);
                    } else {
                        point.label_depth = Some(depth);
                        point.text_seen = false;
                        point.label_candidate.clear();
                    }
                } else if namespace_is(&namespace, NCX_NS)
                    && name.as_ref() == b"text"
                    && nav_map_depth.is_some()
                {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if point.label_depth != Some(depth - 1) || point.text_seen {
                        return Err(ImportError::InvalidXml);
                    }
                    point.text_seen = true;
                    point.text_depth = Some(depth);
                } else if namespace_is(&namespace, NCX_NS)
                    && name.as_ref() == b"content"
                    && nav_map_depth.is_some()
                {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if depth != point.depth + 1
                        || point.label_depth.is_some()
                        || point.label.is_none()
                    {
                        return Err(ImportError::InvalidXml);
                    }
                    set_ncx_source(reader.decoder(), &event, point)?;
                    point.content_depth = Some(depth);
                } else if nav_map_depth.is_some() {
                    return Err(ImportError::InvalidXml);
                }
            }
            Event::Empty(event) => {
                let event_depth = next_xml_depth(depth)?;
                let name = event.local_name();
                if matches!(name.as_ref(), b"audio" | b"img") {
                    return Err(ImportError::InvalidXml);
                }
                if ignored_depth.is_some() {
                    continue;
                }
                if namespace_is(&namespace, NCX_NS) && name.as_ref() == b"content" {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if event_depth != point.depth + 1
                        || point.label_depth.is_some()
                        || point.label.is_none()
                    {
                        return Err(ImportError::InvalidXml);
                    }
                    set_ncx_source(reader.decoder(), &event, point)?;
                } else if event_depth == 1 && name.as_ref() == b"ncx" {
                    return Err(ImportError::UnsupportedEpub);
                } else if event_depth == 2 {
                    require_namespace(&namespace, NCX_NS)?;
                    return Err(ImportError::InvalidXml);
                } else if nav_map_depth.is_some() {
                    return Err(ImportError::InvalidXml);
                }
            }
            Event::Text(text)
                if points
                    .last()
                    .is_some_and(|point| point.text_depth.is_some()) =>
            {
                let value = text.decode().map_err(|_| ImportError::InvalidXml)?;
                push_ncx_label(
                    &mut points.last_mut().expect("checked point").label_candidate,
                    &value,
                )?;
            }
            Event::CData(text)
                if points
                    .last()
                    .is_some_and(|point| point.text_depth.is_some()) =>
            {
                let value = text.decode().map_err(|_| ImportError::InvalidXml)?;
                push_ncx_label(
                    &mut points.last_mut().expect("checked point").label_candidate,
                    &value,
                )?;
            }
            Event::GeneralRef(value) => {
                if let Some(point) = points.last_mut().filter(|point| point.text_depth.is_some()) {
                    push_xml_reference(&mut point.label_candidate, &value, MAX_TOC_LABEL_BYTES)?;
                } else {
                    xml_reference(&value)?;
                }
            }
            Event::End(event) => {
                if let Some(ignored) = ignored_depth {
                    if ignored == depth {
                        ignored_depth = None;
                    }
                    depth = depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
                    continue;
                }
                let name = event.local_name();
                let ncx = namespace_is(&namespace, NCX_NS);
                if ncx && name.as_ref() == b"text" && nav_map_depth.is_some() {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if point.text_depth != Some(depth) {
                        return Err(ImportError::InvalidXml);
                    }
                    point.text_depth = None;
                } else if ncx && name.as_ref() == b"navLabel" && nav_map_depth.is_some() {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if point.label_depth != Some(depth) || point.text_depth.is_some() {
                        return Err(ImportError::InvalidXml);
                    }
                    point.label_depth = None;
                    let label = normalize_metadata(&point.label_candidate);
                    if !label.is_empty() {
                        if label.encode_utf16().count() > 256 {
                            return Err(ImportError::InvalidXml);
                        }
                        point.label = Some(label);
                    }
                    point.label_candidate.clear();
                } else if ncx && name.as_ref() == b"content" && nav_map_depth.is_some() {
                    let point = points.last_mut().ok_or(ImportError::InvalidXml)?;
                    if point.content_depth != Some(depth) {
                        return Err(ImportError::InvalidXml);
                    }
                    point.content_depth = None;
                } else if ncx && name.as_ref() == b"navPoint" {
                    let point = points.pop().ok_or(ImportError::InvalidXml)?;
                    if point.depth != depth
                        || point.label_depth.is_some()
                        || point.text_depth.is_some()
                        || point.content_depth.is_some()
                    {
                        return Err(ImportError::InvalidXml);
                    }
                    let label = point.label.ok_or(ImportError::InvalidXml)?;
                    let raw_href = point.source.ok_or(ImportError::InvalidXml)?;
                    let (path, fragment) = archive::resolve_reference(nav_path, &raw_href)?;
                    if !sections.contains(&path) {
                        return Err(ImportError::UnsupportedEpub);
                    }
                    let href =
                        fragment.map_or_else(|| path.clone(), |value| format!("{path}#{value}"));
                    if !hrefs.insert(href.clone()) {
                        return Err(ImportError::InvalidXml);
                    }
                    result[point.slot] = TocItem { label, href };
                } else if ncx && name.as_ref() == b"navMap" {
                    if nav_map_depth != Some(depth) || !points.is_empty() {
                        return Err(ImportError::InvalidXml);
                    }
                    nav_map_depth = None;
                }
                depth = depth.checked_sub(1).ok_or(ImportError::InvalidXml)?;
            }
            Event::DocType(value) => {
                let value: &[u8] = value.as_ref();
                if depth != 0 || ncx_seen || doctype_seen || value != NCX_DOCTYPE {
                    return Err(ImportError::InvalidXml);
                }
                doctype_seen = true;
            }
            Event::Text(text) if depth == 0 && !xml_whitespace(text.as_ref()) => {
                return Err(ImportError::InvalidXml);
            }
            Event::CData(_) if depth == 0 => return Err(ImportError::InvalidXml),
            Event::Eof
                if depth == 0
                    && nav_map_depth.is_none()
                    && ignored_depth.is_none()
                    && points.is_empty() =>
            {
                break;
            }
            Event::Eof => return Err(ImportError::InvalidXml),
            _ => {}
        }
    }
    if !ncx_seen || nav_map_count != 1 || result.is_empty() {
        return Err(ImportError::UnsupportedEpub);
    }
    Ok(result)
}

fn set_ncx_source(
    decoder: Decoder,
    event: &BytesStart<'_>,
    point: &mut NcxPoint,
) -> Result<(), ImportError> {
    let source = attribute(decoder, event, b"src")?.ok_or(ImportError::InvalidXml)?;
    if point.source.replace(source).is_some() {
        return Err(ImportError::InvalidXml);
    }
    Ok(())
}

fn push_ncx_label(target: &mut String, value: &str) -> Result<(), ImportError> {
    if target.len().saturating_add(value.len()) > MAX_TOC_LABEL_BYTES {
        return Err(ImportError::InvalidXml);
    }
    target.push_str(value);
    Ok(())
}

fn xml_reference(value: &BytesRef<'_>) -> Result<String, ImportError> {
    if let Some(value) = value
        .resolve_char_ref()
        .map_err(|_| ImportError::InvalidXml)?
    {
        return Ok(value.to_string());
    }
    let name = value.decode().map_err(|_| ImportError::InvalidXml)?;
    quick_xml::escape::resolve_predefined_entity(&name)
        .map(str::to_owned)
        .ok_or(ImportError::InvalidXml)
}

fn push_xml_reference(
    target: &mut String,
    value: &BytesRef<'_>,
    limit: usize,
) -> Result<(), ImportError> {
    let value = xml_reference(value)?;
    if target.len().saturating_add(value.len()) > limit {
        return Err(ImportError::InvalidXml);
    }
    target.push_str(&value);
    Ok(())
}

fn is_toc_nav(reader: &NsReader<&[u8]>, event: &BytesStart<'_>) -> Result<bool, ImportError> {
    let epub_type = namespaced_attribute(reader, event, b"type", EPUB_NS)?.unwrap_or_default();
    let role = attribute(reader.decoder(), event, b"role")?.unwrap_or_default();
    Ok(epub_type
        .split_ascii_whitespace()
        .any(|value| value == "toc")
        || role == "doc-toc")
}

fn attribute(
    decoder: Decoder,
    event: &BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, ImportError> {
    let mut found = None;
    for value in event.attributes().with_checks(true) {
        let value = value.map_err(|_| ImportError::InvalidXml)?;
        if value.key.as_ref() == name {
            if found.is_some() {
                return Err(ImportError::InvalidXml);
            }
            found = Some(
                value
                    .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                    .map_err(|_| ImportError::InvalidXml)?
                    .into_owned(),
            );
        }
    }
    Ok(found)
}

fn namespaced_attribute(
    reader: &NsReader<&[u8]>,
    event: &BytesStart<'_>,
    name: &[u8],
    namespace: &[u8],
) -> Result<Option<String>, ImportError> {
    let mut found = None;
    for value in event.attributes().with_checks(true) {
        let value = value.map_err(|_| ImportError::InvalidXml)?;
        let (resolved, local) = reader.resolver().resolve_attribute(value.key);
        if local.as_ref() == name && namespace_is(&resolved, namespace) {
            if found.is_some() {
                return Err(ImportError::InvalidXml);
            }
            found = Some(
                value
                    .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                    .map_err(|_| ImportError::InvalidXml)?
                    .into_owned(),
            );
        }
    }
    Ok(found)
}

fn require_namespace(value: &ResolveResult<'_>, expected: &[u8]) -> Result<(), ImportError> {
    if namespace_is(value, expected) {
        Ok(())
    } else {
        Err(ImportError::UnsupportedEpub)
    }
}

fn namespace_is(value: &ResolveResult<'_>, expected: &[u8]) -> bool {
    matches!(value, ResolveResult::Bound(namespace) if namespace.as_ref() == expected)
}

fn xml_whitespace(value: &[u8]) -> bool {
    value
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
}

fn next_xml_depth(depth: usize) -> Result<usize, ImportError> {
    let depth = depth.checked_add(1).ok_or(ImportError::InvalidXml)?;
    if depth > MAX_XML_DEPTH {
        return Err(ImportError::InvalidXml);
    }
    Ok(depth)
}

fn resource_type(media_type: &str, path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    match media_type {
        "text/css" => path.ends_with(".css"),
        "image/svg+xml" => path.ends_with(".svg"),
        "image/png" => path.ends_with(".png"),
        "image/jpeg" => path.ends_with(".jpg") || path.ends_with(".jpeg"),
        "image/gif" => path.ends_with(".gif"),
        "image/webp" => path.ends_with(".webp"),
        _ => false,
    }
}
