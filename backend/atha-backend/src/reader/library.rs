//! Content-addressed local library over imported reader roots.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use imagesize::ImageType;
use serde::{Deserialize, Serialize};

use crate::{create_real_directory, is_reparse_point};

use super::{
    cbz,
    classification::{self, SourceClassification},
    epub::{self, ImportError, READER_MANIFEST, import_epub},
    fb2, kindle,
    resources::{BookRoot, MAX_RESOURCE_BYTES, Resource, ResourceError},
    text,
};

const LIBRARY_SCHEMA: u8 = 1;
const MAX_TITLE_CHARS: usize = 512;
const MAX_AUTHOR_CHARS: usize = 512;
const MAX_AUTHORS: usize = 16;
const BOOK_METADATA: &str = ".atha-book.json";
pub(crate) const BOOK_EXTENSIONS: [&str; 10] = [
    "epub", "cbz", "fb2", "fbz", "mobi", "azw", "azw3", "md", "markdown", "txt",
];
const DELETE_PREFIX: &str = ".atha-delete-";
const CUSTOM_COVER_SUFFIX: &str = ".cover";
const CUSTOM_COVER_PREVIOUS_SUFFIX: &str = ".cover.previous";
const CUSTOM_COVER_STAGING_PREFIX: &str = ".cover.staging-";
const MAX_COVER_SIDE: usize = 8_192;
const MAX_COVER_PIXELS: usize = 20_000_000;
static NEXT_COVER_TEMPORARY: AtomicU64 = AtomicU64::new(0);
pub use super::classification::BookOrganization;
pub(crate) use super::classification::{BOOK_ORGANIZATION_DIR, MAX_ORGANIZATION_BYTES};
pub const MAX_CUSTOM_COVER_BYTES: u64 = MAX_RESOURCE_BYTES;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBook {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub has_cover: bool,
    pub has_custom_cover: bool,
    pub imported_at: u64,
    pub prepared: bool,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub subjects: Vec<String>,
    pub series: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingBookDeletion {
    pub id: String,
}

#[derive(Debug)]
pub struct OpenedBook {
    pub book: LibraryBook,
    pub root: BookRoot,
}

#[derive(Clone, Debug)]
pub struct LocalLibrary {
    records: PathBuf,
    imports: PathBuf,
    sources: PathBuf,
    organization: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredBook {
    schema: u8,
    id: String,
    title: String,
    authors: Vec<String>,
    cover_path: Option<String>,
    imported_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredOrganization {
    schema: u8,
    id: String,
    imported_at: u64,
    organization: BookOrganization,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_classification: Option<SourceClassification>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ImportedMetadata {
    schema: u8,
    content_version: String,
    title: Option<String>,
    authors: Vec<String>,
    cover_path: Option<String>,
}

struct ImportedSource {
    content_version: String,
    title: Option<String>,
    authors: Vec<String>,
    cover_path: Option<String>,
}

type DurableLibraryState = (
    Vec<StoredBook>,
    Vec<(PathBuf, String)>,
    Vec<(PathBuf, String)>,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryError {
    InvalidRoot,
    InvalidBookId,
    UnknownBook,
    CorruptRecord,
    MissingSource,
    MissingCover,
    InvalidCover,
    InvalidOrganization,
    ReadFailed,
    WriteFailed,
    UnsupportedSource,
    Import(ImportError),
    Cbz(cbz::ImportError),
    Fb2(fb2::ImportError),
    Kindle(kindle::ImportError),
    Text(text::ImportError),
    Resource(ResourceError),
}

impl LocalLibrary {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let root = data_root.as_ref();
        let records = root.join("Library");
        let imports = root.join("ImportedBooks");
        let sources = root.join("SourceBooks");
        let organization = root.join(BOOK_ORGANIZATION_DIR);
        create_real_directory(root).map_err(|_| LibraryError::InvalidRoot)?;
        create_real_directory(&records).map_err(|_| LibraryError::InvalidRoot)?;
        create_real_directory(&imports).map_err(|_| LibraryError::InvalidRoot)?;
        create_real_directory(&sources).map_err(|_| LibraryError::InvalidRoot)?;
        let library = Self {
            records,
            imports,
            sources,
            organization,
        };
        library.recover_custom_covers()?;
        library.recover_deletions()?;
        Ok(library)
    }

    pub fn list(&self) -> Result<Vec<LibraryBook>, LibraryError> {
        let mut books = Vec::new();
        for entry in fs::read_dir(&self.records).map_err(|_| LibraryError::ReadFailed)? {
            let entry = entry.map_err(|_| LibraryError::ReadFailed)?;
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let record = read_record(&path)?;
            books.push(self.public_book(&record));
        }
        books.sort_by(|left, right| {
            right
                .imported_at
                .cmp(&left.imported_at)
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(books)
    }

    pub fn import(&self, source: impl AsRef<Path>) -> Result<LibraryBook, LibraryError> {
        self.import_with_title_hint(source, None)
    }

    pub fn import_with_title_hint(
        &self,
        source: impl AsRef<Path>,
        title_hint: Option<&str>,
    ) -> Result<LibraryBook, LibraryError> {
        let source = source.as_ref();
        let imported = import_source(source, &self.imports)?;
        let path = self.record_path(&imported.content_version)?;
        if path.exists() {
            return Ok(self.public_book(&read_record(&path)?));
        }
        let title = imported
            .title
            .as_deref()
            .map(normalize_text)
            .filter(|value| !value.is_empty())
            .or_else(|| title_hint.map(normalize_text))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                source
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(normalize_text)
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "未命名书籍".into());
        let record = StoredBook {
            schema: LIBRARY_SCHEMA,
            id: imported.content_version,
            title: truncate(&title, MAX_TITLE_CHARS),
            authors: imported
                .authors
                .iter()
                .map(|value| truncate(&normalize_text(value), MAX_AUTHOR_CHARS))
                .filter(|value| !value.is_empty())
                .take(MAX_AUTHORS)
                .collect(),
            cover_path: imported.cover_path,
            imported_at: now_millis()?,
            source_path: None,
        };
        validate_record(&record)?;
        write_record(&path, &record)?;
        Ok(self.public_book(&record))
    }

    pub fn stage_with_title_hint(
        &self,
        source: impl AsRef<Path>,
        title_hint: Option<&str>,
    ) -> Result<LibraryBook, LibraryError> {
        let source = source.as_ref();
        let extension = source_extension(source)?;
        let temporary = copy_source(source, &self.sources, extension)?;
        let result = (|| {
            let content_version = source_identity(&temporary, extension)?;
            let source_name = format!("{content_version}.{extension}");
            let stored_source = self.sources.join(&source_name);
            if stored_source.is_file()
                && source_identity(&stored_source, extension)
                    .is_ok_and(|existing| existing == content_version)
            {
                fs::remove_file(&temporary).map_err(|_| LibraryError::WriteFailed)?;
            } else if stored_source.exists() && !stored_source.is_file() {
                return Err(LibraryError::WriteFailed);
            } else {
                fs::rename(&temporary, &stored_source).map_err(|_| LibraryError::WriteFailed)?;
            }

            let path = self.record_path(&content_version)?;
            if path.exists() {
                match read_record(&path) {
                    Ok(record) => return Ok(self.public_book(&record)),
                    Err(LibraryError::CorruptRecord) => {
                        fs::remove_file(&path).map_err(|_| LibraryError::WriteFailed)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            let title = title_hint
                .map(normalize_text)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    source
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .map(normalize_text)
                })
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "未命名书籍".into());
            let record = StoredBook {
                schema: LIBRARY_SCHEMA,
                id: content_version,
                title: truncate(&title, MAX_TITLE_CHARS),
                authors: Vec::new(),
                cover_path: None,
                imported_at: now_millis()?,
                source_path: Some(source_name),
            };
            validate_record(&record)?;
            write_record(&path, &record)?;
            Ok(self.public_book(&record))
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }

    pub fn open_book(&self, id: &str) -> Result<OpenedBook, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
        let root_path = self.imports.join(id);
        let source_paths = self.source_paths(&record);
        let cached = complete_any_import_cache(&root_path, id);
        let upgrade_epub =
            cached && !source_paths.is_empty() && epub::needs_upgrade(&root_path, id);
        let cached_root = if cached && !upgrade_epub {
            open_root(&root_path)
        } else {
            Err(LibraryError::CorruptRecord)
        };
        let root = match cached_root {
            Ok(root) => root,
            Err(error) => {
                if source_paths.is_empty() {
                    return Err(error);
                }
                let mut rebuilt = None;
                let mut last_error = None;
                for source_path in &source_paths {
                    let source = self.sources.join(source_path);
                    let Ok(extension) = source_extension(&source) else {
                        continue;
                    };
                    if source_identity(&source, extension)
                        .ok()
                        .is_none_or(|identity| identity != record.id)
                    {
                        continue;
                    }
                    match import_source(&source, &self.imports) {
                        Ok(imported) if imported.content_version == record.id => {
                            rebuilt = Some(open_root(&root_path));
                            break;
                        }
                        Ok(_) => last_error = Some(LibraryError::CorruptRecord),
                        Err(error) => last_error = Some(error),
                    }
                }
                rebuilt.unwrap_or_else(|| {
                    if cached {
                        open_root(&root_path)
                    } else {
                        Err(last_error.unwrap_or(LibraryError::CorruptRecord))
                    }
                })?
            }
        };
        self.enrich_cached_classification(&record.id, &source_paths);
        Ok(OpenedBook {
            book: self.public_book(&record),
            root,
        })
    }

    pub fn update_book_organization(
        &self,
        id: &str,
        organization: BookOrganization,
    ) -> Result<LibraryBook, LibraryError> {
        let path = self.record_path(id)?;
        self.ensure_roots()?;
        let record = read_record(&path)?;
        let update = organization
            .normalized()
            .map_err(|()| LibraryError::InvalidOrganization)?;
        if update != BookOrganization::default() {
            let mut data = self
                .read_organization(&record)?
                .unwrap_or_else(|| StoredOrganization::from_book(&record));
            let previous = data.organization.clone();
            data.organization.update(update);
            if data.organization != previous {
                create_real_directory(&self.organization).map_err(|_| LibraryError::WriteFailed)?;
                classification::write_json(&self.organization_path(id), &data)
                    .map_err(|_| LibraryError::WriteFailed)?;
                sync_directory(&self.organization)?;
                sync_directory(
                    self.organization
                        .parent()
                        .ok_or(LibraryError::InvalidRoot)?,
                )?;
            }
        }
        Ok(self.public_book(&record))
    }

    pub fn cover(&self, id: &str) -> Result<Resource, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
        let custom = self.custom_cover_path(id)?;
        if custom.is_file() {
            return read_custom_cover(&custom);
        }
        let path = read_imported_metadata(&self.imports.join(id), id)
            .and_then(|metadata| metadata.cover_path)
            .or(record.cover_path)
            .ok_or(LibraryError::MissingCover)?;
        let root = BookRoot::new(self.imports.join(id)).map_err(LibraryError::Resource)?;
        let resource = root
            .read(&format!("/{path}"))
            .map_err(LibraryError::Resource)?;
        if !resource.content_type.starts_with("image/") {
            return Err(LibraryError::MissingCover);
        }
        Ok(resource)
    }

    pub fn set_custom_cover(
        &self,
        id: &str,
        source: impl AsRef<Path>,
    ) -> Result<LibraryBook, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
        let bytes = read_custom_cover_bytes(source.as_ref())?;
        validate_custom_cover(&bytes)?;
        let target = self.custom_cover_path(id)?;
        let previous = self.custom_cover_previous_path(id)?;
        if (target.exists() && !custom_cover_present(&target)) || previous.exists() {
            return Err(LibraryError::WriteFailed);
        }
        let temporary = self.reserve_cover_temporary()?;
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| LibraryError::WriteFailed)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| LibraryError::WriteFailed)?;
            if target.is_file() {
                fs::rename(&target, &previous).map_err(|_| LibraryError::WriteFailed)?;
            }
            if fs::rename(&temporary, &target).is_err() {
                if previous.is_file() {
                    let _ = fs::rename(&previous, &target);
                }
                return Err(LibraryError::WriteFailed);
            }
            sync_directory(&self.records)?;
            remove_path_if_exists(&previous)?;
            sync_directory(&self.records)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        Ok(self.public_book(&record))
    }

    pub fn reset_custom_cover(&self, id: &str) -> Result<LibraryBook, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
        remove_path_if_exists(&self.custom_cover_path(id)?)?;
        sync_directory(&self.records)?;
        Ok(self.public_book(&record))
    }

    pub fn remove(&self, id: &str) -> Result<(), LibraryError> {
        let path = self.record_path(id)?;
        if !path.is_file() {
            return Err(LibraryError::UnknownBook);
        }
        fs::remove_file(path).map_err(|_| LibraryError::WriteFailed)?;
        sync_directory(&self.records)?;
        self.remove_organization(id)?;
        remove_path_if_exists(&self.custom_cover_path(id)?)?;
        remove_path_if_exists(&self.custom_cover_previous_path(id)?)?;
        sync_directory(&self.records)
    }

    pub fn prepare_local_data_deletion(
        &self,
        id: &str,
    ) -> Result<PendingBookDeletion, LibraryError> {
        let record = self.record_path(id)?;
        if !record.is_file() {
            return Err(LibraryError::UnknownBook);
        }
        let intent = self.deletion_intent(id)?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&intent)
            .and_then(|file| file.sync_all())
            .map_err(|_| LibraryError::WriteFailed)?;
        sync_directory(self.records.parent().ok_or(LibraryError::InvalidRoot)?)?;
        self.remove_book_files(id)?;
        Ok(PendingBookDeletion { id: id.into() })
    }

    pub fn finish_local_data_deletion(&self, id: &str) -> Result<(), LibraryError> {
        let intent = self.deletion_intent(id)?;
        if !intent.is_file() {
            return Err(LibraryError::UnknownBook);
        }
        self.remove_book_files(id)?;
        remove_path_if_exists(&intent)?;
        sync_directory(self.records.parent().ok_or(LibraryError::InvalidRoot)?)
    }

    pub fn pending_local_data_deletions(&self) -> Result<Vec<PendingBookDeletion>, LibraryError> {
        let mut deletions = Vec::new();
        let root = self.records.parent().ok_or(LibraryError::InvalidRoot)?;
        for entry in fs::read_dir(root).map_err(|_| LibraryError::ReadFailed)? {
            let entry = entry.map_err(|_| LibraryError::ReadFailed)?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|value| value.strip_prefix(DELETE_PREFIX))
            else {
                continue;
            };
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LibraryError::ReadFailed)?;
            if !metadata.file_type().is_file() || !valid_id(id) {
                return Err(LibraryError::CorruptRecord);
            }
            deletions.push(PendingBookDeletion { id: id.into() });
        }
        deletions.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(deletions)
    }

    pub fn resume_local_data_deletion(&self, id: &str) -> Result<(), LibraryError> {
        if !self.deletion_intent(id)?.is_file() {
            return Err(LibraryError::UnknownBook);
        }
        self.remove_book_files(id)
    }

    pub(crate) fn write_backup_state(
        &self,
        target_root: impl AsRef<Path>,
    ) -> Result<(), LibraryError> {
        let target_records = target_root.as_ref().join("Library");
        let target_sources = target_root.as_ref().join("SourceBooks");
        let target_organization = target_root.as_ref().join(BOOK_ORGANIZATION_DIR);
        fs::create_dir(&target_records).map_err(|_| LibraryError::WriteFailed)?;
        fs::create_dir(&target_sources).map_err(|_| LibraryError::WriteFailed)?;
        fs::create_dir(&target_organization).map_err(|_| LibraryError::WriteFailed)?;
        let (mut records, sources, covers) = self.durable_state()?;
        let mut organizations = self.durable_organizations(&records, false)?;
        let source_ids = sources
            .iter()
            .filter_map(|(_, name)| name.split_once('.').map(|(id, _)| id))
            .collect::<HashSet<_>>();
        if records
            .iter()
            .any(|record| !source_ids.contains(record.id.as_str()))
        {
            return Err(LibraryError::MissingSource);
        }
        for record in &mut records {
            let root = self.imports.join(&record.id);
            let mut organization = organizations
                .remove(&record.id)
                .unwrap_or_else(|| StoredOrganization::from_book(record));
            if let Some(imported) = read_imported_metadata(&root, &record.id) {
                if let Some(title) = imported
                    .title
                    .as_deref()
                    .map(normalize_text)
                    .filter(|value| !value.is_empty())
                {
                    record.title = truncate(&title, MAX_TITLE_CHARS);
                }
                let authors = imported
                    .authors
                    .iter()
                    .map(|value| truncate(&normalize_text(value), MAX_AUTHOR_CHARS))
                    .filter(|value| !value.is_empty())
                    .take(MAX_AUTHORS)
                    .collect::<Vec<_>>();
                if !authors.is_empty() {
                    record.authors = authors;
                }
                record.cover_path = imported.cover_path;
                if prepared_cache(&root, record)
                    && let Some(classification) =
                        classification::read_cache(&self.imports, &record.id)
                    && !classification.is_empty()
                {
                    organization.source_classification = Some(classification);
                }
            }
            write_record(&target_records.join(format!("{}.json", record.id)), record)?;
            if organization.organization != BookOrganization::default()
                || organization.source_classification.is_some()
            {
                classification::write_json(
                    &target_organization.join(format!("{}.json", record.id)),
                    &organization,
                )
                .map_err(|_| LibraryError::WriteFailed)?;
            }
        }
        for (source, name) in sources {
            fs::copy(source, target_sources.join(name)).map_err(|_| LibraryError::WriteFailed)?;
        }
        for (cover, name) in covers {
            fs::copy(cover, target_records.join(name)).map_err(|_| LibraryError::WriteFailed)?;
        }
        Ok(())
    }

    pub(crate) fn validate_durable_state(&self) -> Result<(), LibraryError> {
        let (records, sources, _) = self.durable_state()?;
        self.durable_organizations(&records, true)?;
        let source_ids = sources
            .iter()
            .filter_map(|(_, name)| name.split_once('.').map(|(id, _)| id))
            .collect::<HashSet<_>>();
        if records
            .iter()
            .any(|record| !source_ids.contains(record.id.as_str()))
        {
            Err(LibraryError::MissingSource)
        } else {
            Ok(())
        }
    }

    fn record_path(&self, id: &str) -> Result<PathBuf, LibraryError> {
        if !valid_id(id) {
            return Err(LibraryError::InvalidBookId);
        }
        Ok(self.records.join(format!("{id}.json")))
    }

    fn custom_cover_path(&self, id: &str) -> Result<PathBuf, LibraryError> {
        if !valid_id(id) {
            return Err(LibraryError::InvalidBookId);
        }
        Ok(self.records.join(format!("{id}{CUSTOM_COVER_SUFFIX}")))
    }

    fn custom_cover_previous_path(&self, id: &str) -> Result<PathBuf, LibraryError> {
        if !valid_id(id) {
            return Err(LibraryError::InvalidBookId);
        }
        Ok(self
            .records
            .join(format!("{id}{CUSTOM_COVER_PREVIOUS_SUFFIX}")))
    }

    fn reserve_cover_temporary(&self) -> Result<PathBuf, LibraryError> {
        for _ in 0..64 {
            let sequence = NEXT_COVER_TEMPORARY.fetch_add(1, Ordering::Relaxed);
            let path = self.records.join(format!(
                "{CUSTOM_COVER_STAGING_PREFIX}{}-{sequence}",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(path);
            }
        }
        Err(LibraryError::WriteFailed)
    }

    fn public_book(&self, record: &StoredBook) -> LibraryBook {
        let root = self.imports.join(&record.id);
        let imported = read_imported_metadata(&root, &record.id);
        let prepared = imported.is_some() && prepared_cache(&root, record);
        let imported_classification = prepared
            .then(|| classification::read_cache(&self.imports, &record.id))
            .flatten();
        let organization = self.read_organization(record).ok().flatten();
        record.public(
            imported.as_ref(),
            prepared,
            custom_cover_present(
                &self
                    .records
                    .join(format!("{}{}", record.id, CUSTOM_COVER_SUFFIX)),
            ),
            imported_classification,
            organization,
        )
    }

    fn enrich_cached_classification(&self, id: &str, sources: &[String]) {
        if !sources.iter().any(|source| {
            Path::new(source)
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| matches!(extension, "epub" | "cbz" | "fb2" | "fbz"))
        }) {
            return;
        }
        if classification::read_cache(&self.imports, id).is_some() {
            return;
        }
        let _lock = super::lock_import();
        if classification::read_cache(&self.imports, id).is_some() {
            return;
        }
        let classification = sources.iter().find_map(|name| {
            let path = self.sources.join(name);
            if !fs::symlink_metadata(&path)
                .is_ok_and(|entry| entry.file_type().is_file() && !is_reparse_point(&entry))
            {
                return None;
            }
            match source_extension(&path).ok()? {
                "epub" => epub::read_source_classification(&path, id).ok(),
                "cbz" => cbz::read_source_classification(&path, id).ok(),
                "fb2" | "fbz" => fb2::read_source_classification(&path, id).ok(),
                _ => None,
            }
        });
        if let Some(classification) = classification {
            // Optional cache enrichment must not make a previously readable book fail to open.
            let _ = classification::write_cache(&self.imports, id, classification);
        }
    }

    fn source_paths(&self, record: &StoredBook) -> Vec<String> {
        let mut paths = record
            .source_path
            .iter()
            .filter(|name| self.sources.join(name).is_file())
            .cloned()
            .collect::<Vec<_>>();
        for extension in BOOK_EXTENSIONS {
            let name = format!("{}.{extension}", record.id);
            if self.sources.join(&name).is_file() && !paths.contains(&name) {
                paths.push(name);
            }
        }
        paths
    }

    fn source_paths_for_id(&self, id: &str) -> Vec<PathBuf> {
        BOOK_EXTENSIONS
            .into_iter()
            .map(|extension| self.sources.join(format!("{id}.{extension}")))
            .filter(|path| path.is_file())
            .collect()
    }

    fn deletion_intent(&self, id: &str) -> Result<PathBuf, LibraryError> {
        if !valid_id(id) {
            return Err(LibraryError::InvalidBookId);
        }
        Ok(self
            .records
            .parent()
            .ok_or(LibraryError::InvalidRoot)?
            .join(format!("{DELETE_PREFIX}{id}")))
    }

    fn recover_deletions(&self) -> Result<(), LibraryError> {
        let root = self.records.parent().ok_or(LibraryError::InvalidRoot)?;
        for entry in fs::read_dir(root).map_err(|_| LibraryError::InvalidRoot)? {
            let entry = entry.map_err(|_| LibraryError::InvalidRoot)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LibraryError::InvalidRoot)?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_prefix(DELETE_PREFIX))
            else {
                continue;
            };
            if !metadata.file_type().is_file() || !valid_id(id) {
                return Err(LibraryError::InvalidRoot);
            }
            self.remove_book_files(id)?;
        }
        Ok(())
    }

    fn recover_custom_covers(&self) -> Result<(), LibraryError> {
        for entry in fs::read_dir(&self.records).map_err(|_| LibraryError::InvalidRoot)? {
            let entry = entry.map_err(|_| LibraryError::InvalidRoot)?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or(LibraryError::InvalidRoot)?
                .to_owned();
            if name.starts_with(CUSTOM_COVER_STAGING_PREFIX) {
                remove_path_if_exists(&entry.path())?;
                continue;
            }
            let Some(id) = name.strip_suffix(CUSTOM_COVER_PREVIOUS_SUFFIX) else {
                continue;
            };
            if !valid_id(id) {
                return Err(LibraryError::InvalidRoot);
            }
            let target = self.custom_cover_path(id)?;
            if target.is_file() {
                remove_path_if_exists(&entry.path())?;
            } else {
                fs::rename(entry.path(), target).map_err(|_| LibraryError::WriteFailed)?;
            }
        }
        for entry in fs::read_dir(&self.records).map_err(|_| LibraryError::InvalidRoot)? {
            let entry = entry.map_err(|_| LibraryError::InvalidRoot)?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or(LibraryError::InvalidRoot)?
                .to_owned();
            let Some(id) = name.strip_suffix(CUSTOM_COVER_SUFFIX) else {
                continue;
            };
            if valid_id(id) && !self.record_path(id)?.is_file() {
                remove_path_if_exists(&entry.path())?;
            }
        }
        sync_directory(&self.records)
    }

    fn remove_book_files(&self, id: &str) -> Result<(), LibraryError> {
        self.ensure_roots()?;
        remove_path_if_exists(&self.imports.join(id))?;
        remove_adjunct(&classification::cache_path(&self.imports, id))?;
        for path in self.source_paths_for_id(id) {
            remove_path_if_exists(&path)?;
        }
        remove_path_if_exists(&self.custom_cover_path(id)?)?;
        remove_path_if_exists(&self.custom_cover_previous_path(id)?)?;
        remove_path_if_exists(&self.record_path(id)?)?;
        self.remove_organization(id)?;
        sync_directory(&self.imports)?;
        sync_directory(&self.sources)?;
        sync_directory(&self.records)
    }

    fn durable_state(&self) -> Result<DurableLibraryState, LibraryError> {
        self.ensure_roots()?;
        let mut records = Vec::new();
        let mut covers = Vec::new();
        for entry in fs::read_dir(&self.records).map_err(|_| LibraryError::ReadFailed)? {
            let entry = entry.map_err(|_| LibraryError::ReadFailed)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LibraryError::ReadFailed)?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or(LibraryError::CorruptRecord)?
                .to_owned();
            if record_temporary(&name) {
                continue;
            }
            if let Some(id) = name.strip_suffix(CUSTOM_COVER_SUFFIX) {
                if !valid_id(id)
                    || !metadata.file_type().is_file()
                    || is_reparse_point(&metadata)
                    || read_custom_cover(&entry.path()).is_err()
                {
                    return Err(LibraryError::CorruptRecord);
                }
                covers.push((entry.path(), name));
                continue;
            }
            if !metadata.file_type().is_file()
                || is_reparse_point(&metadata)
                || entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "json")
            {
                return Err(LibraryError::CorruptRecord);
            }
            records.push(read_record(&entry.path())?);
        }
        records.sort_by(|left, right| left.id.cmp(&right.id));
        let record_ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<HashSet<_>>();
        if covers.iter().any(|(_, name)| {
            name.strip_suffix(CUSTOM_COVER_SUFFIX)
                .is_none_or(|id| !record_ids.contains(id))
        }) {
            return Err(LibraryError::CorruptRecord);
        }
        covers.sort_by(|left, right| left.1.cmp(&right.1));

        let mut sources = Vec::new();
        for entry in fs::read_dir(&self.sources).map_err(|_| LibraryError::ReadFailed)? {
            let entry = entry.map_err(|_| LibraryError::ReadFailed)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LibraryError::ReadFailed)?;
            let name = entry
                .file_name()
                .to_str()
                .ok_or(LibraryError::CorruptRecord)?
                .to_owned();
            if name.starts_with(".source.staging-") {
                continue;
            }
            let (id, extension) = name.rsplit_once('.').ok_or(LibraryError::CorruptRecord)?;
            if !metadata.file_type().is_file()
                || is_reparse_point(&metadata)
                || !valid_source_path(&name, id)
                || source_identity(&entry.path(), extension)? != id
            {
                return Err(LibraryError::CorruptRecord);
            }
            sources.push((entry.path(), name));
        }
        sources.sort_by(|left, right| left.1.cmp(&right.1));
        Ok((records, sources, covers))
    }

    fn organization_path(&self, id: &str) -> PathBuf {
        self.organization.join(format!("{id}.json"))
    }

    fn has_organization_directory(&self) -> Result<bool, LibraryError> {
        match fs::symlink_metadata(&self.organization) {
            Ok(entry) if entry.file_type().is_dir() && !is_reparse_point(&entry) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            _ => Err(LibraryError::CorruptRecord),
        }
    }

    fn read_organization(
        &self,
        book: &StoredBook,
    ) -> Result<Option<StoredOrganization>, LibraryError> {
        if !self.has_organization_directory()? {
            return Ok(None);
        }
        Ok(
            read_stored_organization(&self.organization_path(&book.id), &book.id)?
                .filter(|data| data.imported_at == book.imported_at),
        )
    }

    fn remove_organization(&self, id: &str) -> Result<(), LibraryError> {
        if self.has_organization_directory()? {
            remove_adjunct(&self.organization_path(id))?;
            sync_directory(&self.organization)?;
        }
        Ok(())
    }

    fn durable_organizations(
        &self,
        records: &[StoredBook],
        reject_stale: bool,
    ) -> Result<HashMap<String, StoredOrganization>, LibraryError> {
        let mut result = HashMap::new();
        if !self.has_organization_directory()? {
            return Ok(result);
        }
        let generations = records
            .iter()
            .map(|book| (book.id.as_str(), book.imported_at))
            .collect::<HashMap<_, _>>();
        for entry in fs::read_dir(&self.organization).map_err(|_| LibraryError::ReadFailed)? {
            let entry = entry.map_err(|_| LibraryError::ReadFailed)?;
            let name = entry.file_name();
            let name = name.to_str().ok_or(LibraryError::CorruptRecord)?;
            if classification::is_temporary_file(name) {
                continue;
            }
            let id = name
                .strip_suffix(".json")
                .filter(|id| valid_id(id))
                .ok_or(LibraryError::CorruptRecord)?;
            if !reject_stale && !generations.contains_key(id) {
                continue;
            }
            let data =
                read_stored_organization(&entry.path(), id)?.ok_or(LibraryError::CorruptRecord)?;
            if generations.get(id) == Some(&data.imported_at) {
                result.insert(id.to_owned(), data);
            } else if reject_stale {
                return Err(LibraryError::CorruptRecord);
            }
        }
        Ok(result)
    }

    fn ensure_roots(&self) -> Result<(), LibraryError> {
        let root = self.records.parent().ok_or(LibraryError::InvalidRoot)?;
        if [root, &self.records, &self.imports, &self.sources]
            .into_iter()
            .all(real_directory)
        {
            Ok(())
        } else {
            Err(LibraryError::InvalidRoot)
        }
    }
}

impl StoredOrganization {
    fn from_book(book: &StoredBook) -> Self {
        Self {
            schema: 1,
            id: book.id.clone(),
            imported_at: book.imported_at,
            organization: BookOrganization::default(),
            source_classification: None,
        }
    }
}

impl StoredBook {
    fn public(
        &self,
        imported: Option<&ImportedMetadata>,
        prepared: bool,
        has_custom_cover: bool,
        source: Option<SourceClassification>,
        stored: Option<StoredOrganization>,
    ) -> LibraryBook {
        let imported_title = imported
            .and_then(|metadata| metadata.title.as_deref())
            .map(normalize_text)
            .filter(|value| !value.is_empty());
        let imported_authors = imported.map(|metadata| {
            metadata
                .authors
                .iter()
                .map(|value| truncate(&normalize_text(value), MAX_AUTHOR_CHARS))
                .filter(|value| !value.is_empty())
                .take(MAX_AUTHORS)
                .collect::<Vec<_>>()
        });
        let (organization, stored_source) = stored
            .map(|data| (data.organization, data.source_classification))
            .unwrap_or_default();
        let source = source.or(stored_source).unwrap_or_default();
        LibraryBook {
            id: self.id.clone(),
            title: imported_title.unwrap_or_else(|| self.title.clone()),
            authors: imported_authors
                .filter(|authors| !authors.is_empty())
                .unwrap_or_else(|| self.authors.clone()),
            has_cover: has_custom_cover
                || (prepared
                    && imported
                        .and_then(|metadata| metadata.cover_path.as_ref())
                        .or(self.cover_path.as_ref())
                        .is_some()),
            has_custom_cover,
            imported_at: self.imported_at,
            prepared,
            favorite: organization.favorite.unwrap_or(false),
            tags: organization.tags.unwrap_or(source.tags),
            subjects: organization.subjects.unwrap_or(source.subjects),
            series: organization.series.unwrap_or(source.series),
        }
    }
}

impl LibraryError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid-library-root",
            Self::InvalidBookId => "invalid-library-book-id",
            Self::UnknownBook => "unknown-library-book",
            Self::CorruptRecord => "corrupt-library-record",
            Self::MissingSource => "missing-library-source",
            Self::MissingCover => "missing-library-cover",
            Self::InvalidCover => "invalid-library-cover",
            Self::InvalidOrganization => "invalid-library-organization",
            Self::ReadFailed => "library-read-failed",
            Self::WriteFailed => "library-write-failed",
            Self::UnsupportedSource => "invalid-library-source",
            Self::Import(error) => error.code(),
            Self::Cbz(error) => error.code(),
            Self::Fb2(error) => error.code(),
            Self::Kindle(error) => error.code(),
            Self::Text(error) => error.code(),
            Self::Resource(error) => error.code(),
        }
    }
}

impl fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for LibraryError {}

fn import_source(source: &Path, imports: &Path) -> Result<ImportedSource, LibraryError> {
    let imported = match source_extension(source)? {
        "epub" => {
            let book = import_epub(source, imports).map_err(LibraryError::Import)?;
            ImportedSource {
                content_version: book.content_version,
                title: book.title,
                authors: book.authors,
                cover_path: book.cover_path,
            }
        }
        "cbz" => {
            let book = cbz::import_cbz(source, imports).map_err(LibraryError::Cbz)?;
            ImportedSource {
                content_version: book.content_version,
                title: book.title,
                authors: book.authors,
                cover_path: book.cover_path,
            }
        }
        "fb2" | "fbz" => {
            let book = fb2::import_fb2(source, imports).map_err(LibraryError::Fb2)?;
            ImportedSource {
                content_version: book.content_version,
                title: book.title,
                authors: book.authors,
                cover_path: book.cover_path,
            }
        }
        "mobi" | "azw" | "azw3" => {
            let book = kindle::import_kindle(source, imports).map_err(LibraryError::Kindle)?;
            ImportedSource {
                content_version: book.content_version,
                title: book.title,
                authors: book.authors,
                cover_path: book.cover_path,
            }
        }
        "md" | "markdown" => {
            let book = text::import_markdown(source, imports).map_err(LibraryError::Text)?;
            ImportedSource {
                content_version: book.content_version,
                title: book.title,
                authors: book.authors,
                cover_path: book.cover_path,
            }
        }
        "txt" => {
            let book = text::import_txt(source, imports).map_err(LibraryError::Text)?;
            ImportedSource {
                content_version: book.content_version,
                title: book.title,
                authors: book.authors,
                cover_path: book.cover_path,
            }
        }
        _ => unreachable!(),
    };
    Ok(imported)
}

fn source_identity(source: &Path, extension: &str) -> Result<String, LibraryError> {
    match extension {
        "epub" => epub::source_identity(source).map_err(LibraryError::Import),
        "cbz" => cbz::source_identity(source).map_err(LibraryError::Cbz),
        "fb2" | "fbz" => fb2::source_identity(source).map_err(LibraryError::Fb2),
        "mobi" | "azw" | "azw3" => kindle::source_identity(source).map_err(LibraryError::Kindle),
        "md" | "markdown" => text::markdown_source_identity(source).map_err(LibraryError::Text),
        "txt" => text::txt_source_identity(source).map_err(LibraryError::Text),
        _ => Err(LibraryError::UnsupportedSource),
    }
}

fn complete_any_import_cache(root: &Path, id: &str) -> bool {
    epub::complete_cache(root, id)
        || cbz::complete_cache(root, id)
        || fb2::complete_cache(root, id)
        || kindle::complete_cache(root, id)
        || text::complete_cache(root, id, "md")
        || text::complete_cache(root, id, "txt")
}

fn source_extension(source: &Path) -> Result<&str, LibraryError> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .ok_or(LibraryError::UnsupportedSource)?;
    for supported in BOOK_EXTENSIONS {
        if extension.eq_ignore_ascii_case(supported) {
            return Ok(supported);
        }
    }
    Err(LibraryError::UnsupportedSource)
}

fn copy_source(source: &Path, sources: &Path, extension: &str) -> Result<PathBuf, LibraryError> {
    let metadata = source.metadata().map_err(|_| LibraryError::ReadFailed)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(LibraryError::UnsupportedSource);
    }
    if metadata.len() > super::MAX_SOURCE_BYTES {
        return Err(LibraryError::UnsupportedSource);
    }
    let (temporary, mut output) = (0..32)
        .find_map(|attempt| {
            let path = sources.join(format!(
                ".source.staging-{}-{attempt}.{extension}",
                std::process::id()
            ));
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .ok()
                .map(|file| (path, file))
        })
        .ok_or(LibraryError::WriteFailed)?;
    let result = (|| {
        let mut input = File::open(source).map_err(|_| LibraryError::ReadFailed)?;
        let copied = std::io::copy(
            &mut Read::take(&mut input, super::MAX_SOURCE_BYTES.saturating_add(1)),
            &mut output,
        )
        .map_err(|_| LibraryError::WriteFailed)?;
        if copied == 0 || copied > super::MAX_SOURCE_BYTES {
            return Err(LibraryError::UnsupportedSource);
        }
        output.sync_all().map_err(|_| LibraryError::WriteFailed)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(temporary)
}

fn remove_path_if_exists(path: &Path) -> Result<(), LibraryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !is_reparse_point(&metadata) => {
            fs::remove_dir_all(path).map_err(|_| LibraryError::WriteFailed)
        }
        Ok(metadata) if metadata.file_type().is_file() => {
            fs::remove_file(path).map_err(|_| LibraryError::WriteFailed)
        }
        Ok(_) => Err(LibraryError::WriteFailed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(LibraryError::WriteFailed),
    }
}

fn custom_cover_present(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_file()
            && !is_reparse_point(&metadata)
            && metadata.len() > 0
            && metadata.len() <= MAX_CUSTOM_COVER_BYTES
    })
}

fn read_custom_cover_bytes(path: &Path) -> Result<Vec<u8>, LibraryError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| LibraryError::InvalidCover)?;
    if !metadata.file_type().is_file()
        || is_reparse_point(&metadata)
        || metadata.len() == 0
        || metadata.len() > MAX_CUSTOM_COVER_BYTES
    {
        return Err(LibraryError::InvalidCover);
    }
    let bytes = fs::read(path).map_err(|_| LibraryError::ReadFailed)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(LibraryError::InvalidCover);
    }
    Ok(bytes)
}

fn validate_custom_cover(bytes: &[u8]) -> Result<&'static str, LibraryError> {
    let content_type = match imagesize::image_type(bytes) {
        Ok(ImageType::Jpeg) => "image/jpeg",
        Ok(ImageType::Png) => "image/png",
        Ok(ImageType::Webp) => "image/webp",
        _ => return Err(LibraryError::InvalidCover),
    };
    let size = imagesize::blob_size(bytes).map_err(|_| LibraryError::InvalidCover)?;
    let pixels = size
        .width
        .checked_mul(size.height)
        .ok_or(LibraryError::InvalidCover)?;
    if size.width == 0
        || size.height == 0
        || size.width > MAX_COVER_SIDE
        || size.height > MAX_COVER_SIDE
        || pixels > MAX_COVER_PIXELS
    {
        return Err(LibraryError::InvalidCover);
    }
    Ok(content_type)
}

fn read_custom_cover(path: &Path) -> Result<Resource, LibraryError> {
    let bytes = read_custom_cover_bytes(path)?;
    let content_type = validate_custom_cover(&bytes)?;
    Ok(Resource {
        bytes,
        content_type,
    })
}

fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !is_reparse_point(&metadata))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), LibraryError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| LibraryError::WriteFailed)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), LibraryError> {
    Ok(())
}

fn open_root(path: &Path) -> Result<BookRoot, LibraryError> {
    let root = BookRoot::new(path).map_err(LibraryError::Resource)?;
    root.read(&format!("/{READER_MANIFEST}"))
        .map_err(LibraryError::Resource)?;
    Ok(root)
}

fn prepared_cache(root: &Path, record: &StoredBook) -> bool {
    root.join(READER_MANIFEST).is_file() && has_import_marker(root, record)
}

fn read_stored_organization(
    path: &Path,
    id: &str,
) -> Result<Option<StoredOrganization>, LibraryError> {
    let data: Option<StoredOrganization> = classification::read_json(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            LibraryError::CorruptRecord
        } else {
            LibraryError::ReadFailed
        }
    })?;
    if data.as_ref().is_some_and(|data| {
        data.schema != 1
            || data.id != id
            || data.imported_at == 0
            || !data.organization.is_valid()
            || data
                .source_classification
                .as_ref()
                .is_some_and(|source| !source.is_valid())
    }) {
        return Err(LibraryError::CorruptRecord);
    }
    Ok(data)
}

fn remove_adjunct(path: &Path) -> Result<(), LibraryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Err(LibraryError::WriteFailed),
        Ok(_) => fs::remove_file(path).map_err(|_| LibraryError::WriteFailed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(LibraryError::WriteFailed),
    }
}

fn read_imported_metadata(path: &Path, id: &str) -> Option<ImportedMetadata> {
    if !real_directory(path) {
        return None;
    }
    let metadata_path = path.join(BOOK_METADATA);
    let file = fs::symlink_metadata(&metadata_path).ok()?;
    if !file.file_type().is_file() || is_reparse_point(&file) {
        return None;
    }
    let metadata: ImportedMetadata = serde_json::from_slice(&fs::read(metadata_path).ok()?).ok()?;
    if metadata.schema != 1
        || metadata.content_version != id
        || metadata
            .title
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.chars().count() > MAX_TITLE_CHARS)
        || metadata.authors.len() > MAX_AUTHORS
        || metadata
            .authors
            .iter()
            .any(|value| value.is_empty() || value.chars().count() > MAX_AUTHOR_CHARS)
        || metadata
            .cover_path
            .as_ref()
            .is_some_and(|value| !valid_cover_path(value))
    {
        return None;
    }
    Some(metadata)
}

fn has_import_marker(path: &Path, record: &StoredBook) -> bool {
    type MarkerCheck = fn(&Path, &str) -> bool;
    const CHECKS: [(&[&str], MarkerCheck); 5] = [
        (&["epub"], epub::has_cache_marker),
        (&["cbz"], cbz::has_cache_marker),
        (&["fb2", "fbz"], fb2::has_cache_marker),
        (&["mobi", "azw", "azw3"], kindle::has_cache_marker),
        (&["md", "markdown", "txt"], text::has_cache_marker),
    ];
    // The record hints at the probe order, not the cache format or source availability.
    // Old records may have no source path; a hint that does not match must not hide a marker.
    let preferred = record
        .source_path
        .as_deref()
        .and_then(|name| name.rsplit_once('.'))
        .and_then(|(_, extension)| {
            CHECKS
                .iter()
                .position(|(extensions, _)| extensions.contains(&extension))
        });
    preferred.is_some_and(|i| CHECKS[i].1(path, &record.id))
        || CHECKS
            .iter()
            .enumerate()
            .any(|(i, (_, check))| Some(i) != preferred && check(path, &record.id))
}

fn read_record(path: &Path) -> Result<StoredBook, LibraryError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            LibraryError::UnknownBook
        } else {
            LibraryError::ReadFailed
        }
    })?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err(LibraryError::UnknownBook);
    }
    let record = serde_json::from_slice::<StoredBook>(
        &fs::read(path).map_err(|_| LibraryError::ReadFailed)?,
    )
    .map_err(|_| LibraryError::CorruptRecord)?;
    validate_record(&record)?;
    if path.file_stem().and_then(|value| value.to_str()) != Some(record.id.as_str()) {
        return Err(LibraryError::CorruptRecord);
    }
    Ok(record)
}

fn write_record(path: &Path, record: &StoredBook) -> Result<(), LibraryError> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| LibraryError::WriteFailed)?;
    let result = (|| {
        serde_json::to_writer_pretty(&mut file, record).map_err(|_| LibraryError::WriteFailed)?;
        file.write_all(b"\n")
            .and_then(|()| file.sync_all())
            .map_err(|_| LibraryError::WriteFailed)?;
        fs::rename(&temporary, path).map_err(|_| LibraryError::WriteFailed)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn validate_record(record: &StoredBook) -> Result<(), LibraryError> {
    if record.schema != LIBRARY_SCHEMA
        || !valid_id(&record.id)
        || record.title.is_empty()
        || record.title.chars().count() > MAX_TITLE_CHARS
        || record.authors.len() > MAX_AUTHORS
        || record
            .authors
            .iter()
            .any(|value| value.is_empty() || value.chars().count() > MAX_AUTHOR_CHARS)
        || record.imported_at == 0
        || record
            .cover_path
            .as_ref()
            .is_some_and(|value| !valid_cover_path(value))
        || record
            .source_path
            .as_ref()
            .is_some_and(|value| !valid_source_path(value, &record.id))
    {
        return Err(LibraryError::CorruptRecord);
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn record_temporary(name: &str) -> bool {
    let Some((id, suffix)) = name.split_once('.') else {
        return false;
    };
    valid_id(id)
        && suffix
            .strip_suffix(".tmp")
            .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit()))
}

fn valid_source_path(value: &str, id: &str) -> bool {
    let Some((stem, extension)) = value.rsplit_once('.') else {
        return false;
    };
    stem == id
        && matches!(
            extension,
            "epub" | "cbz" | "fb2" | "fbz" | "mobi" | "azw" | "azw3" | "md" | "markdown" | "txt"
        )
}

fn valid_cover_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && !value.contains(['\0', '\\', ':', '%', '?', '#'])
        && value
            .split('/')
            .all(|part| !part.is_empty() && !matches!(part, "." | ".."))
        && matches!(
            Path::new(value)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("svg" | "png" | "jpg" | "jpeg" | "gif" | "webp")
        )
}

fn normalize_text(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| {
            if character.is_whitespace() {
                Some(' ')
            } else if character.is_control() {
                None
            } else {
                Some(character)
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn now_millis() -> Result<u64, LibraryError> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LibraryError::WriteFailed)?
        .as_millis();
    u64::try_from(value).map_err(|_| LibraryError::WriteFailed)
}
