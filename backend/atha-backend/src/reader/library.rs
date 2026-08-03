//! Content-addressed local library over imported EPUB reader roots.

use std::{
    error::Error,
    fmt, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use super::{
    epub::{ImportError, READER_MANIFEST, import_epub},
    resources::{BookRoot, Resource, ResourceError},
};

const LIBRARY_SCHEMA: u8 = 1;
const MAX_TITLE_CHARS: usize = 512;
const MAX_AUTHOR_CHARS: usize = 512;
const MAX_AUTHORS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBook {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub has_cover: bool,
    pub imported_at: u64,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryError {
    InvalidRoot,
    InvalidBookId,
    UnknownBook,
    CorruptRecord,
    MissingCover,
    ReadFailed,
    WriteFailed,
    Import(ImportError),
    Resource(ResourceError),
}

impl LocalLibrary {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let root = data_root.as_ref();
        let records = root.join("Library");
        let imports = root.join("ImportedBooks");
        fs::create_dir_all(&records).map_err(|_| LibraryError::InvalidRoot)?;
        fs::create_dir_all(&imports).map_err(|_| LibraryError::InvalidRoot)?;
        Ok(Self { records, imports })
    }

    pub fn list(&self) -> Result<Vec<LibraryBook>, LibraryError> {
        let mut books = Vec::new();
        for entry in fs::read_dir(&self.records).map_err(|_| LibraryError::ReadFailed)? {
            let entry = entry.map_err(|_| LibraryError::ReadFailed)?;
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            books.push(read_record(&path)?.public());
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
        let source = source.as_ref();
        if source
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("epub"))
        {
            return Err(LibraryError::Import(ImportError::InvalidSource));
        }
        let imported = import_epub(source, &self.imports).map_err(LibraryError::Import)?;
        let path = self.record_path(&imported.content_version)?;
        if path.exists() {
            return Ok(read_record(&path)?.public());
        }
        let title = imported
            .title
            .as_deref()
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
        };
        validate_record(&record)?;
        write_record(&path, &record)?;
        Ok(record.public())
    }

    pub fn open_book(&self, id: &str) -> Result<OpenedBook, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
        let root = BookRoot::new(self.imports.join(id)).map_err(LibraryError::Resource)?;
        root.read(&format!("/{READER_MANIFEST}"))
            .map_err(LibraryError::Resource)?;
        Ok(OpenedBook {
            book: record.public(),
            root,
        })
    }

    pub fn cover(&self, id: &str) -> Result<Resource, LibraryError> {
        let record = read_record(&self.record_path(id)?)?;
        let path = record.cover_path.ok_or(LibraryError::MissingCover)?;
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

    fn record_path(&self, id: &str) -> Result<PathBuf, LibraryError> {
        if !valid_id(id) {
            return Err(LibraryError::InvalidBookId);
        }
        Ok(self.records.join(format!("{id}.json")))
    }
}

impl StoredBook {
    fn public(&self) -> LibraryBook {
        LibraryBook {
            id: self.id.clone(),
            title: self.title.clone(),
            authors: self.authors.clone(),
            has_cover: self.cover_path.is_some(),
            imported_at: self.imported_at,
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
            Self::MissingCover => "missing-library-cover",
            Self::ReadFailed => "library-read-failed",
            Self::WriteFailed => "library-write-failed",
            Self::Import(error) => error.code(),
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

fn read_record(path: &Path) -> Result<StoredBook, LibraryError> {
    if !path.is_file() {
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
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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
