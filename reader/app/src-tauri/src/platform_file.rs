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
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
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
    fn temporary_picker_path_is_removed_on_drop() {
        let parent = std::env::temp_dir().join(format!(
            "atha-picker-test-{}-{}",
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = reserve_temporary(&parent, "epub").expect("reserve picker cache");
        let directory = temporary.directory.clone();
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
}
