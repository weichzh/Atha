use zip::CompressionMethod;

use super::ImportError;

pub(super) use crate::reader::archive::{
    Archive as EpubArchive, ArchiveIndex, copy, fingerprint, hash_file, inspect,
    open_fingerprinted as open, read, require, safe_path,
};

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
