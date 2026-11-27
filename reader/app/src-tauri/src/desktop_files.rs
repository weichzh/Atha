use std::{ffi::OsString, path::PathBuf};

#[cfg(any(windows, target_os = "linux"))]
use std::path::Path;

use tauri_plugin_dialog::FilePath;

use crate::library_commands::{BOOK_EXTENSIONS, MAX_IMPORT_FILES};

const MAX_IMPORT_PATH_CHARS: usize = 32_768;

pub(crate) fn dropped_library_paths(paths: Vec<String>) -> Result<Vec<FilePath>, String> {
    if paths.is_empty()
        || paths.len() > MAX_IMPORT_FILES
        || paths.iter().any(|path| {
            path.is_empty() || path.chars().count() > MAX_IMPORT_PATH_CHARS || path.contains('\0')
        })
    {
        return Err("invalid-library-import".into());
    }
    Ok(paths
        .into_iter()
        .map(|path| FilePath::Path(PathBuf::from(path)))
        .collect())
}

// Cold startup retains the native process's full path semantics (including
// Windows per-drive working directories). Only forwarded arguments are bounded
// by what can be reconstructed from the other process's single cwd.
pub(crate) fn associated_book_paths(
    values: impl IntoIterator<Item = OsString>,
) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        if index == MAX_IMPORT_FILES {
            return None;
        }
        paths.push(book_path(value)?);
    }
    (!paths.is_empty()).then_some(paths)
}

fn book_path(value: OsString) -> Option<PathBuf> {
    let path = match value.to_str() {
        Some(value) if value.starts_with("file:") => {
            tauri::Url::parse(value).ok()?.to_file_path().ok()?
        }
        _ => PathBuf::from(value),
    };
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    BOOK_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(path)
}

#[cfg(any(windows, target_os = "linux"))]
pub(crate) fn forwarded_book_paths(
    values: impl IntoIterator<Item = OsString>,
    cwd: Option<&Path>,
) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        if index == MAX_IMPORT_FILES
            || value.is_empty()
            || value.len() > MAX_IMPORT_PATH_CHARS
            || value.as_encoded_bytes().contains(&0)
        {
            return None;
        }
        let path = book_path(value)?;
        let path = if path.is_absolute() {
            path
        } else {
            resolve_relative_path(cwd?, &path)?
        };
        if path.as_os_str().len() > MAX_IMPORT_PATH_CHARS {
            return None;
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    (!paths.is_empty()).then_some(paths)
}

#[cfg(any(windows, target_os = "linux"))]
fn resolve_relative_path(cwd: &Path, path: &Path) -> Option<PathBuf> {
    if !cwd.is_absolute() {
        return None;
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            // The plugin supplies one cwd, not Windows' per-drive working
            // directories. A drive-relative argument is unambiguous only
            // when it uses that same drive.
            let Component::Prefix(base) = cwd.components().next()? else {
                return None;
            };
            let drive = |prefix| match prefix {
                Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => Some(drive),
                _ => None,
            };
            if drive(prefix.kind()).is_some() && drive(prefix.kind()) == drive(base.kind()) {
                return Some(cwd.join(components.as_path()));
            }
            return None;
        }
    }
    let absolute = cwd.join(path);
    absolute.is_absolute().then_some(absolute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn file_association_arguments_are_only_supported_book_paths() {
        let cwd = std::env::temp_dir();
        let first = cwd.join("one.EPUB");
        let second = cwd.join("two words.fb2");
        let second_url = tauri::Url::from_file_path(&second).expect("native file URL");
        let paths = forwarded_book_paths(
            [
                first.clone().into_os_string(),
                OsString::from(second_url.as_str()),
                OsString::from("two words.fb2"),
            ],
            Some(&cwd),
        )
        .expect("book arguments");
        assert_eq!(paths, [first, second]);
        assert!(
            forwarded_book_paths(
                std::iter::repeat_n(OsString::from("book.epub"), MAX_IMPORT_FILES + 1),
                Some(&cwd)
            )
            .is_none()
        );
        assert!(forwarded_book_paths([OsString::from("relative.epub")], None).is_none());
        assert!(forwarded_book_paths([OsString::from("--epub")], None).is_none());
        assert!(forwarded_book_paths([OsString::from("/tmp/book.pdf")], None).is_none());
        assert!(forwarded_book_paths(std::iter::empty(), None).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn resolves_only_windows_paths_with_an_unambiguous_drive() {
        assert_eq!(
            resolve_relative_path(Path::new(r"C:\Books"), Path::new(r"C:Title.epub")),
            Some(PathBuf::from(r"C:\Books\Title.epub")),
        );
        assert_eq!(
            resolve_relative_path(Path::new(r"C:\Books"), Path::new(r"\Title.epub")),
            Some(PathBuf::from(r"C:\Title.epub")),
        );
        assert!(
            resolve_relative_path(Path::new(r"C:\Books"), Path::new(r"D:Title.epub")).is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    fn cold_start_preserves_drive_relative_and_legacy_file_urls() {
        let relative = OsString::from(r"D:Title.epub");
        let legacy = OsString::from("file:/C|/Book.epub");
        assert_eq!(
            associated_book_paths([relative.clone(), legacy]),
            Some(vec![
                PathBuf::from(relative),
                PathBuf::from(r"C:\Book.epub")
            ])
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_cold_arguments_do_not_require_unicode() {
        use std::os::unix::ffi::OsStringExt;
        let input = OsString::from_vec(b"\xff title.epub".to_vec());
        assert_eq!(
            associated_book_paths([input.clone()]),
            Some(vec![input.into()])
        );
    }

    #[test]
    fn dropped_paths_have_a_small_explicit_boundary() {
        assert!(dropped_library_paths(vec!["/tmp/book.epub".into()]).is_ok());
        assert!(dropped_library_paths(Vec::new()).is_err());
        assert!(dropped_library_paths(vec!["bad\0path.epub".into()]).is_err());
        assert!(dropped_library_paths(vec!["x".repeat(MAX_IMPORT_PATH_CHARS + 1)]).is_err());
        assert!(dropped_library_paths(vec!["book.epub".into(); MAX_IMPORT_FILES + 1]).is_err());
    }
}
