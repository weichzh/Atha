//! Application-level backup, restore, storage accounting, and recovery.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    create_real_directory, is_reparse_point,
    messages::MessageStore,
    reader::{
        dictionary::LocalDictionaries,
        library::{BOOK_EXTENSIONS, LibraryError, LocalLibrary},
    },
};

const DATA_SCHEMA: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const BROWSER_STATE_ENTRY: &str = "BrowserState.json";
const MESSAGE_BACKUP_ENTRY: &str = "Messages.atha-backup";
const JOURNAL_FILE: &str = ".atha-data-restore.json";
const PUBLISH_FILE: &str = ".atha-data-restore.publishing";
const COMMIT_FILE: &str = ".atha-data-restore.committed";
const ROLLBACK_FILE: &str = ".atha-data-restore.rollback-pending";
const LOCK_FILE: &str = ".atha-data.lock";
const DELETE_PREFIX: &str = ".atha-delete-";
const PREVIOUS_BROWSER_STATE: &str = "PreviousBrowserState.json";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_BROWSER_STATE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_LOCAL_DATA_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_LOCAL_DATA_FILES: usize = 100_000;
const MAX_BROWSER_RECORDS: usize = 10_000;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct LocalData {
    root: PathBuf,
}

pub struct LocalDataOperationGuard {
    _file: File,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserState {
    pub schema: u32,
    pub records: Vec<BrowserStateRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserStateRecord {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingLocalDataRestore {
    pub token: String,
    pub browser_state: BrowserState,
    pub rollback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub books_bytes: u64,
    pub cache_bytes: u64,
    pub messages_bytes: u64,
    pub dictionaries_bytes: u64,
    pub preferences_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalDataError {
    InvalidRoot,
    Busy,
    InvalidBrowserState,
    MissingBookSource,
    InvalidBackup,
    BackupFailed,
    RestoreFailed,
    RecoveryFailed,
    UnsafeData,
    UnknownRestore,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DataManifest {
    schema: u32,
    files: Vec<DataFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DataFile {
    path: String,
    content_hash: String,
    byte_length: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RestoreState {
    Prepared,
    Committed,
    RollbackPending,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreJournal {
    schema: u32,
    token: String,
    state: RestoreState,
}

impl BrowserState {
    pub fn validate(&self) -> Result<(), LocalDataError> {
        if self.schema != DATA_SCHEMA || self.records.len() > MAX_BROWSER_RECORDS {
            return Err(LocalDataError::InvalidBrowserState);
        }
        let mut previous = None;
        for record in &self.records {
            if record.value.is_empty()
                || !valid_browser_key(&record.key)
                || previous.is_some_and(|key: &str| key >= record.key.as_str())
                || !valid_browser_value(&record.key, &record.value)
            {
                return Err(LocalDataError::InvalidBrowserState);
            }
            previous = Some(record.key.as_str());
        }
        let bytes = serde_json::to_vec(self).map_err(|_| LocalDataError::InvalidBrowserState)?;
        if bytes.len() as u64 > MAX_BROWSER_STATE_BYTES {
            Err(LocalDataError::InvalidBrowserState)
        } else {
            Ok(())
        }
    }
}

impl LocalData {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, LocalDataError> {
        let root = root.as_ref();
        create_real_directory(root).map_err(|_| LocalDataError::InvalidRoot)?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn create_backup(
        &self,
        target: impl AsRef<Path>,
        browser_state: &BrowserState,
        library: &LocalLibrary,
        dictionaries: &LocalDictionaries,
        messages: &MessageStore,
    ) -> Result<(), LocalDataError> {
        browser_state.validate()?;
        let _lock = self.lock()?;
        if self.recovery_files_exist() || self.deletion_intents_exist()? {
            return Err(LocalDataError::Busy);
        }
        let target = target.as_ref();
        if fs::symlink_metadata(target).is_ok() {
            return Err(LocalDataError::BackupFailed);
        }
        let token = temporary_token()?;
        let stage = self.stage_path(&token);
        fs::create_dir(&stage).map_err(|_| LocalDataError::BackupFailed)?;
        let archive_temp = adjacent_temporary(target, &token)?;
        let result = (|| {
            library
                .write_backup_state(&stage)
                .map_err(|error| match error {
                    LibraryError::MissingSource => LocalDataError::MissingBookSource,
                    _ => LocalDataError::UnsafeData,
                })?;
            dictionaries
                .write_backup_state(&stage)
                .map_err(|_| LocalDataError::UnsafeData)?;
            messages
                .create_backup(stage.join(MESSAGE_BACKUP_ENTRY))
                .map_err(|_| LocalDataError::BackupFailed)?;
            write_json_file(&stage.join(BROWSER_STATE_ENTRY), browser_state)
                .map_err(|_| LocalDataError::BackupFailed)?;
            if validate_stage(&stage, messages)? != *browser_state {
                return Err(LocalDataError::UnsafeData);
            }
            let manifest = build_manifest(&stage)?;
            write_archive(&stage, &archive_temp, &manifest)?;
            if inspect_archive(&archive_temp)? != manifest {
                return Err(LocalDataError::BackupFailed);
            }
            publish_archive(&archive_temp, target)
        })();
        let _ = fs::remove_dir_all(stage);
        let _ = fs::remove_file(archive_temp);
        result
    }

    pub fn prepare_restore(
        &self,
        source: impl AsRef<Path>,
        previous_browser_state: &BrowserState,
        messages: &MessageStore,
    ) -> Result<PendingLocalDataRestore, LocalDataError> {
        previous_browser_state.validate()?;
        let _lock = self.lock()?;
        if self.recovery_files_exist() || self.deletion_intents_exist()? {
            return Err(LocalDataError::Busy);
        }
        let token = temporary_token()?;
        let stage = self.stage_path(&token);
        let rollback = self.rollback_path(&token);
        fs::create_dir(&stage).map_err(|_| LocalDataError::RestoreFailed)?;
        fs::create_dir(&rollback).map_err(|_| LocalDataError::RestoreFailed)?;
        let result = (|| {
            let manifest = inspect_archive(source.as_ref())?;
            extract_archive(source.as_ref(), &stage, &manifest)?;
            let browser_state = validate_stage(&stage, messages)?;
            sync_directory_tree(&stage).map_err(|_| LocalDataError::RestoreFailed)?;
            write_json_file(
                &rollback.join(PREVIOUS_BROWSER_STATE),
                previous_browser_state,
            )
            .map_err(|_| LocalDataError::RestoreFailed)?;
            messages
                .create_backup(rollback.join(MESSAGE_BACKUP_ENTRY))
                .map_err(|_| LocalDataError::RestoreFailed)?;
            sync_directory(&rollback).map_err(|_| LocalDataError::RestoreFailed)?;
            write_journal(
                &self.journal_path(),
                &RestoreJournal {
                    schema: DATA_SCHEMA,
                    token: token.clone(),
                    state: RestoreState::Prepared,
                },
            )?;
            Ok(PendingLocalDataRestore {
                token: token.clone(),
                browser_state,
                rollback: false,
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(stage);
            let _ = fs::remove_dir_all(rollback);
        }
        result
    }

    pub fn commit_restore(
        &self,
        token: &str,
        messages: &MessageStore,
    ) -> Result<PendingLocalDataRestore, LocalDataError> {
        self.commit_restore_with(token, messages, || Ok(()))
    }

    fn commit_restore_with(
        &self,
        token: &str,
        messages: &MessageStore,
        before_marker: impl FnOnce() -> Result<(), LocalDataError>,
    ) -> Result<PendingLocalDataRestore, LocalDataError> {
        let _lock = self.lock()?;
        let journal = self.read_journal()?;
        validate_token(token)?;
        if journal.token != token {
            return Err(LocalDataError::UnknownRestore);
        }
        let stage = self.stage_path(token);
        if journal.state == RestoreState::Committed {
            return Ok(PendingLocalDataRestore {
                token: token.to_owned(),
                browser_state: read_browser_state(&stage.join(BROWSER_STATE_ENTRY))?,
                rollback: false,
            });
        }
        if journal.state != RestoreState::Prepared {
            return Err(LocalDataError::RecoveryFailed);
        }
        let browser_state = read_browser_state(&stage.join(BROWSER_STATE_ENTRY))?;
        validate_stage(&stage, messages)?;
        let result = (|| {
            write_recovery_marker(&self.root.join(PUBLISH_FILE), &journal)?;
            for name in ["Library", "SourceBooks", "Dictionaries", "ImportedBooks"] {
                swap_directory(
                    &self.root.join(name),
                    &stage.join(name),
                    &self.rollback_path(token).join(name),
                )?;
            }
            messages
                .restore_backup(stage.join(MESSAGE_BACKUP_ENTRY))
                .map_err(|_| LocalDataError::RestoreFailed)?;
            messages
                .recover_assets()
                .map_err(|_| LocalDataError::RestoreFailed)?;
            sync_directory(&self.rollback_path(token))
                .map_err(|_| LocalDataError::RestoreFailed)?;
            sync_directory(&stage).map_err(|_| LocalDataError::RestoreFailed)?;
            before_marker()?;
            write_recovery_marker(
                &self.root.join(COMMIT_FILE),
                &RestoreJournal {
                    state: RestoreState::Committed,
                    ..journal.clone()
                },
            )
        })();
        if result.is_err() {
            return match self.begin_rollback_locked(&journal, messages) {
                Ok(()) => Err(LocalDataError::RestoreFailed),
                Err(_) => Err(LocalDataError::RecoveryFailed),
            };
        }
        Ok(PendingLocalDataRestore {
            token: token.to_owned(),
            browser_state,
            rollback: false,
        })
    }

    #[cfg(test)]
    fn interrupt_restore_after_swap(
        &self,
        token: &str,
        names: &[&str],
    ) -> Result<(), LocalDataError> {
        let _lock = self.lock()?;
        let journal = self.read_journal()?;
        if journal.token != token || journal.state != RestoreState::Prepared {
            return Err(LocalDataError::UnknownRestore);
        }
        let stage = self.stage_path(token);
        let rollback = self.rollback_path(token);
        write_recovery_marker(&self.root.join(PUBLISH_FILE), &journal)?;
        for name in names {
            swap_directory(
                &self.root.join(name),
                &stage.join(name),
                &rollback.join(name),
            )?;
        }
        sync_directory(&rollback).map_err(|_| LocalDataError::RestoreFailed)?;
        sync_directory(&stage).map_err(|_| LocalDataError::RestoreFailed)?;
        sync_directory(&self.root).map_err(|_| LocalDataError::RestoreFailed)
    }

    pub fn pending_restore(&self) -> Result<Option<PendingLocalDataRestore>, LocalDataError> {
        let _lock = self.lock()?;
        let journal = match self.read_optional_journal()? {
            Some(journal) => journal,
            None => return Ok(None),
        };
        let rollback = journal.state == RestoreState::RollbackPending;
        let state_path = if rollback {
            self.rollback_path(&journal.token)
                .join(PREVIOUS_BROWSER_STATE)
        } else if journal.state == RestoreState::Committed {
            self.stage_path(&journal.token).join(BROWSER_STATE_ENTRY)
        } else {
            return Err(LocalDataError::RecoveryFailed);
        };
        Ok(Some(PendingLocalDataRestore {
            browser_state: read_browser_state(&state_path)?,
            token: journal.token,
            rollback,
        }))
    }

    pub fn finish_restore(&self, token: &str) -> Result<(), LocalDataError> {
        let _lock = self.lock()?;
        let journal = self.read_journal()?;
        if journal.token != token
            || !matches!(
                journal.state,
                RestoreState::Committed | RestoreState::RollbackPending
            )
        {
            return Err(LocalDataError::UnknownRestore);
        }
        fs::remove_file(self.journal_path()).map_err(|_| LocalDataError::RecoveryFailed)?;
        sync_directory(&self.root).map_err(|_| LocalDataError::RecoveryFailed)?;
        // Once journal removal is durable, the selected browser state is confirmed; cleanup is best effort.
        let _ = fs::remove_file(self.root.join(COMMIT_FILE));
        let _ = fs::remove_file(self.root.join(ROLLBACK_FILE));
        let _ = fs::remove_file(self.root.join(PUBLISH_FILE));
        let _ = remove_directory_if_exists(&self.rollback_path(token));
        let _ = remove_directory_if_exists(&self.stage_path(token));
        let _ = sync_directory(&self.root);
        Ok(())
    }

    pub fn rollback_restore(
        &self,
        token: &str,
        messages: &MessageStore,
    ) -> Result<BrowserState, LocalDataError> {
        let _lock = self.lock()?;
        let journal = self.read_journal()?;
        if journal.token != token {
            return Err(LocalDataError::UnknownRestore);
        }
        let previous = read_browser_state(&self.rollback_path(token).join(PREVIOUS_BROWSER_STATE))?;
        self.begin_rollback_locked(&journal, messages)?;
        Ok(previous)
    }

    pub fn abort_restore(&self, token: &str) -> Result<(), LocalDataError> {
        let _lock = self.lock()?;
        let journal = self.read_journal()?;
        validate_token(token)?;
        if journal.token != token || journal.state != RestoreState::Prepared {
            return Err(LocalDataError::UnknownRestore);
        }
        if self.root.join(PUBLISH_FILE).exists()
            || self.root.join(COMMIT_FILE).exists()
            || self.root.join(ROLLBACK_FILE).exists()
            || ["Library", "SourceBooks", "Dictionaries", "ImportedBooks"]
                .iter()
                .any(|name| self.rollback_path(token).join(name).exists())
        {
            return Err(LocalDataError::RecoveryFailed);
        }
        fs::remove_file(self.journal_path()).map_err(|_| LocalDataError::RecoveryFailed)?;
        sync_directory(&self.root).map_err(|_| LocalDataError::RecoveryFailed)?;
        let _ = fs::remove_file(self.root.join(PUBLISH_FILE));
        let _ = remove_directory_if_exists(&self.stage_path(token));
        let _ = remove_directory_if_exists(&self.rollback_path(token));
        let _ = sync_directory(&self.root);
        Ok(())
    }

    pub fn recover(&self, messages: &MessageStore) -> Result<(), LocalDataError> {
        let _lock = self.lock()?;
        let Some(journal) = self.read_optional_journal()? else {
            self.cleanup_orphans()?;
            return Ok(());
        };
        match journal.state {
            RestoreState::Prepared => self.rollback_and_cleanup_locked(&journal, messages)?,
            RestoreState::RollbackPending => self.rollback_backend_locked(&journal, messages)?,
            RestoreState::Committed => {}
        }
        Ok(())
    }

    pub fn require_ready(&self) -> Result<(), LocalDataError> {
        self.operation_guard().map(|_| ())
    }

    pub fn operation_guard(&self) -> Result<LocalDataOperationGuard, LocalDataError> {
        let operation = self.coordination_guard()?;
        if self.deletion_intents_exist()? {
            return Err(LocalDataError::Busy);
        }
        Ok(operation)
    }

    pub fn deletion_guard(&self) -> Result<LocalDataOperationGuard, LocalDataError> {
        let file = self.lock()?;
        if self.recovery_files_exist() || self.deletion_intents_exist()? {
            return Err(LocalDataError::Busy);
        }
        Ok(LocalDataOperationGuard { _file: file })
    }

    pub fn coordination_guard(&self) -> Result<LocalDataOperationGuard, LocalDataError> {
        let file = self.lock_file()?;
        fs2::FileExt::try_lock_shared(&file).map_err(|_| LocalDataError::Busy)?;
        if self.recovery_files_exist() {
            return Err(LocalDataError::Busy);
        }
        Ok(LocalDataOperationGuard { _file: file })
    }

    pub fn storage_usage(
        &self,
        browser_state: &BrowserState,
    ) -> Result<StorageUsage, LocalDataError> {
        browser_state.validate()?;
        let _lock = self.operation_guard()?;
        let books_bytes = directory_bytes(&self.root.join("Library"))?
            .checked_add(directory_bytes(&self.root.join("SourceBooks"))?)
            .ok_or(LocalDataError::UnsafeData)?;
        let cache_bytes = directory_bytes(&self.root.join("ImportedBooks"))?;
        let messages_bytes = directory_bytes(&self.root.join("Messages"))?;
        let dictionaries_bytes = directory_bytes(&self.root.join("Dictionaries"))?;
        let preferences_bytes = serde_json::to_vec(browser_state)
            .map_err(|_| LocalDataError::InvalidBrowserState)?
            .len() as u64;
        let total_bytes = [
            books_bytes,
            cache_bytes,
            messages_bytes,
            dictionaries_bytes,
            preferences_bytes,
        ]
        .into_iter()
        .try_fold(0_u64, |total, value| total.checked_add(value))
        .ok_or(LocalDataError::UnsafeData)?;
        Ok(StorageUsage {
            books_bytes,
            cache_bytes,
            messages_bytes,
            dictionaries_bytes,
            preferences_bytes,
            total_bytes,
        })
    }

    fn lock(&self) -> Result<File, LocalDataError> {
        let file = self.lock_file()?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|_| LocalDataError::Busy)?;
        Ok(file)
    }

    fn lock_file(&self) -> Result<File, LocalDataError> {
        if !real_directory(&self.root) {
            return Err(LocalDataError::InvalidRoot);
        }
        let path = self.root.join(LOCK_FILE);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(metadata) if metadata.file_type().is_file() && !is_reparse_point(&metadata) => {}
            _ => return Err(LocalDataError::InvalidRoot),
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|_| LocalDataError::InvalidRoot)?;
        let metadata = fs::symlink_metadata(path).map_err(|_| LocalDataError::InvalidRoot)?;
        if metadata.file_type().is_file() && !is_reparse_point(&metadata) {
            Ok(file)
        } else {
            Err(LocalDataError::InvalidRoot)
        }
    }

    fn begin_rollback_locked(
        &self,
        journal: &RestoreJournal,
        messages: &MessageStore,
    ) -> Result<(), LocalDataError> {
        let marker = RestoreJournal {
            state: RestoreState::RollbackPending,
            ..journal.clone()
        };
        match read_recovery_marker(&self.root.join(ROLLBACK_FILE))? {
            Some(existing) if existing == marker => {}
            Some(_) => return Err(LocalDataError::RecoveryFailed),
            None => write_recovery_marker(&self.root.join(ROLLBACK_FILE), &marker)?,
        }
        self.rollback_backend_locked(journal, messages)
    }

    fn rollback_backend_locked(
        &self,
        journal: &RestoreJournal,
        messages: &MessageStore,
    ) -> Result<(), LocalDataError> {
        let rollback = self.rollback_path(&journal.token);
        for name in ["Library", "SourceBooks", "Dictionaries", "ImportedBooks"] {
            let previous = rollback.join(name);
            match fs::symlink_metadata(&previous) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Ok(metadata) if metadata.file_type().is_dir() && !is_reparse_point(&metadata) => {}
                _ => return Err(LocalDataError::RecoveryFailed),
            }
            remove_directory_if_exists(&self.root.join(name))?;
            fs::rename(previous, self.root.join(name))
                .map_err(|_| LocalDataError::RecoveryFailed)?;
        }
        let message_backup = rollback.join(MESSAGE_BACKUP_ENTRY);
        match fs::symlink_metadata(&message_backup) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(metadata) if metadata.file_type().is_file() && !is_reparse_point(&metadata) => {
                messages
                    .restore_backup(message_backup)
                    .map_err(|_| LocalDataError::RecoveryFailed)?;
                messages
                    .recover_assets()
                    .map_err(|_| LocalDataError::RecoveryFailed)?;
            }
            _ => return Err(LocalDataError::RecoveryFailed),
        }
        sync_directory(&rollback).map_err(|_| LocalDataError::RecoveryFailed)?;
        sync_directory(&self.root).map_err(|_| LocalDataError::RecoveryFailed)
    }

    fn rollback_and_cleanup_locked(
        &self,
        journal: &RestoreJournal,
        messages: &MessageStore,
    ) -> Result<(), LocalDataError> {
        if ["Library", "SourceBooks", "Dictionaries", "ImportedBooks"]
            .iter()
            .any(|name| self.rollback_path(&journal.token).join(name).exists())
            || self.root.join(PUBLISH_FILE).is_file()
        {
            self.rollback_backend_locked(journal, messages)?;
        }
        remove_directory_if_exists(&self.stage_path(&journal.token))?;
        remove_directory_if_exists(&self.rollback_path(&journal.token))?;
        remove_file_if_exists(&self.root.join(COMMIT_FILE))?;
        remove_file_if_exists(&self.root.join(ROLLBACK_FILE))?;
        remove_file_if_exists(&self.root.join(PUBLISH_FILE))?;
        match fs::remove_file(self.journal_path()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(LocalDataError::RecoveryFailed),
        }
        sync_directory(&self.root).map_err(|_| LocalDataError::RecoveryFailed)
    }

    fn cleanup_orphans(&self) -> Result<(), LocalDataError> {
        for entry in fs::read_dir(&self.root).map_err(|_| LocalDataError::InvalidRoot)? {
            let entry = entry.map_err(|_| LocalDataError::InvalidRoot)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".atha-data-stage-") || name.starts_with(".atha-data-rollback-") {
                remove_directory_if_exists(&entry.path())?;
            } else if name.starts_with(".atha-data-restore.") && name.ends_with(".tmp") {
                remove_file_if_exists(&entry.path())?;
            }
        }
        remove_file_if_exists(&self.root.join(COMMIT_FILE))?;
        remove_file_if_exists(&self.root.join(ROLLBACK_FILE))?;
        remove_file_if_exists(&self.root.join(PUBLISH_FILE))?;
        Ok(())
    }

    fn read_optional_journal(&self) -> Result<Option<RestoreJournal>, LocalDataError> {
        match fs::read(self.journal_path()) {
            Ok(bytes) => {
                let mut journal: RestoreJournal =
                    serde_json::from_slice(&bytes).map_err(|_| LocalDataError::RecoveryFailed)?;
                validate_journal(&journal)?;
                if journal.state != RestoreState::Prepared {
                    return Err(LocalDataError::RecoveryFailed);
                }
                if let Some(publishing) = read_recovery_marker(&self.root.join(PUBLISH_FILE))? {
                    validate_marker(&journal, &publishing, RestoreState::Prepared)?;
                }
                if let Some(rollback) = read_recovery_marker(&self.root.join(ROLLBACK_FILE))? {
                    validate_marker(&journal, &rollback, RestoreState::RollbackPending)?;
                    journal.state = RestoreState::RollbackPending;
                } else if let Some(committed) = read_recovery_marker(&self.root.join(COMMIT_FILE))?
                {
                    validate_marker(&journal, &committed, RestoreState::Committed)?;
                    journal.state = RestoreState::Committed;
                }
                Ok(Some(journal))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(LocalDataError::RecoveryFailed),
        }
    }

    fn read_journal(&self) -> Result<RestoreJournal, LocalDataError> {
        self.read_optional_journal()?
            .ok_or(LocalDataError::UnknownRestore)
    }

    fn journal_path(&self) -> PathBuf {
        self.root.join(JOURNAL_FILE)
    }

    fn recovery_files_exist(&self) -> bool {
        [JOURNAL_FILE, PUBLISH_FILE, COMMIT_FILE, ROLLBACK_FILE]
            .iter()
            .any(|name| self.root.join(name).exists())
    }

    fn deletion_intents_exist(&self) -> Result<bool, LocalDataError> {
        for entry in fs::read_dir(&self.root).map_err(|_| LocalDataError::InvalidRoot)? {
            let entry = entry.map_err(|_| LocalDataError::InvalidRoot)?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_prefix(DELETE_PREFIX))
            else {
                continue;
            };
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LocalDataError::InvalidRoot)?;
            if !metadata.file_type().is_file() || !valid_content_id(id) {
                return Err(LocalDataError::UnsafeData);
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn stage_path(&self, token: &str) -> PathBuf {
        self.root.join(format!(".atha-data-stage-{token}"))
    }

    fn rollback_path(&self, token: &str) -> PathBuf {
        self.root.join(format!(".atha-data-rollback-{token}"))
    }
}

impl LocalDataError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid-local-data-root",
            Self::Busy => "local-data-busy",
            Self::InvalidBrowserState => "invalid-browser-state",
            Self::MissingBookSource => "missing-book-source",
            Self::InvalidBackup => "invalid-local-data-backup",
            Self::BackupFailed => "local-data-backup-failed",
            Self::RestoreFailed => "local-data-restore-failed",
            Self::RecoveryFailed => "local-data-recovery-failed",
            Self::UnsafeData => "unsafe-local-data",
            Self::UnknownRestore => "unknown-local-data-restore",
        }
    }
}

impl fmt::Display for LocalDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for LocalDataError {}

fn validate_stage(stage: &Path, messages: &MessageStore) -> Result<BrowserState, LocalDataError> {
    let library = LocalLibrary::open(stage).map_err(|_| LocalDataError::InvalidBackup)?;
    library
        .validate_durable_state()
        .map_err(|_| LocalDataError::InvalidBackup)?;
    let dictionaries = LocalDictionaries::open(stage).map_err(|_| LocalDataError::InvalidBackup)?;
    dictionaries
        .validate_durable_state()
        .map_err(|_| LocalDataError::InvalidBackup)?;
    let outer_files = collect_stage_file_count(stage)?;
    let outer_bytes = directory_bytes(stage)?;
    let remaining_files = MAX_LOCAL_DATA_FILES
        .checked_sub(
            outer_files
                .checked_add(1)
                .ok_or(LocalDataError::InvalidBackup)?,
        )
        .ok_or(LocalDataError::InvalidBackup)?;
    let remaining_bytes = MAX_LOCAL_DATA_BYTES
        .checked_sub(
            outer_bytes
                .checked_add(MAX_MANIFEST_BYTES)
                .ok_or(LocalDataError::InvalidBackup)?,
        )
        .ok_or(LocalDataError::InvalidBackup)?;
    messages
        .validate_backup(
            stage.join(MESSAGE_BACKUP_ENTRY),
            remaining_files,
            remaining_bytes,
        )
        .map_err(|_| LocalDataError::InvalidBackup)?;
    read_browser_state(&stage.join(BROWSER_STATE_ENTRY))
}

fn collect_stage_file_count(path: &Path) -> Result<usize, LocalDataError> {
    let mut total = 0_usize;
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| LocalDataError::InvalidBackup)? {
            let entry = entry.map_err(|_| LocalDataError::InvalidBackup)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LocalDataError::InvalidBackup)?;
            if is_reparse_point(&metadata) {
                return Err(LocalDataError::InvalidBackup);
            } else if metadata.file_type().is_dir() {
                pending.push(entry.path());
            } else if metadata.file_type().is_file() {
                total = total.checked_add(1).ok_or(LocalDataError::InvalidBackup)?;
            } else {
                return Err(LocalDataError::InvalidBackup);
            }
        }
    }
    Ok(total)
}

fn build_manifest(stage: &Path) -> Result<DataManifest, LocalDataError> {
    let mut files = Vec::new();
    collect_files(stage, stage, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.len() > MAX_LOCAL_DATA_FILES
        || files.iter().any(|file| !allowed_entry(&file.path))
        || !files.iter().any(|file| file.path == BROWSER_STATE_ENTRY)
        || !files.iter().any(|file| file.path == MESSAGE_BACKUP_ENTRY)
    {
        return Err(LocalDataError::UnsafeData);
    }
    validate_manifest(&DataManifest {
        schema: DATA_SCHEMA,
        files,
    })
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<DataFile>,
) -> Result<(), LocalDataError> {
    for entry in fs::read_dir(directory).map_err(|_| LocalDataError::BackupFailed)? {
        let entry = entry.map_err(|_| LocalDataError::BackupFailed)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| LocalDataError::BackupFailed)?;
        if is_reparse_point(&metadata) {
            return Err(LocalDataError::UnsafeData);
        }
        if metadata.file_type().is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if metadata.file_type().is_file() {
            let relative = portable_relative(root, &entry.path())?;
            let (hash, byte_length) = hash_file(&entry.path(), LocalDataError::BackupFailed)?;
            files.push(DataFile {
                path: relative,
                content_hash: hash,
                byte_length,
            });
            if files.len() > MAX_LOCAL_DATA_FILES {
                return Err(LocalDataError::UnsafeData);
            }
        } else {
            return Err(LocalDataError::UnsafeData);
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &DataManifest) -> Result<DataManifest, LocalDataError> {
    if manifest.schema != DATA_SCHEMA
        || manifest.files.is_empty()
        || manifest.files.len() >= MAX_LOCAL_DATA_FILES
    {
        return Err(LocalDataError::InvalidBackup);
    }
    let mut total = 0_u64;
    let mut previous = None;
    for file in &manifest.files {
        if file.byte_length == 0
            || !is_hash(&file.content_hash)
            || !allowed_entry(&file.path)
            || previous.is_some_and(|path: &str| path >= file.path.as_str())
        {
            return Err(LocalDataError::InvalidBackup);
        }
        total = total
            .checked_add(file.byte_length)
            .ok_or(LocalDataError::InvalidBackup)?;
        previous = Some(file.path.as_str());
    }
    if total > MAX_LOCAL_DATA_BYTES - MAX_MANIFEST_BYTES
        || !manifest
            .files
            .iter()
            .any(|file| file.path == BROWSER_STATE_ENTRY)
        || !manifest
            .files
            .iter()
            .any(|file| file.path == MESSAGE_BACKUP_ENTRY)
    {
        return Err(LocalDataError::InvalidBackup);
    }
    Ok(manifest.clone())
}

fn write_archive(
    stage: &Path,
    target: &Path,
    manifest: &DataManifest,
) -> Result<(), LocalDataError> {
    let manifest_bytes =
        serde_json::to_vec_pretty(manifest).map_err(|_| LocalDataError::BackupFailed)?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(LocalDataError::BackupFailed);
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| LocalDataError::BackupFailed)?;
    let mut archive = ZipWriter::new(file);
    let compressed = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    archive
        .start_file(MANIFEST_ENTRY, compressed)
        .map_err(|_| LocalDataError::BackupFailed)?;
    archive
        .write_all(&manifest_bytes)
        .map_err(|_| LocalDataError::BackupFailed)?;
    for entry in &manifest.files {
        archive
            .start_file(&entry.path, compressed)
            .map_err(|_| LocalDataError::BackupFailed)?;
        let mut source = File::open(path_for_entry(stage, &entry.path)?)
            .map_err(|_| LocalDataError::BackupFailed)?;
        std::io::copy(&mut source, &mut archive).map_err(|_| LocalDataError::BackupFailed)?;
    }
    let file = archive.finish().map_err(|_| LocalDataError::BackupFailed)?;
    file.sync_all().map_err(|_| LocalDataError::BackupFailed)
}

fn inspect_archive(path: &Path) -> Result<DataManifest, LocalDataError> {
    let mut archive = open_archive(path)?;
    if archive.len() < 3
        || archive.len() > MAX_LOCAL_DATA_FILES
        || archive
            .has_overlapping_files()
            .map_err(|_| LocalDataError::InvalidBackup)?
    {
        return Err(LocalDataError::InvalidBackup);
    }
    let mut entries = HashMap::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| LocalDataError::InvalidBackup)?;
        if entry.is_dir()
            || entry.is_symlink()
            || entries
                .insert(entry.name().to_owned(), entry.size())
                .is_some()
        {
            return Err(LocalDataError::InvalidBackup);
        }
    }
    let manifest_bytes = {
        let mut entry = archive
            .by_name(MANIFEST_ENTRY)
            .map_err(|_| LocalDataError::InvalidBackup)?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err(LocalDataError::InvalidBackup);
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .by_ref()
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LocalDataError::InvalidBackup)?;
        bytes
    };
    if manifest_bytes.len() as u64 != entries[MANIFEST_ENTRY] {
        return Err(LocalDataError::InvalidBackup);
    }
    let manifest: DataManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| LocalDataError::InvalidBackup)?;
    let manifest = validate_manifest(&manifest)?;
    let expected = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .chain(std::iter::once(MANIFEST_ENTRY))
        .collect::<HashSet<_>>();
    if entries.keys().map(String::as_str).collect::<HashSet<_>>() != expected {
        return Err(LocalDataError::InvalidBackup);
    }
    for file in &manifest.files {
        if entries.get(&file.path) != Some(&file.byte_length) {
            return Err(LocalDataError::InvalidBackup);
        }
        verify_entry_hash(&mut archive, file)?;
    }
    Ok(manifest)
}

fn extract_archive(
    source: &Path,
    stage: &Path,
    manifest: &DataManifest,
) -> Result<(), LocalDataError> {
    let mut archive = open_archive(source)?;
    for file in &manifest.files {
        let target = path_for_entry(stage, &file.path)?;
        let parent = target.parent().ok_or(LocalDataError::InvalidBackup)?;
        fs::create_dir_all(parent).map_err(|_| LocalDataError::RestoreFailed)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|_| LocalDataError::RestoreFailed)?;
        let mut entry = archive
            .by_name(&file.path)
            .map_err(|_| LocalDataError::InvalidBackup)?;
        let copied = std::io::copy(
            &mut Read::take(&mut entry, file.byte_length.saturating_add(1)),
            &mut output,
        )
        .map_err(|_| LocalDataError::RestoreFailed)?;
        if copied != file.byte_length {
            return Err(LocalDataError::InvalidBackup);
        }
        output
            .sync_all()
            .map_err(|_| LocalDataError::RestoreFailed)?;
        let (hash, length) = hash_file(&target, LocalDataError::InvalidBackup)?;
        if length != file.byte_length || hash != file.content_hash {
            return Err(LocalDataError::InvalidBackup);
        }
    }
    Ok(())
}

fn open_archive(path: &Path) -> Result<ZipArchive<File>, LocalDataError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| LocalDataError::InvalidBackup)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_LOCAL_DATA_BYTES {
        return Err(LocalDataError::InvalidBackup);
    }
    ZipArchive::new(File::open(path).map_err(|_| LocalDataError::InvalidBackup)?)
        .map_err(|_| LocalDataError::InvalidBackup)
}

fn verify_entry_hash(
    archive: &mut ZipArchive<File>,
    expected: &DataFile,
) -> Result<(), LocalDataError> {
    let mut entry = archive
        .by_name(&expected.path)
        .map_err(|_| LocalDataError::InvalidBackup)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = entry
            .read(&mut buffer)
            .map_err(|_| LocalDataError::InvalidBackup)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(LocalDataError::InvalidBackup)?;
        if total > expected.byte_length {
            return Err(LocalDataError::InvalidBackup);
        }
        digest.update(&buffer[..read]);
    }
    if total != expected.byte_length || hex(&digest.finalize()) != expected.content_hash {
        Err(LocalDataError::InvalidBackup)
    } else {
        Ok(())
    }
}

fn hash_file(path: &Path, error: LocalDataError) -> Result<(String, u64), LocalDataError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if !metadata.file_type().is_file() {
        return Err(error);
    }
    let mut file = File::open(path).map_err(|_| error)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|_| error)?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or(error)?;
        if total > MAX_LOCAL_DATA_BYTES {
            return Err(error);
        }
        digest.update(&buffer[..read]);
    }
    Ok((hex(&digest.finalize()), total))
}

fn allowed_entry(path: &str) -> bool {
    if matches!(path, BROWSER_STATE_ENTRY | MESSAGE_BACKUP_ENTRY) {
        return true;
    }
    let parts = path.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        ["Library", record] => record.strip_suffix(".json").is_some_and(valid_content_id),
        ["SourceBooks", source] => source
            .rsplit_once('.')
            .is_some_and(|(id, extension)| valid_content_id(id) && valid_book_extension(extension)),
        ["Dictionaries", id, name] if valid_content_id(id) => {
            matches!(
                *name,
                "dictionary.json" | "dictionary.mdx" | "dictionary.mobi" | "dictionary.offsets"
            ) || name
                .strip_prefix("resource-")
                .and_then(|value| value.strip_suffix(".mdd"))
                .is_some_and(|value| {
                    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                })
        }
        _ => false,
    }
}

fn path_for_entry(root: &Path, entry: &str) -> Result<PathBuf, LocalDataError> {
    if !allowed_entry(entry) {
        return Err(LocalDataError::InvalidBackup);
    }
    Ok(entry
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part)))
}

fn portable_relative(root: &Path, path: &Path) -> Result<String, LocalDataError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| LocalDataError::UnsafeData)?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                parts.push(value.to_str().ok_or(LocalDataError::UnsafeData)?.to_owned())
            }
            _ => return Err(LocalDataError::UnsafeData),
        }
    }
    Ok(parts.join("/"))
}

fn valid_browser_key(key: &str) -> bool {
    if matches!(
        key,
        "atha.reader.application.v1"
            | "atha.reader.statistics.v1"
            | "atha.reader.dictionary.preferences.v1"
    ) {
        return true;
    }
    ["book", "progress", "annotations"].into_iter().any(|kind| {
        key.strip_prefix(&format!("atha.reader.{kind}."))
            .and_then(|value| value.strip_suffix(".v1"))
            .is_some_and(valid_book_key)
    })
}

fn valid_browser_value(key: &str, raw: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("schema").and_then(Value::as_u64) != Some(1) {
        return false;
    }
    if key == "atha.reader.application.v1" {
        exact_json_keys(object, &["preferences", "schema"])
            && object.get("preferences").is_some_and(Value::is_object)
    } else if key == "atha.reader.statistics.v1" {
        exact_json_keys(object, &["books", "days", "schema"])
            && object.get("books").is_some_and(Value::is_array)
            && object.get("days").is_some_and(Value::is_array)
    } else if key == "atha.reader.dictionary.preferences.v1" {
        exact_json_keys(object, &["dictionaryId", "fontScale", "schema"])
    } else if key.starts_with("atha.reader.book.") {
        exact_json_keys(object, &["bookmarks", "preferences", "schema"])
            && object.get("bookmarks").is_some_and(Value::is_array)
            && object.get("preferences").is_some_and(Value::is_object)
    } else if key.starts_with("atha.reader.progress.") {
        exact_json_keys(object, &["contentVersion", "locator", "schema"])
            && object
                .get("contentVersion")
                .and_then(Value::as_str)
                .is_some_and(valid_content_id)
            && object.get("locator").is_some_and(Value::is_string)
    } else if key.starts_with("atha.reader.annotations.") {
        exact_json_keys(object, &["items", "schema"])
            && object.get("items").is_some_and(|items| {
                items.as_array().is_some_and(|items| {
                    items.iter().all(|item| {
                        let Some(anchor) = item.get("sourceAnchor") else {
                            return false;
                        };
                        let (Some(text), Some(hash)) = (
                            anchor.get("selectedText").and_then(Value::as_str),
                            anchor.get("contentHash").and_then(Value::as_str),
                        ) else {
                            return false;
                        };
                        hex(&Sha256::digest(text.as_bytes())) == hash
                    })
                })
            })
    } else {
        false
    }
}

fn exact_json_keys(object: &serde_json::Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

fn read_browser_state(path: &Path) -> Result<BrowserState, LocalDataError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| LocalDataError::InvalidBackup)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_BROWSER_STATE_BYTES {
        return Err(LocalDataError::InvalidBackup);
    }
    let state = serde_json::from_slice::<BrowserState>(
        &fs::read(path).map_err(|_| LocalDataError::InvalidBackup)?,
    )
    .map_err(|_| LocalDataError::InvalidBackup)?;
    state
        .validate()
        .map_err(|_| LocalDataError::InvalidBackup)?;
    Ok(state)
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(std::io::Error::other)?;
    file.sync_all()
}

fn write_journal(path: &Path, journal: &RestoreJournal) -> Result<(), LocalDataError> {
    validate_journal(journal)?;
    if journal.state != RestoreState::Prepared {
        return Err(LocalDataError::RecoveryFailed);
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| LocalDataError::RecoveryFailed)?;
        serde_json::to_writer_pretty(&mut file, journal)
            .map_err(|_| LocalDataError::RecoveryFailed)?;
        file.write_all(b"\n")
            .and_then(|()| file.sync_all())
            .map_err(|_| LocalDataError::RecoveryFailed)?;
        fs::rename(&temporary, path).map_err(|_| LocalDataError::RecoveryFailed)?;
        sync_directory(path.parent().ok_or(LocalDataError::RecoveryFailed)?)
            .map_err(|_| LocalDataError::RecoveryFailed)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn write_recovery_marker(path: &Path, marker: &RestoreJournal) -> Result<(), LocalDataError> {
    validate_journal(marker)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(LocalDataError::RecoveryFailed),
    }
    let temporary = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        write_json_file(&temporary, marker).map_err(|_| LocalDataError::RecoveryFailed)?;
        #[cfg(target_os = "android")]
        fs::rename(&temporary, path).map_err(|_| LocalDataError::RecoveryFailed)?;
        #[cfg(not(target_os = "android"))]
        fs::hard_link(&temporary, path).map_err(|_| LocalDataError::RecoveryFailed)?;
        let parent = path.parent().ok_or(LocalDataError::RecoveryFailed)?;
        sync_directory(parent).map_err(|_| LocalDataError::RecoveryFailed)?;
        remove_file_if_exists(&temporary)?;
        sync_directory(parent).map_err(|_| LocalDataError::RecoveryFailed)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn read_recovery_marker(path: &Path) -> Result<Option<RestoreJournal>, LocalDataError> {
    match fs::read(path) {
        Ok(bytes) => {
            let marker =
                serde_json::from_slice(&bytes).map_err(|_| LocalDataError::RecoveryFailed)?;
            validate_journal(&marker)?;
            Ok(Some(marker))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(LocalDataError::RecoveryFailed),
    }
}

fn validate_marker(
    journal: &RestoreJournal,
    marker: &RestoreJournal,
    expected: RestoreState,
) -> Result<(), LocalDataError> {
    if marker.schema == journal.schema && marker.token == journal.token && marker.state == expected
    {
        Ok(())
    } else {
        Err(LocalDataError::RecoveryFailed)
    }
}

fn validate_journal(journal: &RestoreJournal) -> Result<(), LocalDataError> {
    if journal.schema == DATA_SCHEMA {
        validate_token(&journal.token)
    } else {
        Err(LocalDataError::RecoveryFailed)
    }
}

fn validate_token(token: &str) -> Result<(), LocalDataError> {
    if token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(LocalDataError::UnknownRestore)
    }
}

fn temporary_token() -> Result<String, LocalDataError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LocalDataError::InvalidRoot)?
        .as_nanos();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let digest = Sha256::digest(format!("{}:{now}:{sequence}", std::process::id()).as_bytes());
    Ok(hex(&digest[..16]))
}

fn adjacent_temporary(target: &Path, token: &str) -> Result<PathBuf, LocalDataError> {
    let parent = target.parent().ok_or(LocalDataError::BackupFailed)?;
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(LocalDataError::BackupFailed)?;
    Ok(parent.join(format!(".{name}.{token}.tmp")))
}

fn publish_archive(temporary: &Path, target: &Path) -> Result<(), LocalDataError> {
    #[cfg(target_os = "android")]
    fs::rename(temporary, target).map_err(|_| LocalDataError::BackupFailed)?;
    #[cfg(not(target_os = "android"))]
    fs::hard_link(temporary, target).map_err(|_| LocalDataError::BackupFailed)?;
    sync_directory(target.parent().ok_or(LocalDataError::BackupFailed)?)
        .map_err(|_| LocalDataError::BackupFailed)
}

fn swap_directory(current: &Path, staged: &Path, rollback: &Path) -> Result<(), LocalDataError> {
    if !real_directory(current) || !real_directory(staged) || rollback.exists() {
        return Err(LocalDataError::RestoreFailed);
    }
    fs::rename(current, rollback).map_err(|_| LocalDataError::RestoreFailed)?;
    if fs::rename(staged, current).is_err() {
        let _ = fs::rename(rollback, current);
        return Err(LocalDataError::RestoreFailed);
    }
    Ok(())
}

fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !is_reparse_point(&metadata))
}

fn remove_directory_if_exists(path: &Path) -> Result<(), LocalDataError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !is_reparse_point(&metadata) => {
            fs::remove_dir_all(path).map_err(|_| LocalDataError::RecoveryFailed)
        }
        Ok(_) => Err(LocalDataError::RecoveryFailed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(LocalDataError::RecoveryFailed),
    }
}

fn remove_file_if_exists(path: &Path) -> Result<(), LocalDataError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(LocalDataError::RecoveryFailed),
    }
}

fn directory_bytes(path: &Path) -> Result<u64, LocalDataError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(LocalDataError::UnsafeData),
    };
    if !metadata.file_type().is_dir() || is_reparse_point(&metadata) {
        return Err(LocalDataError::UnsafeData);
    }
    let mut total = 0_u64;
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| LocalDataError::UnsafeData)? {
            let entry = entry.map_err(|_| LocalDataError::UnsafeData)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| LocalDataError::UnsafeData)?;
            if is_reparse_point(&metadata) {
                return Err(LocalDataError::UnsafeData);
            }
            if metadata.file_type().is_dir() {
                pending.push(entry.path());
            } else if metadata.file_type().is_file() {
                total = total
                    .checked_add(metadata.len())
                    .ok_or(LocalDataError::UnsafeData)?;
            } else {
                return Err(LocalDataError::UnsafeData);
            }
        }
    }
    Ok(total)
}

fn valid_content_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_book_key(value: &str) -> bool {
    value.len() == 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_book_extension(value: &str) -> bool {
    BOOK_EXTENSIONS.contains(&value)
}

fn is_hash(value: &str) -> bool {
    valid_content_id(value)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    value
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    File::open(path)?.sync_all()
}

#[cfg(unix)]
fn sync_directory_tree(path: &Path) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if is_reparse_point(&metadata) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "directory tree contains a reparse point",
            ));
        }
        if metadata.file_type().is_dir() {
            sync_directory_tree(&entry.path())?;
        }
    }
    sync_directory(path)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory_tree(_path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    struct Root(PathBuf);

    impl Root {
        fn new(label: &str) -> Self {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(".tmp")
                .join(format!("local-data-unit-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test root");
            Self(path)
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn failure_after_directory_publish_keeps_a_confirmable_rollback() {
        let source = Root::new("source");
        let source_file = source.0.join("source.txt");
        fs::write(&source_file, "新资料\n").expect("write source");
        let source_library = LocalLibrary::open(&source.0).expect("open source library");
        source_library
            .stage_with_title_hint(&source_file, None)
            .expect("stage source book");
        let source_dictionaries = LocalDictionaries::open(&source.0).expect("open dictionaries");
        let source_messages = MessageStore::open(&source.0).expect("open messages");
        let archive = source.0.join("source.atha-data");
        LocalData::open(&source.0)
            .expect("open source data")
            .create_backup(
                &archive,
                &BrowserState {
                    schema: 1,
                    records: Vec::new(),
                },
                &source_library,
                &source_dictionaries,
                &source_messages,
            )
            .expect("create backup");

        let destination = Root::new("destination");
        let current_file = destination.0.join("current.txt");
        fs::write(&current_file, "旧资料\n").expect("write current");
        let current_library = LocalLibrary::open(&destination.0).expect("open current library");
        let current = current_library
            .stage_with_title_hint(&current_file, None)
            .expect("stage current book");
        LocalDictionaries::open(&destination.0).expect("open current dictionaries");
        let messages = MessageStore::open(&destination.0).expect("open current messages");
        let data = LocalData::open(&destination.0).expect("open data");
        let previous = BrowserState {
            schema: 1,
            records: Vec::new(),
        };
        let pending = data
            .prepare_restore(&archive, &previous, &messages)
            .expect("prepare restore");

        assert_eq!(
            data.commit_restore_with(&pending.token, &messages, || {
                Err(LocalDataError::RestoreFailed)
            }),
            Err(LocalDataError::RestoreFailed)
        );
        let rollback = data
            .pending_restore()
            .expect("read pending")
            .expect("rollback pending");
        assert!(rollback.rollback);
        assert_eq!(rollback.browser_state, previous);
        assert_eq!(
            LocalLibrary::open(&destination.0)
                .expect("reopen library")
                .list()
                .expect("list library")[0]
                .id,
            current.id
        );
    }

    #[test]
    fn shared_operation_blocks_exclusive_backup_until_drop() {
        let root = Root::new("operation-lock");
        let library = LocalLibrary::open(&root.0).expect("open library");
        let dictionaries = LocalDictionaries::open(&root.0).expect("open dictionaries");
        let messages = MessageStore::open(&root.0).expect("open messages");
        let data = LocalData::open(&root.0).expect("open data");
        let target = root.0.join("blocked.atha-data");
        let guard = data.operation_guard().expect("start shared operation");
        assert!(matches!(data.deletion_guard(), Err(LocalDataError::Busy)));
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender
                .send(data.create_backup(
                    target,
                    &BrowserState {
                        schema: 1,
                        records: Vec::new(),
                    },
                    &library,
                    &dictionaries,
                    &messages,
                ))
                .expect("send result");
        });
        assert_eq!(
            receiver.recv().expect("receive result"),
            Err(LocalDataError::Busy)
        );
        drop(guard);
        worker.join().expect("join worker");
    }

    #[test]
    fn reopened_recovery_rolls_back_a_partial_directory_publish() {
        let source = Root::new("reopen-source");
        let source_file = source.0.join("source.txt");
        fs::write(&source_file, "新资料\n").expect("write source");
        let source_library = LocalLibrary::open(&source.0).expect("open source library");
        source_library
            .stage_with_title_hint(&source_file, None)
            .expect("stage source book");
        let source_dictionaries = LocalDictionaries::open(&source.0).expect("open dictionaries");
        let source_messages = MessageStore::open(&source.0).expect("open messages");
        let source_data = LocalData::open(&source.0).expect("open source data");
        let archive = source.0.join("source.atha-data");
        source_data
            .create_backup(
                &archive,
                &BrowserState {
                    schema: 1,
                    records: Vec::new(),
                },
                &source_library,
                &source_dictionaries,
                &source_messages,
            )
            .expect("create backup");

        let destination = Root::new("reopen-destination");
        let current_file = destination.0.join("current.txt");
        fs::write(&current_file, "旧资料\n").expect("write current");
        let current_library = LocalLibrary::open(&destination.0).expect("open current library");
        let current = current_library
            .stage_with_title_hint(&current_file, None)
            .expect("stage current book");
        LocalDictionaries::open(&destination.0).expect("open current dictionaries");
        let messages = MessageStore::open(&destination.0).expect("open current messages");
        let data = LocalData::open(&destination.0).expect("open data");
        let pending = data
            .prepare_restore(
                &archive,
                &BrowserState {
                    schema: 1,
                    records: Vec::new(),
                },
                &messages,
            )
            .expect("prepare restore");
        data.interrupt_restore_after_swap(&pending.token, &["Library", "SourceBooks"])
            .expect("leave partial publish");
        assert_eq!(
            data.abort_restore(&pending.token),
            Err(LocalDataError::RecoveryFailed)
        );
        drop(data);
        drop(messages);

        let reopened_messages = MessageStore::open(&destination.0).expect("reopen messages");
        let reopened = LocalData::open(&destination.0).expect("reopen data");
        reopened
            .recover(&reopened_messages)
            .expect("recover after restart");
        assert_eq!(
            LocalLibrary::open(&destination.0)
                .expect("reopen library")
                .list()
                .expect("list library")[0]
                .id,
            current.id
        );
        assert_eq!(reopened.pending_restore().expect("no pending"), None);
    }
}
