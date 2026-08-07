use std::{
    collections::HashSet,
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

use super::{
    model::{MessageError, PreparedResource, SnapshotResourceInput, StoreHealth},
    schema::{SCHEMA_V1, SCHEMA_V2},
    util::encode_hex,
};

pub(crate) const DATABASE_VERSION: i64 = 2;
const DATABASE_NAME: &str = "Messages.sqlite3";
const ASSET_TEMP_PREFIX: &str = ".atha-asset-";
const MAINTENANCE_LOCK: &str = ".atha-maintenance.lock";

#[derive(Clone, Debug)]
pub struct MessageStore {
    pub(crate) database: PathBuf,
    pub(crate) assets: PathBuf,
}

impl MessageStore {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, MessageError> {
        let root = data_root.as_ref().join("Messages");
        let assets = root.join("Assets");
        fs::create_dir_all(&assets).map_err(|_| MessageError::InvalidRoot)?;
        let store = Self {
            database: root.join(DATABASE_NAME),
            assets,
        };
        store.ensure_assets_directory()?;
        let maintenance = store.maintenance_file()?;
        maintenance.try_lock().map_err(|_| MessageError::Database)?;
        store.migrate()?;
        store.recover_assets()?;
        Ok(store)
    }

    pub fn health(&self) -> Result<StoreHealth, MessageError> {
        let connection = self.connect()?;
        let schema_version = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| MessageError::Database)?;
        let sqlite_version = connection
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(|_| MessageError::Database)?;
        let foreign_keys = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .map_err(|_| MessageError::Database)?;
        let fts5 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'message_search' AND sql LIKE '%fts5%')",
                [],
                |row| row.get(0),
            )
            .map_err(|_| MessageError::Database)?;
        let integrity: String = connection
            .pragma_query_value(None, "integrity_check", |row| row.get(0))
            .map_err(|_| MessageError::Database)?;
        let mut foreign_key_check = connection
            .prepare("PRAGMA foreign_key_check")
            .map_err(|_| MessageError::Database)?;
        let foreign_key_violation = foreign_key_check
            .query([])
            .and_then(|mut rows| rows.next().map(|row| row.is_some()))
            .map_err(|_| MessageError::Database)?;
        drop(foreign_key_check);
        let assets_integrity = self.assets_integrity(&connection)?;
        Ok(StoreHealth {
            schema_version,
            sqlite_version,
            foreign_keys,
            fts5,
            integrity: integrity == "ok" && !foreign_key_violation && assets_integrity,
        })
    }

    pub(crate) fn prepare_resources(
        &self,
        _write_lock: &Transaction<'_>,
        inputs: &[SnapshotResourceInput],
    ) -> Result<Vec<PreparedResource>, MessageError> {
        inputs
            .iter()
            .map(|input| {
                let hash = Sha256::digest(&input.bytes);
                let asset_name = encode_hex(&hash);
                self.ensure_asset(_write_lock, &asset_name, &hash, &input.bytes)?;
                Ok(PreparedResource {
                    path: input.path.clone(),
                    media_type: input.media_type.clone(),
                    content_hash: hash.to_vec(),
                    byte_length: input.bytes.len() as i64,
                    asset_name,
                })
            })
            .collect()
    }

    pub(crate) fn ensure_asset(
        &self,
        _write_lock: &Transaction<'_>,
        asset_name: &str,
        expected_hash: &[u8],
        bytes: &[u8],
    ) -> Result<(), MessageError> {
        if !is_asset_name(asset_name)
            || expected_hash.len() != 32
            || encode_hex(expected_hash) != asset_name
        {
            return Err(MessageError::CorruptData);
        }
        let asset_path = self.assets.join(asset_name);
        match fs::symlink_metadata(&asset_path) {
            Ok(_) => self
                .read_asset(asset_name, expected_hash, bytes.len() as u64)
                .map(|_| ()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.publish_asset(asset_name, expected_hash, bytes)
            }
            Err(_) => Err(MessageError::InvalidRoot),
        }
    }

    pub(crate) fn restore_asset(
        &self,
        _write_lock: &Transaction<'_>,
        asset_name: &str,
        expected_hash: &[u8],
        bytes: &[u8],
    ) -> Result<(), MessageError> {
        if self
            .read_asset(asset_name, expected_hash, bytes.len() as u64)
            .is_ok()
        {
            return Ok(());
        }
        self.publish_asset(asset_name, expected_hash, bytes)
    }

    fn publish_asset(
        &self,
        asset_name: &str,
        expected_hash: &[u8],
        bytes: &[u8],
    ) -> Result<(), MessageError> {
        self.ensure_assets_directory()?;
        let asset_path = self.assets.join(asset_name);
        let temporary = self.assets.join(format!(
            "{ASSET_TEMP_PREFIX}{asset_name}-{}.tmp",
            std::process::id()
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|_| MessageError::InvalidRoot)?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| MessageError::InvalidRoot)?;
            drop(file);
            if fs::rename(&temporary, &asset_path).is_err() {
                if self
                    .read_asset(asset_name, expected_hash, bytes.len() as u64)
                    .is_err()
                {
                    return Err(MessageError::InvalidRoot);
                }
                fs::remove_file(&temporary).map_err(|_| MessageError::InvalidRoot)?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn recover_assets(&self) -> Result<(), MessageError> {
        self.ensure_assets_directory()?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let mut statement = transaction
            .prepare("SELECT DISTINCT asset_name FROM snapshot_resource")
            .map_err(|_| MessageError::Database)?;
        let referenced = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| MessageError::Database)?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(|_| MessageError::Database)?;
        drop(statement);
        if referenced.iter().any(|name| !is_asset_name(name)) {
            return Err(MessageError::CorruptData);
        }

        for entry in fs::read_dir(&self.assets).map_err(|_| MessageError::InvalidRoot)? {
            let entry = entry.map_err(|_| MessageError::InvalidRoot)?;
            let file_type = entry.file_type().map_err(|_| MessageError::InvalidRoot)?;
            if !file_type.is_file() && !file_type.is_symlink() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let temporary = name.starts_with(ASSET_TEMP_PREFIX) && name.ends_with(".tmp");
            let orphan = is_asset_name(&name) && !referenced.contains(&name);
            if temporary || orphan {
                fs::remove_file(entry.path()).map_err(|_| MessageError::InvalidRoot)?;
            }
        }
        transaction.commit().map_err(|_| MessageError::Database)
    }

    fn assets_integrity(&self, connection: &Connection) -> Result<bool, MessageError> {
        let mut statement = connection
            .prepare("SELECT DISTINCT asset_name, content_hash, byte_length FROM snapshot_resource")
            .map_err(|_| MessageError::Database)?;
        let resources = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|_| MessageError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MessageError::Database)?;
        for (name, hash, byte_length) in resources {
            if !is_asset_name(&name)
                || hash.len() != 32
                || encode_hex(&hash) != name
                || u64::try_from(byte_length)
                    .ok()
                    .is_none_or(|length| self.read_asset(&name, &hash, length).is_err())
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn read_asset(
        &self,
        asset_name: &str,
        expected_hash: &[u8],
        byte_length: u64,
    ) -> Result<Vec<u8>, MessageError> {
        self.ensure_assets_directory()?;
        if !is_asset_name(asset_name)
            || expected_hash.len() != 32
            || encode_hex(expected_hash) != asset_name
        {
            return Err(MessageError::CorruptData);
        }
        let path = self.assets.join(asset_name);
        let metadata = fs::symlink_metadata(&path).map_err(|_| MessageError::CorruptData)?;
        if !metadata.file_type().is_file() || metadata.len() != byte_length {
            return Err(MessageError::CorruptData);
        }
        let bytes = fs::read(path).map_err(|_| MessageError::CorruptData)?;
        if bytes.len() as u64 != byte_length || Sha256::digest(&bytes).as_slice() != expected_hash {
            return Err(MessageError::CorruptData);
        }
        Ok(bytes)
    }

    fn ensure_assets_directory(&self) -> Result<(), MessageError> {
        let metadata = fs::symlink_metadata(&self.assets).map_err(|_| MessageError::InvalidRoot)?;
        if !metadata.file_type().is_dir() || is_reparse_point(&metadata) {
            return Err(MessageError::InvalidRoot);
        }
        Ok(())
    }

    pub(crate) fn maintenance_file(&self) -> Result<fs::File, MessageError> {
        self.ensure_assets_directory()?;
        let path = self.assets.join(MAINTENANCE_LOCK);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|_| MessageError::InvalidRoot)?;
        let metadata = fs::symlink_metadata(path).map_err(|_| MessageError::InvalidRoot)?;
        if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
            return Err(MessageError::InvalidRoot);
        }
        Ok(file)
    }

    fn migrate(&self) -> Result<(), MessageError> {
        let mut connection = self.connect()?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| MessageError::Database)?;
        if version > DATABASE_VERSION {
            return Err(MessageError::FutureDatabase);
        }
        for next in version + 1..=DATABASE_VERSION {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute_batch(match next {
                    1 => SCHEMA_V1,
                    2 => SCHEMA_V2,
                    _ => return Err(MessageError::Database),
                })
                .map_err(|_| MessageError::Database)?;
            transaction
                .pragma_update(None, "user_version", next)
                .map_err(|_| MessageError::Database)?;
            transaction.commit().map_err(|_| MessageError::Database)?;
        }
        Ok(())
    }

    pub(crate) fn connect(&self) -> Result<Connection, MessageError> {
        let connection = Connection::open(&self.database).map_err(|_| MessageError::Database)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|_| MessageError::Database)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| MessageError::Database)?;
        Ok(connection)
    }
}

fn is_asset_name(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
