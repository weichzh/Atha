use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use rusqlite::{
    Connection, OpenFlags, TransactionBehavior,
    backup::{Backup, StepResult},
    params,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::{
    model::{
        EditionInput, MessageError, SnapshotResourceInput, SourceAnchorInput, SourceSnapshotInput,
        validate_edition, validate_range_locator, validate_source, validate_stored_revision,
    },
    schema::{SCHEMA_V1, SCHEMA_V2},
    store::{DATABASE_VERSION, MessageStore},
    util::{decode_hex, encode_hex},
};

const BACKUP_SCHEMA: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const DATABASE_ENTRY: &str = "Messages.sqlite3";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BACKUP_ASSETS: usize = 65_536;
// ponytail: 8 GiB local-backup ceiling; make configurable only when real stores approach it.
const MAX_BACKUP_BYTES: u64 = 8 * 1024 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupManifest {
    schema: u32,
    database: BackupFile,
    assets: Vec<BackupAsset>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupFile {
    content_hash: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupAsset {
    content_hash: String,
    byte_length: u64,
}

impl MessageStore {
    pub fn create_backup(&self, target: impl AsRef<Path>) -> Result<(), MessageError> {
        let maintenance = self.maintenance_file().map_err(|_| MessageError::Backup)?;
        maintenance
            .try_lock_shared()
            .map_err(|_| MessageError::Backup)?;
        self.create_backup_locked(target.as_ref())
    }

    pub fn restore_backup(&self, source: impl AsRef<Path>) -> Result<(), MessageError> {
        let maintenance = self.maintenance_file().map_err(|_| MessageError::Restore)?;
        maintenance.try_lock().map_err(|_| MessageError::Restore)?;
        self.restore_backup_locked(source.as_ref())
    }

    fn create_backup_locked(&self, target: &Path) -> Result<(), MessageError> {
        match fs::symlink_metadata(target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(MessageError::Backup),
        }
        let suffix = temp_suffix();
        let database_temp = self
            .assets
            .join(format!(".atha-asset-backup-{suffix}.sqlite3.tmp"));
        let archive_temp = adjacent_temp(target, &suffix)?;
        let result = (|| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&database_temp)
                .map_err(|_| MessageError::Backup)?;
            let source = self.connect().map_err(|_| MessageError::Backup)?;
            let mut snapshot =
                Connection::open(&database_temp).map_err(|_| MessageError::Backup)?;
            copy_database(&source, &mut snapshot, MessageError::Backup)?;
            let assets = validate_database(&snapshot, MessageError::CorruptData)?;
            validate_message_content(
                &snapshot,
                |name, length| {
                    let hash = decode_hex::<32>(name).map_err(|_| MessageError::CorruptData)?;
                    self.read_asset(name, &hash, length)
                },
                MessageError::CorruptData,
            )?;
            drop(snapshot);
            drop(source);

            let (database_hash, database_bytes) = hash_file(&database_temp, MessageError::Backup)?;
            let manifest = BackupManifest {
                schema: BACKUP_SCHEMA,
                database: BackupFile {
                    content_hash: encode_hex(&database_hash),
                    byte_length: database_bytes,
                },
                assets,
            };
            write_backup(self, &archive_temp, &database_temp, &manifest)?;
            let inspected = inspect_backup(&archive_temp).map_err(|_| MessageError::Backup)?;
            if inspected != manifest {
                return Err(MessageError::Backup);
            }
            fs::hard_link(&archive_temp, target).map_err(|_| MessageError::Backup)
        })();
        let _ = fs::remove_file(&database_temp);
        let _ = fs::remove_file(&archive_temp);
        result
    }

    fn restore_backup_locked(&self, source: &Path) -> Result<(), MessageError> {
        let manifest = inspect_backup(source)?;
        let database_temp = self
            .assets
            .join(format!(".atha-asset-restore-{}.sqlite3.tmp", temp_suffix()));
        let result = (|| {
            extract_database(source, &database_temp, &manifest.database)?;
            let staged = Connection::open_with_flags(
                &database_temp,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|_| MessageError::InvalidBackup)?;
            let database_assets = validate_database(&staged, MessageError::InvalidBackup)?;
            if database_assets != manifest.assets {
                return Err(MessageError::InvalidBackup);
            }
            let mut content_archive = open_archive(source)?;
            validate_message_content(
                &staged,
                |name, length| {
                    read_asset_entry(
                        &mut content_archive,
                        &BackupAsset {
                            content_hash: name.to_owned(),
                            byte_length: length,
                        },
                    )
                },
                MessageError::InvalidBackup,
            )?;

            if !manifest.assets.is_empty() {
                let mut connection = self.connect().map_err(|_| MessageError::Restore)?;
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| MessageError::Restore)?;
                let mut archive = open_archive(source)?;
                for asset in &manifest.assets {
                    let bytes = read_asset_entry(&mut archive, asset)?;
                    let hash = decode_hex::<32>(&asset.content_hash)
                        .map_err(|_| MessageError::InvalidBackup)?;
                    self.restore_asset(&transaction, &asset.content_hash, &hash, &bytes)
                        .map_err(|_| MessageError::Restore)?;
                }
                transaction.commit().map_err(|_| MessageError::Restore)?;
            }

            let mut destination =
                Connection::open(&self.database).map_err(|_| MessageError::Restore)?;
            destination
                .busy_timeout(Duration::from_millis(100))
                .map_err(|_| MessageError::Restore)?;
            copy_database(&staged, &mut destination, MessageError::Restore)
        })();
        let _ = fs::remove_file(&database_temp);
        result
    }
}

fn write_backup(
    store: &MessageStore,
    target: &Path,
    database: &Path,
    manifest: &BackupManifest,
) -> Result<(), MessageError> {
    let manifest_bytes = serde_json::to_vec_pretty(manifest).map_err(|_| MessageError::Backup)?;
    validate_manifest(manifest, manifest_bytes.len() as u64, MessageError::Backup)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| MessageError::Backup)?;
    let mut archive = ZipWriter::new(file);
    let compressed = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    let stored = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o600);
    archive
        .start_file(MANIFEST_ENTRY, compressed)
        .map_err(|_| MessageError::Backup)?;
    archive
        .write_all(&manifest_bytes)
        .map_err(|_| MessageError::Backup)?;
    archive
        .start_file(DATABASE_ENTRY, compressed)
        .map_err(|_| MessageError::Backup)?;
    let mut database = File::open(database).map_err(|_| MessageError::Backup)?;
    std::io::copy(&mut database, &mut archive).map_err(|_| MessageError::Backup)?;
    for asset in &manifest.assets {
        let hash = decode_hex::<32>(&asset.content_hash).map_err(|_| MessageError::Backup)?;
        let bytes = store.read_asset(&asset.content_hash, &hash, asset.byte_length)?;
        archive
            .start_file(format!("assets/{}", asset.content_hash), stored)
            .map_err(|_| MessageError::Backup)?;
        archive
            .write_all(&bytes)
            .map_err(|_| MessageError::Backup)?;
    }
    let file = archive.finish().map_err(|_| MessageError::Backup)?;
    file.sync_all().map_err(|_| MessageError::Backup)
}

fn inspect_backup(path: &Path) -> Result<BackupManifest, MessageError> {
    let mut archive = open_archive(path)?;
    if archive.len() < 2
        || archive.len() > MAX_BACKUP_ASSETS + 2
        || archive
            .has_overlapping_files()
            .map_err(|_| MessageError::InvalidBackup)?
    {
        return Err(MessageError::InvalidBackup);
    }
    let mut entries = HashMap::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| MessageError::InvalidBackup)?;
        if entry.is_dir()
            || entries
                .insert(entry.name().to_owned(), entry.size())
                .is_some()
        {
            return Err(MessageError::InvalidBackup);
        }
    }

    let manifest_bytes = {
        let mut entry = archive
            .by_name(MANIFEST_ENTRY)
            .map_err(|_| MessageError::InvalidBackup)?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err(MessageError::InvalidBackup);
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .by_ref()
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| MessageError::InvalidBackup)?;
        bytes
    };
    let manifest: BackupManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| MessageError::InvalidBackup)?;
    validate_manifest(
        &manifest,
        manifest_bytes.len() as u64,
        MessageError::InvalidBackup,
    )?;

    let mut expected = HashSet::with_capacity(manifest.assets.len() + 2);
    expected.insert(MANIFEST_ENTRY.to_owned());
    expected.insert(DATABASE_ENTRY.to_owned());
    for asset in &manifest.assets {
        expected.insert(format!("assets/{}", asset.content_hash));
    }
    if entries.keys().cloned().collect::<HashSet<_>>() != expected
        || manifest_bytes.len() as u64 != entries[MANIFEST_ENTRY]
        || entries.get(DATABASE_ENTRY) != Some(&manifest.database.byte_length)
    {
        return Err(MessageError::InvalidBackup);
    }
    for asset in &manifest.assets {
        if entries.get(&format!("assets/{}", asset.content_hash)) != Some(&asset.byte_length) {
            return Err(MessageError::InvalidBackup);
        }
    }
    verify_entry_hash(
        &mut archive,
        DATABASE_ENTRY,
        manifest.database.byte_length,
        &manifest.database.content_hash,
    )?;
    for asset in &manifest.assets {
        verify_entry_hash(
            &mut archive,
            &format!("assets/{}", asset.content_hash),
            asset.byte_length,
            &asset.content_hash,
        )?;
    }
    Ok(manifest)
}

fn validate_manifest(
    manifest: &BackupManifest,
    manifest_bytes: u64,
    error: MessageError,
) -> Result<(), MessageError> {
    if manifest.schema != BACKUP_SCHEMA
        || manifest_bytes > MAX_MANIFEST_BYTES
        || manifest.database.byte_length == 0
        || !is_content_hash(&manifest.database.content_hash)
        || manifest.assets.len() > MAX_BACKUP_ASSETS
    {
        return Err(error);
    }
    let mut names = HashSet::with_capacity(manifest.assets.len());
    let mut total = manifest_bytes
        .checked_add(manifest.database.byte_length)
        .ok_or(error)?;
    for asset in &manifest.assets {
        if asset.byte_length == 0
            || asset.byte_length > MAX_ASSET_BYTES
            || !is_content_hash(&asset.content_hash)
            || !names.insert(asset.content_hash.as_str())
        {
            return Err(error);
        }
        total = total.checked_add(asset.byte_length).ok_or(error)?;
    }
    if total > MAX_BACKUP_BYTES {
        Err(error)
    } else {
        Ok(())
    }
}

fn validate_database(
    connection: &Connection,
    error: MessageError,
) -> Result<Vec<BackupAsset>, MessageError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| error)?;
    let integrity: String = connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .map_err(|_| error)?;
    let fts5: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'message_search' AND sql LIKE '%fts5%')",
            [],
            |row| row.get(0),
        )
        .map_err(|_| error)?;
    let foreign_key_violation = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| error)?;
    let expected = Connection::open_in_memory().map_err(|_| error)?;
    expected.execute_batch(SCHEMA_V1).map_err(|_| error)?;
    expected.execute_batch(SCHEMA_V2).map_err(|_| error)?;
    if version != DATABASE_VERSION
        || integrity != "ok"
        || !fts5
        || foreign_key_violation
        || schema_signature(connection, error)? != schema_signature(&expected, error)?
    {
        return Err(error);
    }

    let mut statement = connection
        .prepare(
            "SELECT asset_name, content_hash, byte_length
             FROM snapshot_resource
             GROUP BY asset_name, content_hash, byte_length
             ORDER BY asset_name, content_hash, byte_length",
        )
        .map_err(|_| error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|_| error)?;
    let mut assets = Vec::new();
    let mut names = HashSet::new();
    for row in rows {
        let (name, hash, byte_length) = row.map_err(|_| error)?;
        let byte_length = u64::try_from(byte_length).map_err(|_| error)?;
        if hash.len() != 32
            || encode_hex(&hash) != name
            || byte_length == 0
            || byte_length > MAX_ASSET_BYTES
            || !names.insert(name.clone())
        {
            return Err(error);
        }
        assets.push(BackupAsset {
            content_hash: name,
            byte_length,
        });
    }
    Ok(assets)
}

fn schema_signature(
    connection: &Connection,
    error: MessageError,
) -> Result<Vec<(String, String, String, String)>, MessageError> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, tbl_name, COALESCE(sql, '')
             FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
             ORDER BY type, name, tbl_name, sql",
        )
        .map_err(|_| error)?;
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|_| error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error)
}

fn validate_message_content(
    connection: &Connection,
    mut load_asset: impl FnMut(&str, u64) -> Result<Vec<u8>, MessageError>,
    error: MessageError,
) -> Result<(), MessageError> {
    let oversized: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM work WHERE length(CAST(title AS BLOB)) > 2048
                UNION ALL SELECT 1 FROM edition
                    WHERE length(CAST(title AS BLOB)) > 2048
                       OR length(CAST(authors_json AS BLOB)) > 65536
                UNION ALL SELECT 1 FROM message_revision
                    WHERE length(CAST(content_json AS BLOB)) > 131072
                       OR length(CAST(plain_text AS BLOB)) > 32768
                UNION ALL SELECT 1 FROM source_anchor
                    WHERE length(CAST(original_locator_json AS BLOB)) > 2048
                       OR length(CAST(current_locator_json AS BLOB)) > 2048
                       OR length(CAST(section_id AS BLOB)) > 256
                       OR length(CAST(selected_text AS BLOB)) > 16384
                       OR length(CAST(prefix_text AS BLOB)) > 128
                       OR length(CAST(suffix_text AS BLOB)) > 128
                UNION ALL SELECT 1 FROM source_snapshot
                    WHERE length(CAST(fragment_html AS BLOB)) > 262144
                       OR length(CAST(reader_css AS BLOB)) > 1048576
                       OR length(CAST(book_css AS BLOB)) > 1048576
                       OR length(CAST(user_css AS BLOB)) > 32768
                       OR length(CAST(presentation_json AS BLOB)) > 4096
                UNION ALL SELECT 1 FROM snapshot_resource
                    WHERE length(CAST(source_path AS BLOB)) > 4096
                       OR length(CAST(media_type AS BLOB)) > 512
                       OR length(CAST(asset_name AS BLOB)) > 64
            )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| error)?;
    if oversized {
        return Err(error);
    }

    let invalid_relationship: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM conversation c
                LEFT JOIN message root ON root.id = c.root_message_id
                WHERE c.root_message_id IS NULL OR root.id IS NULL
                   OR root.conversation_id <> c.id OR root.ordinal <> 0
                   OR root.reply_to_message_id IS NOT NULL
                   OR root.current_source_anchor_id IS NULL
                   OR c.created_at_ms < 0 OR c.updated_at_ms < c.created_at_ms
                UNION ALL
                SELECT 1 FROM message m
                JOIN conversation c ON c.id = m.conversation_id
                LEFT JOIN message parent ON parent.id = m.reply_to_message_id
                WHERE m.current_revision_id IS NULL
                   OR m.created_at_ms < 0 OR m.updated_at_ms < m.created_at_ms
                   OR (m.deleted_at_ms IS NOT NULL
                       AND (m.deleted_at_ms < m.created_at_ms OR m.deleted_at_ms > m.updated_at_ms))
                   OR (m.id = c.root_message_id AND
                       (m.ordinal <> 0 OR m.reply_to_message_id IS NOT NULL
                        OR m.current_source_anchor_id IS NULL))
                   OR (m.id <> c.root_message_id AND
                       (m.current_source_anchor_id IS NOT NULL OR parent.id IS NULL
                        OR parent.conversation_id <> m.conversation_id
                        OR parent.ordinal >= m.ordinal))
                UNION ALL
                SELECT 1 FROM source_anchor a
                JOIN message m ON m.id = a.message_id
                JOIN conversation c ON c.id = m.conversation_id
                WHERE c.root_message_id <> m.id OR a.created_at_ms < 0
                UNION ALL
                SELECT 1 FROM source_snapshot s
                LEFT JOIN source_anchor a ON a.snapshot_id = s.id
                WHERE a.id IS NULL OR s.created_at_ms < 0
                UNION ALL
                SELECT 1 FROM snapshot_resource r
                LEFT JOIN source_anchor a ON a.snapshot_id = r.snapshot_id
                WHERE a.id IS NULL
                UNION ALL
                SELECT 1 FROM message_reference r
                JOIN message source ON source.id = r.source_message_id
                JOIN conversation source_c ON source_c.id = source.conversation_id
                JOIN message target ON target.id = r.target_message_id
                JOIN conversation target_c ON target_c.id = target.conversation_id
                WHERE source_c.edition_id <> target_c.edition_id
                   OR source.reply_to_message_id = target.id OR r.created_at_ms < 0
                UNION ALL
                SELECT 1 FROM message_revision WHERE created_at_ms < 0
                UNION ALL
                SELECT 1 FROM snapshot_resource GROUP BY snapshot_id
                    HAVING count(*) > 64 OR sum(byte_length) > 33554432
            )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| error)?;
    if invalid_relationship {
        return Err(error);
    }

    let mut works = connection
        .prepare("SELECT title, created_at_ms, updated_at_ms FROM work ORDER BY id")
        .map_err(|_| error)?;
    for row in works
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|_| error)?
    {
        let (title, created, updated) = row.map_err(|_| error)?;
        if title.trim().is_empty()
            || title.chars().count() > 512
            || created < 0
            || updated < created
        {
            return Err(error);
        }
    }

    let mut editions = connection
        .prepare(
            "SELECT lower(hex(id)), title, authors_json, imported_at_ms FROM edition ORDER BY id",
        )
        .map_err(|_| error)?;
    for row in editions
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|_| error)?
    {
        let (content_version, title, authors, imported) = row.map_err(|_| error)?;
        let edition = EditionInput {
            content_version,
            title,
            authors: serde_json::from_str(&authors).map_err(|_| error)?,
        };
        if imported < 0 || validate_edition(&edition).is_err() {
            return Err(error);
        }
    }

    let mut revisions = connection
        .prepare(
            "SELECT schema_version, kind, content_json, plain_text, created_at_ms
             FROM message_revision ORDER BY message_id, created_at_ms, id",
        )
        .map_err(|_| error)?;
    for row in revisions
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|_| error)?
    {
        let (schema, kind, content_json, plain_text, created) = row.map_err(|_| error)?;
        validate_revision_content(schema, &kind, &content_json, &plain_text, created, error)?;
    }

    validate_source_content(connection, &mut load_asset, error)?;
    validate_search_projection(connection, error)
}

fn validate_revision_content(
    schema: i64,
    kind: &str,
    content_json: &str,
    plain_text: &str,
    created_at: i64,
    error: MessageError,
) -> Result<(), MessageError> {
    let content = serde_json::from_str::<Value>(content_json).map_err(|_| error)?;
    if created_at >= 0 && validate_stored_revision(schema, kind, &content, plain_text).is_ok() {
        Ok(())
    } else {
        Err(error)
    }
}

fn validate_source_content(
    connection: &Connection,
    load_asset: &mut impl FnMut(&str, u64) -> Result<Vec<u8>, MessageError>,
    error: MessageError,
) -> Result<(), MessageError> {
    let mut sources = connection
        .prepare(
            "SELECT a.snapshot_id, lower(hex(c.edition_id)),
                    a.original_locator_json, a.current_locator_json, a.section_id,
                    a.selected_text, a.prefix_text, a.suffix_text, lower(hex(a.content_hash)),
                    s.fragment_html, s.reader_css, s.book_css, s.user_css, s.presentation_json
             FROM source_anchor a
             JOIN message m ON m.id = a.message_id
             JOIN conversation c ON c.id = m.conversation_id
             JOIN source_snapshot s ON s.id = a.snapshot_id
             ORDER BY a.id",
        )
        .map_err(|_| error)?;
    let rows = sources
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
            ))
        })
        .map_err(|_| error)?;
    for row in rows {
        let (
            snapshot_id,
            edition_id,
            original_locator,
            current_locator,
            section,
            selected_text,
            prefix_text,
            suffix_text,
            content_hash,
            fragment_html,
            reader_css,
            book_css,
            user_css,
            presentation_json,
        ) = row.map_err(|_| error)?;
        if !locator_matches_edition(&original_locator, &edition_id)
            || !locator_matches_edition(&current_locator, &edition_id)
            || validate_range_locator(
                &original_locator,
                &section,
                selected_text.encode_utf16().count(),
            )
            .is_err()
        {
            return Err(error);
        }

        let mut resources = connection
            .prepare(
                "SELECT source_path, media_type, byte_length, asset_name
                 FROM snapshot_resource WHERE snapshot_id = ?1 ORDER BY source_path",
            )
            .map_err(|_| error)?;
        let resource_rows = resources
            .query_map(params![snapshot_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|_| error)?;
        let mut snapshot_resources = Vec::new();
        for resource in resource_rows {
            let (path, media_type, byte_length, asset_name) = resource.map_err(|_| error)?;
            let byte_length = u64::try_from(byte_length).map_err(|_| error)?;
            snapshot_resources.push(SnapshotResourceInput {
                path,
                media_type,
                bytes: load_asset(&asset_name, byte_length).map_err(|_| error)?,
            });
        }
        let anchor = SourceAnchorInput {
            canonical_locator: current_locator,
            section,
            selected_text,
            prefix_text,
            suffix_text,
            content_hash,
        };
        let snapshot = SourceSnapshotInput {
            fragment_html,
            reader_css,
            book_css,
            user_css,
            presentation_json,
            resources: snapshot_resources,
        };
        validate_source(&anchor, &snapshot).map_err(|_| error)?;
    }
    Ok(())
}

fn locator_matches_edition(locator: &str, edition_id: &str) -> bool {
    serde_json::from_str::<Value>(locator)
        .ok()
        .and_then(|value| {
            value
                .get("contentVersion")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .as_deref()
        == Some(edition_id)
}

fn validate_search_projection(
    connection: &Connection,
    error: MessageError,
) -> Result<(), MessageError> {
    let invalid: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM message_search search
                LEFT JOIN message m ON lower(hex(m.id)) = search.message_id
                LEFT JOIN conversation c ON c.id = m.conversation_id
                LEFT JOIN message_revision r
                    ON r.id = m.current_revision_id AND r.message_id = m.id
                LEFT JOIN source_anchor a
                    ON a.id = m.current_source_anchor_id AND a.message_id = m.id
                LEFT JOIN message root ON root.id = c.root_message_id
                LEFT JOIN source_anchor root_a ON root_a.id = root.current_source_anchor_id
                WHERE m.id IS NULL OR m.deleted_at_ms IS NOT NULL
                   OR search.conversation_id <> lower(hex(m.conversation_id))
                   OR search.edition_id <> lower(hex(c.edition_id))
                   OR search.section_id <> COALESCE(a.section_id, root_a.section_id, '')
                   OR search.selected_text <> COALESCE(a.selected_text, root_a.selected_text, '')
                   OR search.plain_text <> r.plain_text
                UNION ALL
                SELECT 1 FROM message m
                LEFT JOIN message_search search ON search.message_id = lower(hex(m.id))
                WHERE m.deleted_at_ms IS NULL
                GROUP BY m.id HAVING count(search.rowid) <> 1
            )",
            [],
            |row| row.get(0),
        )
        .map_err(|_| error)?;
    if invalid { Err(error) } else { Ok(()) }
}

fn copy_database(
    source: &Connection,
    destination: &mut Connection,
    error: MessageError,
) -> Result<(), MessageError> {
    let backup = Backup::new(source, destination).map_err(|_| error)?;
    let mut busy_retries = 0;
    loop {
        match backup.step(128).map_err(|_| error)? {
            StepResult::Done => return Ok(()),
            StepResult::More => {
                busy_retries = 0;
                thread::sleep(Duration::from_millis(1));
            }
            StepResult::Busy | StepResult::Locked if busy_retries < 3 => {
                busy_retries += 1;
                thread::sleep(Duration::from_millis(100));
            }
            StepResult::Busy | StepResult::Locked => return Err(error),
            _ => return Err(error),
        }
    }
}

fn extract_database(
    archive_path: &Path,
    target: &Path,
    expected: &BackupFile,
) -> Result<(), MessageError> {
    let mut archive = open_archive(archive_path)?;
    let mut entry = archive
        .by_name(DATABASE_ENTRY)
        .map_err(|_| MessageError::InvalidBackup)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| MessageError::Restore)?;
    let copied = std::io::copy(
        &mut entry.by_ref().take(expected.byte_length + 1),
        &mut file,
    )
    .map_err(|_| MessageError::InvalidBackup)?;
    if copied != expected.byte_length {
        return Err(MessageError::InvalidBackup);
    }
    file.sync_all().map_err(|_| MessageError::Restore)?;
    drop(file);
    let (hash, length) = hash_file(target, MessageError::InvalidBackup)?;
    if length != expected.byte_length || encode_hex(&hash) != expected.content_hash {
        return Err(MessageError::InvalidBackup);
    }
    Ok(())
}

fn read_asset_entry(
    archive: &mut ZipArchive<File>,
    asset: &BackupAsset,
) -> Result<Vec<u8>, MessageError> {
    let mut entry = archive
        .by_name(&format!("assets/{}", asset.content_hash))
        .map_err(|_| MessageError::InvalidBackup)?;
    let mut bytes = Vec::with_capacity(asset.byte_length as usize);
    entry
        .by_ref()
        .take(asset.byte_length + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| MessageError::InvalidBackup)?;
    if bytes.len() as u64 != asset.byte_length
        || encode_hex(&Sha256::digest(&bytes)) != asset.content_hash
    {
        return Err(MessageError::InvalidBackup);
    }
    Ok(bytes)
}

fn verify_entry_hash(
    archive: &mut ZipArchive<File>,
    name: &str,
    expected_length: u64,
    expected_hash: &str,
) -> Result<(), MessageError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| MessageError::InvalidBackup)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = entry
            .read(&mut buffer)
            .map_err(|_| MessageError::InvalidBackup)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(MessageError::InvalidBackup)?;
        if total > expected_length {
            return Err(MessageError::InvalidBackup);
        }
        digest.update(&buffer[..read]);
    }
    if total != expected_length || encode_hex(&digest.finalize()) != expected_hash {
        return Err(MessageError::InvalidBackup);
    }
    Ok(())
}

fn hash_file(path: &Path, error: MessageError) -> Result<([u8; 32], u64), MessageError> {
    let mut file = File::open(path).map_err(|_| error)?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| error)?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or(error)?;
        digest.update(&buffer[..read]);
    }
    Ok((digest.finalize().into(), total))
}

fn open_archive(path: &Path) -> Result<ZipArchive<File>, MessageError> {
    let file = File::open(path).map_err(|_| MessageError::InvalidBackup)?;
    ZipArchive::new(file).map_err(|_| MessageError::InvalidBackup)
}

fn is_content_hash(value: &str) -> bool {
    decode_hex::<32>(value).is_ok_and(|hash| encode_hex(&hash) == value)
}

fn adjacent_temp(target: &Path, suffix: &str) -> Result<PathBuf, MessageError> {
    let name = target.file_name().ok_or(MessageError::Backup)?;
    let mut temporary = OsString::from(".");
    temporary.push(name);
    temporary.push(format!(".{suffix}.tmp"));
    Ok(target.with_file_name(temporary))
}

fn temp_suffix() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
