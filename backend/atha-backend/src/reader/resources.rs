//! Constrained access to one canonical book root.

use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    path::{Component, Path, PathBuf},
};

pub(super) const MAX_RESOURCE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_READER_SECTIONS: usize = 2_000;

#[derive(Clone, Debug)]
pub struct BookRoot {
    root: PathBuf,
    xhtml_paths: HashSet<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Resource {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceError {
    InvalidRoot,
    InvalidEncoding,
    InvalidPath,
    OutsideRoot,
    NotFound,
    NotAFile,
    UnsupportedMediaType,
    TooLarge,
    ReadFailed,
}

impl BookRoot {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ResourceError> {
        let root = fs::canonicalize(root).map_err(|_| ResourceError::InvalidRoot)?;
        if !root.is_dir() {
            return Err(ResourceError::InvalidRoot);
        }
        let xhtml_paths = manifest_xhtml_paths(&root);
        Ok(Self { root, xhtml_paths })
    }

    pub fn read(&self, request_path: &str) -> Result<Resource, ResourceError> {
        let relative = decode_request_path(request_path)?;
        let candidate = fs::canonicalize(self.root.join(&relative)).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ResourceError::NotFound
            } else {
                ResourceError::ReadFailed
            }
        })?;
        if !candidate.starts_with(&self.root) {
            return Err(ResourceError::OutsideRoot);
        }
        let metadata = candidate
            .metadata()
            .map_err(|_| ResourceError::ReadFailed)?;
        if !metadata.is_file() {
            return Err(ResourceError::NotAFile);
        }
        if metadata.len() > MAX_RESOURCE_BYTES {
            return Err(ResourceError::TooLarge);
        }
        let content_type = if self.xhtml_paths.contains(&relative) {
            "application/xhtml+xml; charset=utf-8"
        } else {
            content_type(&candidate).ok_or(ResourceError::UnsupportedMediaType)?
        };
        let bytes = fs::read(candidate).map_err(|_| ResourceError::ReadFailed)?;
        Ok(Resource {
            bytes,
            content_type,
        })
    }
}

fn manifest_xhtml_paths(root: &Path) -> HashSet<PathBuf> {
    let Ok(manifest_path) = fs::canonicalize(root.join(".atha-reader.json")) else {
        return HashSet::new();
    };
    if !manifest_path.starts_with(root) {
        return HashSet::new();
    }
    let Ok(metadata) = manifest_path.metadata() else {
        return HashSet::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_RESOURCE_BYTES {
        return HashSet::new();
    }
    let Ok(bytes) = fs::read(manifest_path) else {
        return HashSet::new();
    };
    let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return HashSet::new();
    };
    if manifest.get("schema").and_then(serde_json::Value::as_u64) != Some(1) {
        return HashSet::new();
    }
    let Some(sections) = manifest
        .get("sections")
        .and_then(serde_json::Value::as_array)
    else {
        return HashSet::new();
    };
    if sections.len() > MAX_READER_SECTIONS {
        return HashSet::new();
    }
    sections
        .iter()
        .filter_map(|section| section.get("href").and_then(serde_json::Value::as_str))
        .filter_map(|href| decode_request_path(&format!("/{href}")).ok())
        .collect()
}

impl ResourceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid-root",
            Self::InvalidEncoding => "invalid-encoding",
            Self::InvalidPath => "invalid-path",
            Self::OutsideRoot => "outside-root",
            Self::NotFound => "not-found",
            Self::NotAFile => "not-a-file",
            Self::UnsupportedMediaType => "unsupported-media-type",
            Self::TooLarge => "resource-too-large",
            Self::ReadFailed => "read-failed",
        }
    }
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for ResourceError {}

fn decode_request_path(request_path: &str) -> Result<PathBuf, ResourceError> {
    let encoded = request_path
        .strip_prefix('/')
        .ok_or(ResourceError::InvalidPath)?;
    let decoded = percent_decode(encoded)?;
    if decoded.is_empty()
        || decoded.contains(['\0', '\\', ':'])
        || decoded.starts_with('/')
        || decoded.ends_with('/')
        || decoded.split('/').any(|part| part.is_empty())
    {
        return Err(ResourceError::InvalidPath);
    }
    let relative = Path::new(&decoded);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(ResourceError::InvalidPath);
    }
    Ok(relative.to_owned())
}

pub(super) fn percent_decode(value: &str) -> Result<String, ResourceError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err(ResourceError::InvalidEncoding);
        }
        let high = hex(bytes[index + 1]).ok_or(ResourceError::InvalidEncoding)?;
        let low = hex(bytes[index + 2]).ok_or(ResourceError::InvalidEncoding)?;
        decoded.push(high << 4 | low);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| ResourceError::InvalidEncoding)
}

const fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn content_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "xhtml" => Some("application/xhtml+xml; charset=utf-8"),
        "json" => Some("application/json; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}
