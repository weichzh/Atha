use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::{
    model::{ExportInspection, MessageError},
    store::MessageStore,
    util::{decode_hex, encode_hex},
};

const EXPORT_SCHEMA: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXPORT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportManifest {
    schema: u32,
    edition: Value,
    scope: Value,
    conversations: Vec<Value>,
    messages: Vec<Value>,
    revisions: Vec<Value>,
    sources: Vec<Value>,
    snapshots: Vec<Value>,
    snapshot_resources: Vec<Value>,
    relationships: Vec<Value>,
    assets: Vec<ExportAsset>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportAsset {
    content_hash: String,
    byte_length: u64,
}

impl MessageStore {
    pub fn export_edition(
        &self,
        edition_id: &str,
        target: impl AsRef<Path>,
    ) -> Result<(), MessageError> {
        let edition = decode_hex::<32>(edition_id)?;
        self.export_archive(&edition, None, target.as_ref())
    }

    pub fn export_conversation(
        &self,
        conversation_id: &str,
        target: impl AsRef<Path>,
    ) -> Result<(), MessageError> {
        let conversation = decode_hex::<16>(conversation_id)?;
        let connection = self.connect()?;
        let edition = connection
            .query_row(
                "SELECT edition_id FROM conversation WHERE id = ?1",
                params![conversation],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownConversation,
                _ => MessageError::Database,
            })?;
        drop(connection);
        self.export_archive(&edition, Some(&conversation), target.as_ref())
    }

    pub fn inspect_export(path: impl AsRef<Path>) -> Result<ExportInspection, MessageError> {
        inspect_export(path.as_ref())
    }

    fn export_archive(
        &self,
        edition: &[u8],
        conversation: Option<&[u8]>,
        target: &Path,
    ) -> Result<(), MessageError> {
        let connection = self.connect()?;
        let edition_json: String = connection
            .query_row(
                "SELECT json_object(
                    'id', lower(hex(e.id)), 'title', e.title, 'authors', json(e.authors_json),
                    'work', json_object('id', lower(hex(w.id)), 'title', w.title)
                 ) FROM edition e JOIN work w ON w.id = e.work_id WHERE e.id = ?1",
                params![edition],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownEdition,
                _ => MessageError::Database,
            })?;
        connection
            .execute_batch("CREATE TEMP TABLE export_message (id BLOB PRIMARY KEY);")
            .map_err(|_| MessageError::Database)?;
        match conversation {
            Some(conversation) => connection
                .execute(
                    "INSERT INTO export_message SELECT id FROM message WHERE conversation_id = ?1",
                    params![conversation],
                )
                .map_err(|_| MessageError::Database)?,
            None => connection
                .execute(
                    "INSERT INTO export_message
                     SELECT m.id FROM message m JOIN conversation c ON c.id = m.conversation_id
                     WHERE c.edition_id = ?1",
                    params![edition],
                )
                .map_err(|_| MessageError::Database)?,
        };
        loop {
            let roots = connection
                .execute(
                    "INSERT OR IGNORE INTO export_message
                     SELECT c.root_message_id FROM conversation c
                     JOIN message m ON m.conversation_id = c.id
                     JOIN export_message included ON included.id = m.id
                     WHERE c.root_message_id IS NOT NULL",
                    [],
                )
                .map_err(|_| MessageError::Database)?;
            let references = connection
                .execute(
                    "INSERT OR IGNORE INTO export_message
                     SELECT r.target_message_id FROM message_reference r
                     JOIN export_message included ON included.id = r.source_message_id",
                    [],
                )
                .map_err(|_| MessageError::Database)?;
            if roots + references == 0 {
                break;
            }
        }

        let manifest = ExportManifest {
            schema: EXPORT_SCHEMA,
            edition: serde_json::from_str(&edition_json).map_err(|_| MessageError::Database)?,
            scope: serde_json::json!({
                "type": if conversation.is_some() { "conversation" } else { "edition" },
                "id": conversation.map_or_else(|| encode_hex(edition), encode_hex)
            }),
            conversations: json_rows(
                &connection,
                "SELECT DISTINCT json_object(
                    'id', lower(hex(c.id)), 'editionId', lower(hex(c.edition_id)),
                    'rootMessageId', lower(hex(c.root_message_id)),
                    'createdAt', c.created_at_ms, 'updatedAt', c.updated_at_ms)
                 FROM conversation c JOIN message m ON m.conversation_id = c.id
                 JOIN export_message included ON included.id = m.id ORDER BY c.created_at_ms, c.id",
            )?,
            messages: json_rows(
                &connection,
                "SELECT json_object(
                    'id', lower(hex(m.id)), 'conversationId', lower(hex(m.conversation_id)),
                    'ordinal', m.ordinal, 'authorType', m.author_type,
                    'replyToMessageId', CASE WHEN m.reply_to_message_id IS NULL THEN NULL ELSE lower(hex(m.reply_to_message_id)) END,
                    'currentRevisionId', lower(hex(m.current_revision_id)),
                    'currentSourceId', CASE WHEN m.current_source_anchor_id IS NULL THEN NULL ELSE lower(hex(m.current_source_anchor_id)) END,
                    'createdAt', m.created_at_ms, 'updatedAt', m.updated_at_ms, 'deletedAt', m.deleted_at_ms)
                 FROM message m JOIN export_message included ON included.id = m.id
                 ORDER BY m.conversation_id, m.ordinal",
            )?,
            revisions: json_rows(
                &connection,
                "SELECT json_object(
                    'id', lower(hex(r.id)), 'messageId', lower(hex(r.message_id)),
                    'schema', r.schema_version, 'kind', r.kind, 'content', json(r.content_json),
                    'plainText', r.plain_text, 'createdAt', r.created_at_ms)
                 FROM message_revision r JOIN export_message included ON included.id = r.message_id
                 ORDER BY r.message_id, r.created_at_ms, r.id",
            )?,
            sources: json_rows(
                &connection,
                "SELECT json_object(
                    'id', lower(hex(a.id)), 'messageId', lower(hex(a.message_id)),
                    'snapshotId', lower(hex(a.snapshot_id)),
                    'originalLocator', json(a.original_locator_json), 'currentLocator', json(a.current_locator_json),
                    'section', a.section_id, 'selectedText', a.selected_text,
                    'prefixText', a.prefix_text, 'suffixText', a.suffix_text,
                    'contentHash', lower(hex(a.content_hash)), 'createdAt', a.created_at_ms)
                 FROM source_anchor a JOIN export_message included ON included.id = a.message_id
                 ORDER BY a.message_id, a.created_at_ms, a.id",
            )?,
            snapshots: json_rows(
                &connection,
                "SELECT DISTINCT json_object(
                    'id', lower(hex(s.id)), 'fragmentHtml', s.fragment_html,
                    'readerCss', s.reader_css, 'bookCss', s.book_css, 'userCss', s.user_css,
                    'presentation', json(s.presentation_json), 'createdAt', s.created_at_ms)
                 FROM source_snapshot s JOIN source_anchor a ON a.snapshot_id = s.id
                 JOIN export_message included ON included.id = a.message_id ORDER BY s.id",
            )?,
            snapshot_resources: json_rows(
                &connection,
                "SELECT DISTINCT json_object(
                    'snapshotId', lower(hex(r.snapshot_id)), 'sourcePath', r.source_path,
                    'mediaType', r.media_type, 'contentHash', lower(hex(r.content_hash)),
                    'byteLength', r.byte_length)
                 FROM snapshot_resource r JOIN source_anchor a ON a.snapshot_id = r.snapshot_id
                 JOIN export_message included ON included.id = a.message_id
                 ORDER BY r.snapshot_id, r.source_path",
            )?,
            relationships: json_rows(
                &connection,
                "SELECT json_object(
                    'sourceMessageId', lower(hex(r.source_message_id)),
                    'targetMessageId', lower(hex(r.target_message_id)),
                    'kind', r.kind, 'createdAt', r.created_at_ms)
                 FROM message_reference r JOIN export_message included ON included.id = r.source_message_id
                 ORDER BY r.source_message_id, r.target_message_id",
            )?,
            assets: export_assets(&connection)?,
        };
        self.write_export(target, &manifest)
    }

    fn write_export(&self, target: &Path, manifest: &ExportManifest) -> Result<(), MessageError> {
        let result = (|| {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|_| MessageError::Export)?;
            let mut archive = ZipWriter::new(file);
            let compressed = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o600);
            let stored = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .unix_permissions(0o600);
            archive
                .start_file("manifest.json", compressed)
                .map_err(|_| MessageError::Export)?;
            archive
                .write_all(&serde_json::to_vec_pretty(manifest).map_err(|_| MessageError::Export)?)
                .map_err(|_| MessageError::Export)?;
            for asset in &manifest.assets {
                let bytes = fs::read(self.assets.join(&asset.content_hash))
                    .map_err(|_| MessageError::CorruptData)?;
                if bytes.len() as u64 != asset.byte_length
                    || encode_hex(&Sha256::digest(&bytes)) != asset.content_hash
                {
                    return Err(MessageError::CorruptData);
                }
                archive
                    .start_file(format!("assets/{}", asset.content_hash), stored)
                    .map_err(|_| MessageError::Export)?;
                archive
                    .write_all(&bytes)
                    .map_err(|_| MessageError::Export)?;
            }
            let file = archive.finish().map_err(|_| MessageError::Export)?;
            file.sync_all().map_err(|_| MessageError::Export)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(target);
        }
        result
    }
}

fn json_rows(connection: &Connection, sql: &str) -> Result<Vec<Value>, MessageError> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|_| MessageError::Database)?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| MessageError::Database)?
        .map(|row| {
            serde_json::from_str(&row.map_err(|_| MessageError::Database)?)
                .map_err(|_| MessageError::Database)
        })
        .collect()
}

fn export_assets(connection: &Connection) -> Result<Vec<ExportAsset>, MessageError> {
    let mut statement = connection
        .prepare(
            "SELECT lower(hex(r.content_hash)), MAX(r.byte_length)
             FROM snapshot_resource r JOIN source_anchor a ON a.snapshot_id = r.snapshot_id
             JOIN export_message included ON included.id = a.message_id
             GROUP BY r.content_hash ORDER BY r.content_hash",
        )
        .map_err(|_| MessageError::Database)?;
    statement
        .query_map([], |row| {
            let byte_length: i64 = row.get(1)?;
            Ok(ExportAsset {
                content_hash: row.get(0)?,
                byte_length: byte_length as u64,
            })
        })
        .map_err(|_| MessageError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::Database)
}

fn inspect_export(path: &Path) -> Result<ExportInspection, MessageError> {
    let file = File::open(path).map_err(|_| MessageError::InvalidExport)?;
    let mut archive = ZipArchive::new(file).map_err(|_| MessageError::InvalidExport)?;
    if archive.is_empty()
        || archive.len() > 1_026
        || archive
            .has_overlapping_files()
            .map_err(|_| MessageError::InvalidExport)?
    {
        return Err(MessageError::InvalidExport);
    }
    let mut names = HashSet::with_capacity(archive.len());
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|_| MessageError::InvalidExport)?;
        let name = entry.name();
        let safe = name == "manifest.json"
            || name.strip_prefix("assets/").is_some_and(|hash| {
                hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
        total = total
            .checked_add(entry.size())
            .ok_or(MessageError::InvalidExport)?;
        if !safe
            || entry.encrypted()
            || entry.is_symlink()
            || !entry.is_file()
            || !names.insert(name.to_owned())
            || total > MAX_EXPORT_BYTES
        {
            return Err(MessageError::InvalidExport);
        }
    }
    let manifest = {
        let entry = archive
            .by_name("manifest.json")
            .map_err(|_| MessageError::InvalidExport)?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err(MessageError::InvalidExport);
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .take(MAX_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| MessageError::InvalidExport)?;
        serde_json::from_slice::<ExportManifest>(&bytes).map_err(|_| MessageError::InvalidExport)?
    };
    validate_manifest(&manifest)?;
    let expected_names = std::iter::once("manifest.json".to_owned())
        .chain(
            manifest
                .assets
                .iter()
                .map(|asset| format!("assets/{}", asset.content_hash)),
        )
        .collect::<HashSet<_>>();
    if names != expected_names {
        return Err(MessageError::InvalidExport);
    }
    for asset in &manifest.assets {
        let entry = archive
            .by_name(&format!("assets/{}", asset.content_hash))
            .map_err(|_| MessageError::InvalidExport)?;
        if entry.size() != asset.byte_length {
            return Err(MessageError::InvalidExport);
        }
        let mut bytes = Vec::with_capacity(asset.byte_length as usize);
        entry
            .take(asset.byte_length + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| MessageError::InvalidExport)?;
        if bytes.len() as u64 != asset.byte_length
            || encode_hex(&Sha256::digest(&bytes)) != asset.content_hash
        {
            return Err(MessageError::InvalidExport);
        }
    }
    Ok(ExportInspection {
        edition_id: string_field(&manifest.edition, "id")?.to_owned(),
        conversations: manifest.conversations.len(),
        messages: manifest.messages.len(),
        revisions: manifest.revisions.len(),
        sources: manifest.sources.len(),
        snapshots: manifest.snapshots.len(),
        relationships: manifest.relationships.len(),
        resources: manifest.snapshot_resources.len(),
    })
}

fn validate_manifest(manifest: &ExportManifest) -> Result<(), MessageError> {
    if manifest.schema != EXPORT_SCHEMA {
        return Err(MessageError::InvalidExport);
    }
    let edition_id = string_field(&manifest.edition, "id")?;
    if decode_hex::<32>(edition_id).is_err() {
        return Err(MessageError::InvalidExport);
    }
    let conversations = id_set(&manifest.conversations)?;
    let messages = id_set(&manifest.messages)?;
    let snapshots = id_set(&manifest.snapshots)?;
    let revisions = manifest
        .revisions
        .iter()
        .map(|row| Ok((string_field(row, "id")?, string_field(row, "messageId")?)))
        .collect::<Result<HashMap<_, _>, MessageError>>()?;
    let sources = manifest
        .sources
        .iter()
        .map(|row| {
            Ok((
                string_field(row, "id")?,
                (
                    string_field(row, "messageId")?,
                    string_field(row, "snapshotId")?,
                ),
            ))
        })
        .collect::<Result<HashMap<_, _>, MessageError>>()?;
    for row in &manifest.conversations {
        if string_field(row, "editionId")? != edition_id
            || !messages.contains(string_field(row, "rootMessageId")?)
        {
            return Err(MessageError::InvalidExport);
        }
    }
    for row in &manifest.messages {
        let id = string_field(row, "id")?;
        if !conversations.contains(string_field(row, "conversationId")?)
            || revisions.get(string_field(row, "currentRevisionId")?) != Some(&id)
            || optional_string_field(row, "replyToMessageId")?
                .is_some_and(|parent| !messages.contains(parent))
            || optional_string_field(row, "currentSourceId")?
                .is_some_and(|source| sources.get(source).is_none_or(|value| value.0 != id))
        {
            return Err(MessageError::InvalidExport);
        }
    }
    if revisions
        .values()
        .any(|message| !messages.contains(message))
        || sources
            .values()
            .any(|(message, snapshot)| !messages.contains(message) || !snapshots.contains(snapshot))
    {
        return Err(MessageError::InvalidExport);
    }
    for row in &manifest.relationships {
        if !messages.contains(string_field(row, "sourceMessageId")?)
            || !messages.contains(string_field(row, "targetMessageId")?)
        {
            return Err(MessageError::InvalidExport);
        }
    }
    let assets = manifest
        .assets
        .iter()
        .map(|asset| asset.content_hash.as_str())
        .collect::<HashSet<_>>();
    let mut referenced_assets = HashSet::new();
    for row in &manifest.snapshot_resources {
        if !snapshots.contains(string_field(row, "snapshotId")?) {
            return Err(MessageError::InvalidExport);
        }
        referenced_assets.insert(string_field(row, "contentHash")?);
    }
    if assets != referenced_assets
        || manifest.assets.iter().any(|asset| {
            asset.content_hash.len() != 64
                || !asset
                    .content_hash
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err(MessageError::InvalidExport);
    }
    Ok(())
}

fn id_set(rows: &[Value]) -> Result<HashSet<&str>, MessageError> {
    rows.iter().map(|row| string_field(row, "id")).collect()
}

fn string_field<'a>(value: &'a Value, name: &str) -> Result<&'a str, MessageError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(MessageError::InvalidExport)
}

fn optional_string_field<'a>(
    value: &'a Value,
    name: &str,
) -> Result<Option<&'a str>, MessageError> {
    match value.get(name) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value)),
        _ => Err(MessageError::InvalidExport),
    }
}
