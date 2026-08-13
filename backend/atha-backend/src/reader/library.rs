//! Content-addressed local library over imported reader roots.

use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{create_real_directory, is_reparse_point};

use super::{
    cbz,
    epub::{self, ImportError, READER_MANIFEST, import_epub},
    fb2, kindle,
    resources::{BookRoot, Resource, ResourceError},
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBook {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub has_cover: bool,
    pub imported_at: u64,
    pub prepared: bool,
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

type DurableLibraryState = (Vec<StoredBook>, Vec<(PathBuf, String)>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryError {
    InvalidRoot,
    InvalidBookId,
    UnknownBook,
    CorruptRecord,
    MissingSource,
    MissingCover,
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
        create_real_directory(root).map_err(|_| LibraryError::InvalidRoot)?;
        create_real_directory(&records).map_err(|_| LibraryError::InvalidRoot)?;
        create_real_directory(&imports).map_err(|_| LibraryError::InvalidRoot)?;
        create_real_directory(&sources).map_err(|_| LibraryError::InvalidRoot)?;
        let library = Self {
            records,
            imports,
            sources,
        };
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
                for source_path in source_paths {
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
        Ok(OpenedBook {
            book: self.public_book(&record),
            root,
        })
    }

    pub fn cover(&self, id: &str) -> Result<Resource, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
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

    pub fn remove(&self, id: &str) -> Result<(), LibraryError> {
        let path = self.record_path(id)?;
        if !path.is_file() {
            return Err(LibraryError::UnknownBook);
        }
        fs::remove_file(path).map_err(|_| LibraryError::WriteFailed)
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
        fs::create_dir(&target_records).map_err(|_| LibraryError::WriteFailed)?;
        fs::create_dir(&target_sources).map_err(|_| LibraryError::WriteFailed)?;
        let (mut records, sources) = self.durable_state()?;
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
            if let Some(imported) =
                read_imported_metadata(&self.imports.join(&record.id), &record.id)
            {
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
            }
            write_record(&target_records.join(format!("{}.json", record.id)), record)?;
        }
        for (source, name) in sources {
            fs::copy(source, target_sources.join(name)).map_err(|_| LibraryError::WriteFailed)?;
        }
        Ok(())
    }

    pub(crate) fn validate_durable_state(&self) -> Result<(), LibraryError> {
        let (records, sources) = self.durable_state()?;
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

    fn public_book(&self, record: &StoredBook) -> LibraryBook {
        let root = self.imports.join(&record.id);
        let imported = read_imported_metadata(&root, &record.id);
        let prepared = imported.is_some()
            && root.join(READER_MANIFEST).is_file()
            && has_import_marker(&root, &record.id);
        record.public(imported.as_ref(), prepared)
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

    fn remove_book_files(&self, id: &str) -> Result<(), LibraryError> {
        self.ensure_roots()?;
        remove_path_if_exists(&self.imports.join(id))?;
        for path in self.source_paths_for_id(id) {
            remove_path_if_exists(&path)?;
        }
        remove_path_if_exists(&self.record_path(id)?)?;
        sync_directory(&self.imports)?;
        sync_directory(&self.sources)?;
        sync_directory(&self.records)
    }

    fn durable_state(&self) -> Result<DurableLibraryState, LibraryError> {
        self.ensure_roots()?;
        let mut records = Vec::new();
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
        Ok((records, sources))
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

impl StoredBook {
    fn public(&self, imported: Option<&ImportedMetadata>, prepared: bool) -> LibraryBook {
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
        LibraryBook {
            id: self.id.clone(),
            title: imported_title.unwrap_or_else(|| self.title.clone()),
            authors: imported_authors
                .filter(|authors| !authors.is_empty())
                .unwrap_or_else(|| self.authors.clone()),
            has_cover: prepared
                && imported
                    .and_then(|metadata| metadata.cover_path.as_ref())
                    .or(self.cover_path.as_ref())
                    .is_some(),
            imported_at: self.imported_at,
            prepared,
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

fn has_import_marker(path: &Path, id: &str) -> bool {
    epub::has_cache_marker(path, id)
        || cbz::has_cache_marker(path, id)
        || fb2::has_cache_marker(path, id)
        || kindle::has_cache_marker(path, id)
        || text::has_cache_marker(path, id)
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
