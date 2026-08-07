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

const DATABASE_VERSION: i64 = 2;
const DATABASE_NAME: &str = "Messages.sqlite3";
const ASSET_TEMP_PREFIX: &str = ".atha-asset-";

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
                let asset_path = self.assets.join(&asset_name);
                match fs::symlink_metadata(&asset_path) {
                    Ok(_) if asset_file_matches(&asset_path, &hash, input.bytes.len() as i64) => {}
                    Ok(_) => return Err(MessageError::CorruptData),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        self.publish_asset(&asset_name, &hash, &input.bytes)?;
                    }
                    Err(_) => return Err(MessageError::InvalidRoot),
                }
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

    fn publish_asset(
        &self,
        asset_name: &str,
        expected_hash: &[u8],
        bytes: &[u8],
    ) -> Result<(), MessageError> {
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
                if !asset_file_matches(&asset_path, expected_hash, bytes.len() as i64) {
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
                || !asset_file_matches(&self.assets.join(name), &hash, byte_length)
            {
                return Ok(false);
            }
        }
        Ok(true)
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

fn asset_file_matches(path: &Path, expected_hash: &[u8], byte_length: i64) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() || byte_length < 0 || metadata.len() != byte_length as u64 {
        return false;
    }
    fs::read(path).is_ok_and(|bytes| Sha256::digest(bytes).as_slice() == expected_hash)
}
