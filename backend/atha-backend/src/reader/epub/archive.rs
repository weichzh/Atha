use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
};

use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive};

use super::{ImportError, MAX_ENTRIES, MAX_MEMBER_BYTES, MAX_SOURCE_BYTES, MAX_TOTAL_BYTES};

pub(super) type EpubArchive = ZipArchive<File>;

#[derive(Debug)]
pub(super) struct ArchiveIndex {
    members: HashMap<String, usize>,
}

pub(super) fn open(source: &Path) -> Result<EpubArchive, ImportError> {
    let file = File::open(source).map_err(|_| ImportError::InvalidSource)?;
    ZipArchive::new(file).map_err(|_| ImportError::InvalidArchive)
}

pub(super) fn inspect(archive: &mut EpubArchive) -> Result<ArchiveIndex, ImportError> {
    if archive.is_empty()
        || archive.len() > MAX_ENTRIES
        || archive
            .has_overlapping_files()
            .map_err(|_| ImportError::InvalidArchive)?
    {
        return Err(ImportError::InvalidArchive);
    }
    let mut members = HashMap::with_capacity(archive.len());
    let mut folded = HashSet::with_capacity(archive.len());
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|_| ImportError::InvalidArchive)?;
        let name = safe_path(entry.name().trim_end_matches('/'))?;
        if entry.encrypted() {
            return Err(ImportError::Encrypted);
        }
        if entry.is_symlink() {
            return Err(ImportError::UnsafePath);
        }
        total = total
            .checked_add(entry.size())
            .ok_or(ImportError::ArchiveTooLarge)?;
        if entry.size() > MAX_MEMBER_BYTES || total > MAX_TOTAL_BYTES {
            return Err(ImportError::ArchiveTooLarge);
        }
        if !folded.insert(name.to_lowercase()) {
            return Err(ImportError::UnsafePath);
        }
        if entry.is_file() {
            members.insert(name, index);
        }
    }
    Ok(ArchiveIndex { members })
}

pub(super) fn verify_mimetype(
    archive: &mut EpubArchive,
    index: &ArchiveIndex,
) -> Result<(), ImportError> {
    let first = archive
        .by_index_raw(0)
        .map_err(|_| ImportError::InvalidArchive)?;
    if first.name() != "mimetype" || first.compression() != CompressionMethod::Stored {
        return Err(ImportError::UnsupportedEpub);
    }
    drop(first);
    if read(archive, index, "mimetype")? != b"application/epub+zip" {
        return Err(ImportError::UnsupportedEpub);
    }
    Ok(())
}

impl ArchiveIndex {
    pub(super) fn contains(&self, path: &str) -> bool {
        self.members.contains_key(path)
    }

    fn require(&self, path: &str) -> Result<usize, ImportError> {
        self.members
            .get(path)
            .copied()
            .ok_or(ImportError::InvalidArchive)
    }
}

pub(super) fn require(index: &ArchiveIndex, path: &str) -> Result<(), ImportError> {
    index.require(path).map(|_| ())
}

pub(super) fn read(
    archive: &mut EpubArchive,
    index: &ArchiveIndex,
    path: &str,
) -> Result<Vec<u8>, ImportError> {
    let entry = archive
        .by_index(index.require(path)?)
        .map_err(|_| ImportError::InvalidArchive)?;
    let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
    entry
        .take(MAX_MEMBER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ImportError::InvalidArchive)?;
    if bytes.len() as u64 > MAX_MEMBER_BYTES {
        return Err(ImportError::ArchiveTooLarge);
    }
    Ok(bytes)
}

pub(super) fn copy(
    archive: &mut EpubArchive,
    index: &ArchiveIndex,
    path: &str,
    staging: &Path,
    extracted: &mut u64,
) -> Result<(), ImportError> {
    let entry = archive
        .by_index(index.require(path)?)
        .map_err(|_| ImportError::InvalidArchive)?;
    let destination = staging.join(path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|_| ImportError::WriteFailed)?;
    }
    let mut output = File::create(destination).map_err(|_| ImportError::WriteFailed)?;
    let copied = std::io::copy(&mut entry.take(MAX_MEMBER_BYTES + 1), &mut output)
        .map_err(|_| ImportError::InvalidArchive)?;
    *extracted = extracted
        .checked_add(copied)
        .ok_or(ImportError::ArchiveTooLarge)?;
    if copied > MAX_MEMBER_BYTES || *extracted > MAX_TOTAL_BYTES {
        return Err(ImportError::ArchiveTooLarge);
    }
    Ok(())
}

pub(super) fn resolve_reference(
    base_file: &str,
    value: &str,
) -> Result<(String, Option<String>), ImportError> {
    if value.is_empty()
        || value.contains(['\0', '\\', ':', '?', '%'])
        || value.starts_with('/')
        || value.matches('#').count() > 1
    {
        return Err(ImportError::UnsafePath);
    }
    let (path, fragment) = value
        .split_once('#')
        .map_or((value, None), |(path, fragment)| (path, Some(fragment)));
    if path.is_empty() {
        return Err(ImportError::UnsafePath);
    }
    let mut parts = base_file.split('/').collect::<Vec<_>>();
    parts.pop();
    for part in path.split('/') {
        match part {
            "" => return Err(ImportError::UnsafePath),
            "." => {}
            ".." => {
                parts.pop().ok_or(ImportError::UnsafePath)?;
            }
            value => parts.push(value),
        }
    }
    let path = safe_path(&parts.join("/"))?;
    let fragment = match fragment {
        Some(value)
            if value.is_empty()
                || value.encode_utf16().count() > 256
                || value.contains(['\\', '%', '?', '#'])
                || value.chars().any(char::is_control) =>
        {
            return Err(ImportError::UnsafePath);
        }
        Some(value) => Some(value.to_owned()),
        None => None,
    };
    Ok((path, fragment))
}

pub(super) fn safe_path(value: &str) -> Result<String, ImportError> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.contains(['\0', '\\', ':', '%', '?', '#'])
        || value.chars().any(char::is_control)
    {
        return Err(ImportError::UnsafePath);
    }
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or_default();
        let upper_stem = stem.to_ascii_uppercase();
        let reserved = matches!(upper_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || upper_stem.len() == 4
                && (upper_stem.starts_with("COM") || upper_stem.starts_with("LPT"))
                && upper_stem.as_bytes()[3].is_ascii_digit()
                && upper_stem.as_bytes()[3] != b'0';
        if part.is_empty() || matches!(part, "." | "..") || part.ends_with(['.', ' ']) || reserved {
            return Err(ImportError::UnsafePath);
        }
    }
    Ok(value.to_owned())
}

pub(super) fn hash_file(path: &Path) -> Result<String, ImportError> {
    let file = File::open(path).map_err(|_| ImportError::InvalidSource)?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| ImportError::InvalidSource)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ImportError::SourceTooLarge)?;
        if total > MAX_SOURCE_BYTES {
            return Err(ImportError::SourceTooLarge);
        }
        digest.update(&buffer[..read]);
    }
    let mut value = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.finalize() {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(value)
}
