use std::{
    fs::{self, File, OpenOptions as StdOpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::FilePath;
use tauri_plugin_fs::{FsExt, OpenOptions};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

pub(crate) fn cleanup(app: &AppHandle) -> io::Result<()> {
    let root = picker_root(app)?;
    match fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) struct PickerInput {
    path: PickerPath,
    suffix: &'static str,
    title_hint: Option<String>,
}

impl PickerInput {
    pub(crate) fn open(
        app: &AppHandle,
        selected: FilePath,
        suffix: &'static str,
        max_bytes: u64,
    ) -> io::Result<Self> {
        if let Ok(path) = selected.clone().into_path() {
            return Ok(Self {
                path: PickerPath::Direct(path),
                suffix,
                title_hint: None,
            });
        }

        let temporary = TemporaryPath::reserve(app, suffix)?;
        let mut options = OpenOptions::new();
        options.read(true);
        let mut source = app.fs().open(selected, options)?;
        let mut target = StdOpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary.path())?;
        copy_limited(&mut source, &mut target, max_bytes)?;
        target.sync_all()?;
        Ok(Self {
            path: PickerPath::Temporary(temporary),
            suffix,
            title_hint: None,
        })
    }

    pub(crate) fn open_book(
        app: &AppHandle,
        selected: FilePath,
        max_bytes: u64,
    ) -> io::Result<Self> {
        if selected.clone().into_path().is_ok() {
            return Self::open(app, selected, "book", max_bytes);
        }
        let file_name = app
            .path()
            .file_name(&selected.to_string())
            .ok_or_else(invalid_book_name)?;
        let (suffix, title_hint) =
            book_source_metadata(&file_name).ok_or_else(invalid_book_name)?;
        let mut input = Self::open(app, selected, suffix, max_bytes)?;
        input.title_hint = Some(title_hint.to_owned());
        Ok(input)
    }

    pub(crate) fn open_dictionary(
        app: &AppHandle,
        selected: FilePath,
        max_bytes: u64,
    ) -> io::Result<Self> {
        let file_name = selected
            .clone()
            .into_path()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .or_else(|| app.path().file_name(&selected.to_string()))
            .ok_or_else(invalid_dictionary_name)?;
        let suffix = dictionary_source_suffix(&file_name).ok_or_else(invalid_dictionary_name)?;
        Self::open(app, selected, suffix, max_bytes)
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub(crate) fn title_hint(&self) -> Option<&str> {
        self.title_hint.as_deref()
    }

    pub(crate) const fn suffix(&self) -> &'static str {
        self.suffix
    }
}

pub(crate) struct PickerOutput {
    app: AppHandle,
    destination: Option<FilePath>,
    path: PickerPath,
}

impl PickerOutput {
    pub(crate) fn new(
        app: &AppHandle,
        selected: FilePath,
        suffix: &'static str,
    ) -> io::Result<Self> {
        let (destination, path) = match selected.clone().into_path() {
            Ok(path) => (None, PickerPath::Direct(path)),
            Err(_) => (
                Some(selected),
                PickerPath::Temporary(TemporaryPath::reserve(app, suffix)?),
            ),
        };
        Ok(Self {
            app: app.clone(),
            destination,
            path,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub(crate) fn commit(mut self) -> io::Result<()> {
        let Some(destination) = self.destination.take() else {
            return Ok(());
        };
        let mut source = File::open(self.path())?;
        let mut options = OpenOptions::new();
        options.write(true).truncate(true);
        let mut target = self.app.fs().open(destination, options)?;
        io::copy(&mut source, &mut target)?;
        target.flush()
    }
}

enum PickerPath {
    Direct(PathBuf),
    Temporary(TemporaryPath),
}

impl AsRef<Path> for PickerPath {
    fn as_ref(&self) -> &Path {
        match self {
            Self::Direct(path) => path,
            Self::Temporary(path) => path.path(),
        }
    }
}

struct TemporaryPath {
    directory: PathBuf,
    path: PathBuf,
}

impl TemporaryPath {
    fn reserve(app: &AppHandle, suffix: &'static str) -> io::Result<Self> {
        let parent = picker_root(app)?;
        reserve_temporary(&parent, suffix)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

fn picker_root(app: &AppHandle) -> io::Result<PathBuf> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(io::Error::other)?
        .join("Picker"))
}

impl Drop for TemporaryPath {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn reserve_temporary(parent: &Path, suffix: &'static str) -> io::Result<TemporaryPath> {
    fs::create_dir_all(parent)?;
    for _ in 0..64 {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let directory = parent.join(format!("{}-{sequence}", std::process::id()));
        match fs::create_dir(&directory) {
            Ok(()) => {
                return Ok(TemporaryPath {
                    path: directory.join(format!("content.{suffix}")),
                    directory,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not reserve picker cache",
    ))
}

fn book_source_metadata(file_name: &str) -> Option<(&'static str, &str)> {
    if file_name.is_empty() || file_name.contains(['/', '\\', '\0']) {
        return None;
    }
    let path = Path::new(file_name);
    let extension = path.extension()?.to_str()?;
    let suffix = [
        "epub", "cbz", "fb2", "fbz", "mobi", "azw", "azw3", "md", "markdown", "txt",
    ]
    .into_iter()
    .find(|candidate| extension.eq_ignore_ascii_case(candidate))?;
    let title_hint = path.file_stem()?.to_str()?;
    (!title_hint.is_empty()).then_some((suffix, title_hint))
}

fn invalid_book_name() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "unsupported book file name")
}

fn dictionary_source_suffix(file_name: &str) -> Option<&'static str> {
    if file_name.is_empty() || file_name.contains(['/', '\\', '\0']) {
        return None;
    }
    let extension = Path::new(file_name).extension()?.to_str()?;
    ["mdx", "mdd", "mobi"]
        .into_iter()
        .find(|candidate| extension.eq_ignore_ascii_case(candidate))
}

fn invalid_dictionary_name() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "unsupported dictionary file name",
    )
}

fn copy_limited(
    source: &mut impl Read,
    target: &mut impl Write,
    max_bytes: u64,
) -> io::Result<u64> {
    let copied = io::copy(&mut source.take(max_bytes.saturating_add(1)), target)?;
    if copied > max_bytes {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "picker input exceeds limit",
        ))
    } else {
        Ok(copied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_source_metadata_accepts_only_supported_file_names() {
        for (name, suffix, title_hint) in [
            ("book.epub", "epub", "book"),
            ("book.CBZ", "cbz", "book"),
            ("book.fb2", "fb2", "book"),
            ("book.FBZ", "fbz", "book"),
            ("book.mobi", "mobi", "book"),
            ("book.AZW", "azw", "book"),
            ("book.azw3", "azw3", "book"),
            ("notes.md", "md", "notes"),
            ("notes.MARKDOWN", "markdown", "notes"),
            ("novel.txt", "txt", "novel"),
        ] {
            assert_eq!(book_source_metadata(name), Some((suffix, title_hint)));
        }
        for name in ["book", "book.pdf", "book.txt.exe", ".txt"] {
            assert_eq!(book_source_metadata(name), None);
        }
    }

    #[test]
    fn temporary_picker_path_is_removed_on_drop() {
        let parent = std::env::temp_dir().join(format!(
            "atha-picker-test-{}-{}",
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = reserve_temporary(&parent, "epub").expect("reserve picker cache");
        let directory = temporary.directory.clone();
        assert_eq!(
            temporary
                .path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("content.epub")
        );
        fs::write(temporary.path(), b"epub").expect("write picker cache");

        drop(temporary);

        assert!(!directory.exists());
        fs::remove_dir(parent).expect("remove picker test root");
    }

    #[test]
    fn picker_copy_enforces_the_existing_domain_limit() {
        let mut exact = io::Cursor::new(b"1234");
        let mut exact_target = Vec::new();
        assert_eq!(
            copy_limited(&mut exact, &mut exact_target, 4).expect("copy at limit"),
            4
        );

        let mut oversized = io::Cursor::new(b"12345");
        let error =
            copy_limited(&mut oversized, &mut Vec::new(), 4).expect_err("reject input over limit");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn dictionary_source_names_accept_only_mdict_files() {
        assert_eq!(dictionary_source_suffix("dictionary.MDX"), Some("mdx"));
        assert_eq!(dictionary_source_suffix("resources.mdd"), Some("mdd"));
        assert_eq!(dictionary_source_suffix("classic.mobi"), Some("mobi"));
        for name in [
            "dictionary",
            "dictionary.azw3",
            "dictionary.mdx.exe",
            ".mdx",
        ] {
            assert_eq!(dictionary_source_suffix(name), None);
        }
    }
}
