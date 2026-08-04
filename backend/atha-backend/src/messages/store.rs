use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, TransactionBehavior};
use sha2::{Digest, Sha256};

use super::{
    model::{MessageError, PreparedResource, SnapshotResourceInput},
    schema::SCHEMA_V1,
    util::encode_hex,
};

const DATABASE_VERSION: i64 = 1;
const DATABASE_NAME: &str = "Messages.sqlite3";

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
        Ok(store)
    }

    pub(crate) fn prepare_resources(
        &self,
        inputs: &[SnapshotResourceInput],
    ) -> Result<Vec<PreparedResource>, MessageError> {
        inputs
            .iter()
            .map(|input| {
                let hash = Sha256::digest(&input.bytes);
                let asset_name = encode_hex(&hash);
                let asset_path = self.assets.join(&asset_name);
                match OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&asset_path)
                {
                    Ok(mut file) => file
                        .write_all(&input.bytes)
                        .map_err(|_| MessageError::InvalidRoot)?,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let existing =
                            fs::read(&asset_path).map_err(|_| MessageError::CorruptData)?;
                        if Sha256::digest(&existing).as_slice() != hash.as_slice() {
                            return Err(MessageError::CorruptData);
                        }
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

    fn migrate(&self) -> Result<(), MessageError> {
        let mut connection = self.connect()?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| MessageError::Database)?;
        if version > DATABASE_VERSION {
            return Err(MessageError::FutureDatabase);
        }
        if version == DATABASE_VERSION {
            return Ok(());
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute_batch(SCHEMA_V1)
            .map_err(|_| MessageError::Database)?;
        transaction
            .pragma_update(None, "user_version", DATABASE_VERSION)
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)
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
