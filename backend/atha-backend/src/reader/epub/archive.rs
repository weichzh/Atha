use super::ImportError;

pub(super) use crate::reader::archive::{
    Archive as EpubArchive, ArchiveIndex, copy, fingerprint, hash_file, inspect,
    open_fingerprinted as open, read, safe_path,
};

pub(super) fn resolve_reference(
    base_file: &str,
    value: &str,
) -> Result<(String, Option<String>), ImportError> {
    if value.is_empty()
        || value.contains(['\0', '\\', ':', '?'])
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
    let mut parts = base_file.split('/').map(str::to_owned).collect::<Vec<_>>();
    parts.pop();
    for encoded in path.split('/') {
        let part = crate::reader::resources::percent_decode(encoded)
            .map_err(|_| ImportError::UnsafePath)?;
        if part.contains(['\0', '/', '\\', ':', '?', '#', '%'])
            || part.chars().any(char::is_control)
            || encoded.contains('%') && matches!(part.as_str(), "." | "..")
        {
            return Err(ImportError::UnsafePath);
        }
        match part.as_str() {
            "" => return Err(ImportError::UnsafePath),
            "." => {}
            ".." => {
                parts.pop().ok_or(ImportError::UnsafePath)?;
            }
            value => parts.push(value.to_owned()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decodes_safe_path_segments_once() {
        assert_eq!(
            resolve_reference("OPS/book.opf", "images/cover%20one.png"),
            Ok(("OPS/images/cover one.png".into(), None))
        );
        assert_eq!(
            resolve_reference("OPS/book.opf", "fonts/%E4%B8%AD.ttf"),
            Ok(("OPS/fonts/中.ttf".into(), None))
        );
        for value in [
            "%2e%2e/escape.xhtml",
            "images%2fescape.png",
            "images/%00.png",
            "images/%25.png",
            "images/%zz.png",
        ] {
            assert_eq!(
                resolve_reference("OPS/book.opf", value),
                Err(ImportError::UnsafePath),
                "{value}"
            );
        }
    }
}
