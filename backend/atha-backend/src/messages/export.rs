use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::{
    model::{
        EditionInput, ExportInspection, MessageError, RichTextInput, SnapshotResourceInput,
        SourceAnchorInput, SourceSnapshotInput, rich_message_content, validate_edition,
        validate_range_locator, validate_source,
    },
    store::MessageStore,
    util::{decode_hex, encode_hex},
};

const EXPORT_SCHEMA: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXPORT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXPORT_ENTRIES: usize = 1_026;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportManifest {
    schema: u32,
    edition: ExportEdition,
    scope: ExportScope,
    conversations: Vec<ExportConversation>,
    messages: Vec<ExportMessage>,
    revisions: Vec<ExportRevision>,
    sources: Vec<ExportSource>,
    snapshots: Vec<ExportSnapshot>,
    snapshot_resources: Vec<ExportSnapshotResource>,
    relationships: Vec<ExportRelationship>,
    assets: Vec<ExportAsset>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportEdition {
    id: String,
    title: String,
    authors: Vec<String>,
    work: ExportWork,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportWork {
    id: String,
    title: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportScope {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportConversation {
    id: String,
    edition_id: String,
    root_message_id: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportMessage {
    id: String,
    conversation_id: String,
    ordinal: i64,
    author_type: String,
    reply_to_message_id: Option<String>,
    current_revision_id: String,
    current_source_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportRevision {
    id: String,
    message_id: String,
    schema: u32,
    kind: String,
    content: Value,
    plain_text: String,
    created_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportSource {
    id: String,
    message_id: String,
    snapshot_id: String,
    original_locator: Value,
    current_locator: Value,
    section: String,
    selected_text: String,
    prefix_text: String,
    suffix_text: String,
    content_hash: String,
    created_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportSnapshot {
    id: String,
    fragment_html: String,
    reader_css: String,
    book_css: String,
    user_css: String,
    presentation: Value,
    created_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportSnapshotResource {
    snapshot_id: String,
    source_path: String,
    media_type: String,
    content_hash: String,
    byte_length: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportRelationship {
    source_message_id: String,
    target_message_id: String,
    kind: String,
    created_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
            let reply_parents = connection
                .execute(
                    "INSERT OR IGNORE INTO export_message
                     SELECT m.reply_to_message_id FROM message m
                     JOIN export_message included ON included.id = m.id
                     WHERE m.reply_to_message_id IS NOT NULL",
                    [],
                )
                .map_err(|_| MessageError::Database)?;
            if roots + references + reply_parents == 0 {
                break;
            }
        }

        let manifest = ExportManifest {
            schema: EXPORT_SCHEMA,
            edition: serde_json::from_str(&edition_json).map_err(|_| MessageError::Database)?,
            scope: ExportScope {
                kind: if conversation.is_some() {
                    "conversation".into()
                } else {
                    "edition".into()
                },
                id: conversation.map_or_else(|| encode_hex(edition), encode_hex),
            },
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
            let manifest_bytes =
                serde_json::to_vec_pretty(manifest).map_err(|_| MessageError::Export)?;
            validate_export_bounds(manifest_bytes.len() as u64, &manifest.assets)?;
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
                .write_all(&manifest_bytes)
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
            drop(file);
            inspect_export(target).map_err(|_| MessageError::CorruptData)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(target);
        }
        result
    }
}

fn validate_export_bounds(manifest_bytes: u64, assets: &[ExportAsset]) -> Result<(), MessageError> {
    let total = assets.iter().try_fold(manifest_bytes, |total, asset| {
        total.checked_add(asset.byte_length)
    });
    if manifest_bytes > MAX_MANIFEST_BYTES
        || assets.len() + 1 > MAX_EXPORT_ENTRIES
        || total.is_none_or(|total| total > MAX_EXPORT_BYTES)
    {
        Err(MessageError::Export)
    } else {
        Ok(())
    }
}

fn json_rows<T: DeserializeOwned>(
    connection: &Connection,
    sql: &str,
) -> Result<Vec<T>, MessageError> {
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
        || archive.len() > MAX_EXPORT_ENTRIES
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
    let mut asset_bytes = HashMap::with_capacity(manifest.assets.len());
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
        asset_bytes.insert(asset.content_hash.clone(), bytes);
    }
    validate_snapshot_payloads(&manifest, &asset_bytes)?;
    Ok(ExportInspection {
        edition_id: manifest.edition.id.clone(),
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
    let invalid = || MessageError::InvalidExport;
    if manifest.schema != EXPORT_SCHEMA
        || validate_edition(&EditionInput {
            content_version: manifest.edition.id.clone(),
            title: manifest.edition.title.clone(),
            authors: manifest.edition.authors.clone(),
        })
        .is_err()
        || decode_hex::<16>(&manifest.edition.work.id).is_err()
        || manifest.edition.work.title.trim().is_empty()
        || manifest.edition.work.title.chars().count() > 512
    {
        return Err(invalid());
    }

    let mut conversations = HashMap::new();
    for row in &manifest.conversations {
        insert_unique_id::<16, _>(&mut conversations, &row.id, row)?;
        if row.edition_id != manifest.edition.id
            || row.created_at < 0
            || row.updated_at < row.created_at
            || decode_hex::<16>(&row.root_message_id).is_err()
        {
            return Err(invalid());
        }
    }
    let mut messages = HashMap::new();
    let mut ordinals = HashSet::new();
    for row in &manifest.messages {
        insert_unique_id::<16, _>(&mut messages, &row.id, row)?;
        if decode_hex::<16>(&row.conversation_id).is_err()
            || decode_hex::<16>(&row.current_revision_id).is_err()
            || row
                .reply_to_message_id
                .as_deref()
                .is_some_and(|id| decode_hex::<16>(id).is_err())
            || row
                .current_source_id
                .as_deref()
                .is_some_and(|id| decode_hex::<16>(id).is_err())
            || row.ordinal < 0
            || !matches!(row.author_type.as_str(), "user" | "assistant" | "system")
            || row.created_at < 0
            || row.updated_at < row.created_at
            || row
                .deleted_at
                .is_some_and(|deleted| deleted < row.created_at || deleted > row.updated_at)
            || !ordinals.insert((row.conversation_id.as_str(), row.ordinal))
        {
            return Err(invalid());
        }
    }
    let mut revisions = HashMap::new();
    for row in &manifest.revisions {
        insert_unique_id::<16, _>(&mut revisions, &row.id, row)?;
        if decode_hex::<16>(&row.message_id).is_err()
            || row.created_at < 0
            || validate_export_revision(row).is_err()
        {
            return Err(invalid());
        }
    }
    let mut snapshots = HashMap::new();
    for row in &manifest.snapshots {
        insert_unique_id::<16, _>(&mut snapshots, &row.id, row)?;
        if row.created_at < 0 {
            return Err(invalid());
        }
    }
    let mut sources = HashMap::new();
    for row in &manifest.sources {
        insert_unique_id::<16, _>(&mut sources, &row.id, row)?;
        if decode_hex::<16>(&row.message_id).is_err()
            || decode_hex::<16>(&row.snapshot_id).is_err()
            || decode_hex::<32>(&row.content_hash).is_err()
            || row.created_at < 0
        {
            return Err(invalid());
        }
    }

    if manifest.scope.kind == "edition" {
        if manifest.scope.id != manifest.edition.id {
            return Err(invalid());
        }
    } else if manifest.scope.kind == "conversation" {
        if !conversations.contains_key(manifest.scope.id.as_str()) {
            return Err(invalid());
        }
    } else {
        return Err(invalid());
    }
    for row in &manifest.conversations {
        let root = messages
            .get(row.root_message_id.as_str())
            .ok_or_else(invalid)?;
        if root.conversation_id != row.id || root.ordinal != 0 || root.current_source_id.is_none() {
            return Err(invalid());
        }
    }
    for row in &manifest.messages {
        let conversation = conversations
            .get(row.conversation_id.as_str())
            .ok_or_else(invalid)?;
        let is_root = conversation.root_message_id == row.id;
        let parent = row
            .reply_to_message_id
            .as_deref()
            .and_then(|parent| messages.get(parent));
        if revisions
            .get(row.current_revision_id.as_str())
            .is_none_or(|revision| revision.message_id != row.id)
            || row.current_source_id.as_deref().is_some_and(|source| {
                sources
                    .get(source)
                    .is_none_or(|source| source.message_id != row.id)
            })
            || is_root
                && (row.ordinal != 0
                    || row.reply_to_message_id.is_some()
                    || row.current_source_id.is_none())
            || !is_root
                && (row.current_source_id.is_some()
                    || parent.is_none_or(|parent| {
                        parent.conversation_id != row.conversation_id
                            || parent.ordinal >= row.ordinal
                    }))
        {
            return Err(invalid());
        }
    }
    for source in sources.values() {
        let message = messages
            .get(source.message_id.as_str())
            .ok_or_else(invalid)?;
        let conversation = conversations
            .get(message.conversation_id.as_str())
            .ok_or_else(invalid)?;
        if conversation.root_message_id != message.id {
            return Err(invalid());
        }
    }
    if revisions
        .values()
        .any(|row| !messages.contains_key(row.message_id.as_str()))
        || sources.values().any(|row| {
            !messages.contains_key(row.message_id.as_str())
                || !snapshots.contains_key(row.snapshot_id.as_str())
        })
        || snapshots.keys().copied().collect::<HashSet<_>>()
            != sources
                .values()
                .map(|row| row.snapshot_id.as_str())
                .collect::<HashSet<_>>()
    {
        return Err(invalid());
    }

    let mut relationships = HashSet::new();
    for row in &manifest.relationships {
        let source = messages
            .get(row.source_message_id.as_str())
            .ok_or_else(invalid)?;
        if row.kind != "quote"
            || row.created_at < 0
            || row.source_message_id == row.target_message_id
            || !messages.contains_key(row.target_message_id.as_str())
            || source.reply_to_message_id.as_deref() == Some(row.target_message_id.as_str())
            || !relationships.insert((
                row.source_message_id.as_str(),
                row.target_message_id.as_str(),
            ))
        {
            return Err(invalid());
        }
    }

    let mut assets = HashMap::new();
    for asset in &manifest.assets {
        if decode_hex::<32>(&asset.content_hash).is_err()
            || asset.byte_length == 0
            || asset.byte_length > 16 * 1024 * 1024
            || assets
                .insert(asset.content_hash.as_str(), asset.byte_length)
                .is_some()
        {
            return Err(invalid());
        }
    }
    let mut resource_paths = HashSet::new();
    let mut referenced_assets = HashSet::new();
    for row in &manifest.snapshot_resources {
        if !snapshots.contains_key(row.snapshot_id.as_str())
            || decode_hex::<32>(&row.content_hash).is_err()
            || assets.get(row.content_hash.as_str()) != Some(&row.byte_length)
            || !resource_paths.insert((row.snapshot_id.as_str(), row.source_path.as_str()))
        {
            return Err(invalid());
        }
        referenced_assets.insert(row.content_hash.as_str());
    }
    if assets.keys().copied().collect::<HashSet<_>>() != referenced_assets {
        return Err(invalid());
    }
    Ok(())
}

fn insert_unique_id<'a, const N: usize, T>(
    rows: &mut HashMap<&'a str, &'a T>,
    id: &'a str,
    row: &'a T,
) -> Result<(), MessageError> {
    if decode_hex::<N>(id).is_err() || rows.insert(id, row).is_some() {
        Err(MessageError::InvalidExport)
    } else {
        Ok(())
    }
}

fn validate_export_revision(row: &ExportRevision) -> Result<(), MessageError> {
    if row.schema != 1 {
        return Err(MessageError::InvalidExport);
    }
    let valid = match (row.kind.as_str(), row.content.get("richText")) {
        ("source-only", None) => {
            row.plain_text.is_empty()
                && row.content
                    == serde_json::json!({ "schema": 1, "kind": "source-only", "text": "" })
        }
        ("text", Some(value)) => serde_json::from_value::<RichTextInput>(value.clone())
            .map_err(|_| MessageError::InvalidExport)
            .and_then(|rich| rich_message_content(&rich))
            .ok()
            .is_some_and(|content| {
                content.plain_text == row.plain_text
                    && serde_json::from_str::<Value>(&content.content_json).ok()
                        == Some(row.content.clone())
            }),
        ("text", None) => {
            !row.plain_text.trim().is_empty()
                && row.plain_text.chars().count() <= 8_000
                && row.content
                    == serde_json::json!({
                        "schema": 1,
                        "kind": "text",
                        "text": row.plain_text,
                    })
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(MessageError::InvalidExport)
    }
}

fn validate_snapshot_payloads(
    manifest: &ExportManifest,
    assets: &HashMap<String, Vec<u8>>,
) -> Result<(), MessageError> {
    let snapshots = manifest
        .snapshots
        .iter()
        .map(|snapshot| (snapshot.id.as_str(), snapshot))
        .collect::<HashMap<_, _>>();
    for source in &manifest.sources {
        let snapshot = snapshots
            .get(source.snapshot_id.as_str())
            .ok_or(MessageError::InvalidExport)?;
        let original_locator = serde_json::to_string(&source.original_locator)
            .map_err(|_| MessageError::InvalidExport)?;
        let current_locator = serde_json::to_string(&source.current_locator)
            .map_err(|_| MessageError::InvalidExport)?;
        if !locator_matches_edition(&source.original_locator, &manifest.edition.id)
            || !locator_matches_edition(&source.current_locator, &manifest.edition.id)
            || validate_range_locator(
                &original_locator,
                &source.section,
                source.selected_text.encode_utf16().count(),
            )
            .is_err()
        {
            return Err(MessageError::InvalidExport);
        }
        let resources = manifest
            .snapshot_resources
            .iter()
            .filter(|resource| resource.snapshot_id == source.snapshot_id)
            .map(|resource| {
                Ok(SnapshotResourceInput {
                    path: resource.source_path.clone(),
                    media_type: resource.media_type.clone(),
                    bytes: assets
                        .get(&resource.content_hash)
                        .ok_or(MessageError::InvalidExport)?
                        .clone(),
                })
            })
            .collect::<Result<Vec<_>, MessageError>>()?;
        let anchor = SourceAnchorInput {
            canonical_locator: current_locator,
            section: source.section.clone(),
            selected_text: source.selected_text.clone(),
            prefix_text: source.prefix_text.clone(),
            suffix_text: source.suffix_text.clone(),
            content_hash: source.content_hash.clone(),
        };
        let snapshot = SourceSnapshotInput {
            fragment_html: snapshot.fragment_html.clone(),
            reader_css: snapshot.reader_css.clone(),
            book_css: snapshot.book_css.clone(),
            user_css: snapshot.user_css.clone(),
            presentation_json: serde_json::to_string(&snapshot.presentation)
                .map_err(|_| MessageError::InvalidExport)?,
            resources,
        };
        validate_source(&anchor, &snapshot).map_err(|_| MessageError::InvalidExport)?;
    }
    Ok(())
}

fn locator_matches_edition(locator: &Value, edition_id: &str) -> bool {
    locator.get("contentVersion").and_then(Value::as_str) == Some(edition_id)
}

#[cfg(test)]
mod tests {
    use super::{ExportAsset, MAX_EXPORT_ENTRIES, MessageError, validate_export_bounds};

    #[test]
    fn writer_rejects_archives_its_inspector_cannot_accept() {
        let assets = (0..MAX_EXPORT_ENTRIES)
            .map(|index| ExportAsset {
                content_hash: format!("{index:064x}"),
                byte_length: 1,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            validate_export_bounds(1, &assets),
            Err(MessageError::Export)
        );
    }
}
