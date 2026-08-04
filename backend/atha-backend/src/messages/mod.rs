//! Local message facts, revisions, source captures, relationships, and export.

use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const DATABASE_VERSION: i64 = 1;
const DATABASE_NAME: &str = "Messages.sqlite3";

#[derive(Clone, Debug)]
pub struct MessageStore {
    database: PathBuf,
    assets: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionInput {
    pub content_version: String,
    pub title: String,
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAnchorInput {
    pub canonical_locator: String,
    pub section: String,
    pub selected_text: String,
    pub prefix_text: String,
    pub suffix_text: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotResourceInput {
    pub path: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshotInput {
    pub fragment_html: String,
    pub reader_css: String,
    pub book_css: String,
    pub user_css: String,
    pub presentation_json: String,
    pub resources: Vec<SnapshotResourceInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootMessageDraft {
    pub edition: EditionInput,
    pub anchor: SourceAnchorInput,
    pub snapshot: SourceSnapshotInput,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyDraft {
    pub conversation_id: String,
    pub reply_to_message_id: String,
    pub text: String,
    pub reference_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSearch {
    pub edition_id: String,
    pub text: String,
    pub section: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReselectDraft {
    pub message_id: String,
    pub expected_source_id: String,
    pub anchor: SourceAnchorInput,
    pub snapshot: SourceSnapshotInput,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedRoot {
    pub conversation_id: String,
    pub message_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedRevision {
    pub message_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedMessage {
    pub message_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedSource {
    pub source_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionView {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSourceView {
    pub id: String,
    pub original_locator: String,
    pub canonical_locator: String,
    pub section: String,
    pub selected_text: String,
    pub prefix_text: String,
    pub suffix_text: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageView {
    pub id: String,
    pub revision_id: String,
    pub kind: String,
    pub text: String,
    pub reply_to_message_id: Option<String>,
    pub reference_ids: Vec<String>,
    pub source: Option<MessageSourceView>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationView {
    pub id: String,
    pub edition_id: String,
    pub messages: Vec<MessageView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRelationships {
    pub references: Vec<String>,
    pub referenced_by: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSearchHit {
    pub message_id: String,
    pub conversation_id: String,
    pub section: String,
    pub selected_text: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotResourceView {
    pub path: String,
    pub media_type: String,
    pub content_hash: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshotView {
    pub fragment_html: String,
    pub reader_css: String,
    pub book_css: String,
    pub user_css: String,
    pub presentation_json: String,
    pub resources: Vec<SnapshotResourceView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCaptureView {
    pub source: MessageSourceView,
    pub snapshot: SourceSnapshotView,
    pub current: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotResourceData {
    pub media_type: String,
    pub content_hash: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct PreparedResource {
    path: String,
    media_type: String,
    content_hash: Vec<u8>,
    byte_length: i64,
    asset_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageError {
    InvalidRoot,
    InvalidInput,
    UnknownConversation,
    UnknownMessage,
    RevisionConflict,
    FutureDatabase,
    CorruptData,
    Database,
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

    pub fn create_root(&self, draft: RootMessageDraft) -> Result<CreatedRoot, MessageError> {
        validate_root(&draft)?;
        let resources = self.prepare_resources(&draft.snapshot.resources)?;
        let edition_id = decode_hex::<32>(&draft.edition.content_version)?;
        let content_hash = decode_hex::<32>(&draft.anchor.content_hash)?;
        let now = now_millis()?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let conversation_id = random_id(&transaction)?;
        let message_id = random_id(&transaction)?;
        let revision_id = random_id(&transaction)?;
        let snapshot_id = random_id(&transaction)?;
        let source_id = random_id(&transaction)?;
        let outbox_id = random_id(&transaction)?;
        let authors = serde_json::to_string(&draft.edition.authors)
            .map_err(|_| MessageError::InvalidInput)?;
        let work_id = transaction
            .query_row(
                "SELECT work_id FROM edition WHERE id = ?1",
                params![edition_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| MessageError::Database)?;
        let work_id = match work_id {
            Some(work_id) => work_id,
            None => {
                let work_id = random_id(&transaction)?;
                transaction
                    .execute(
                        "INSERT INTO work (id, title, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?3)",
                        params![work_id, draft.edition.title, now],
                    )
                    .map_err(|_| MessageError::Database)?;
                transaction
                    .execute(
                        "INSERT INTO edition (id, work_id, title, authors_json, imported_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![edition_id, work_id, draft.edition.title, authors, now],
                    )
                    .map_err(|_| MessageError::Database)?;
                work_id
            }
        };
        transaction
            .execute(
                "UPDATE edition SET title = ?2, authors_json = ?3 WHERE id = ?1",
                params![edition_id, draft.edition.title, authors],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "UPDATE work SET title = ?2, updated_at_ms = ?3 WHERE id = ?1",
                params![work_id, draft.edition.title, now],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO conversation (id, edition_id, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?3)",
                params![conversation_id, edition_id, now],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO message (id, conversation_id, ordinal, author_type, created_at_ms, updated_at_ms) VALUES (?1, ?2, 0, 'user', ?3, ?3)",
                params![message_id, conversation_id, now],
            )
            .map_err(|_| MessageError::Database)?;
        let (kind, text) = draft
            .text
            .as_deref()
            .map_or(("source-only", ""), |value| ("text", value));
        let content_json = serde_json::json!({ "schema": 1, "kind": kind, "text": text });
        transaction
            .execute(
                "INSERT INTO message_revision (id, message_id, schema_version, kind, content_json, plain_text, created_at_ms) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
                params![revision_id, message_id, kind, content_json.to_string(), text, now],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO source_snapshot (id, fragment_html, reader_css, book_css, user_css, presentation_json, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    snapshot_id,
                    draft.snapshot.fragment_html,
                    draft.snapshot.reader_css,
                    draft.snapshot.book_css,
                    draft.snapshot.user_css,
                    draft.snapshot.presentation_json,
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        insert_snapshot_resources(&transaction, &snapshot_id, &resources)?;
        transaction
            .execute(
                "INSERT INTO source_anchor (id, message_id, snapshot_id, original_locator_json, current_locator_json, section_id, selected_text, prefix_text, suffix_text, content_hash, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    source_id,
                    message_id,
                    snapshot_id,
                    draft.anchor.canonical_locator,
                    draft.anchor.section,
                    draft.anchor.selected_text,
                    draft.anchor.prefix_text,
                    draft.anchor.suffix_text,
                    content_hash,
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "UPDATE message SET current_revision_id = ?2, current_source_anchor_id = ?3 WHERE id = ?1",
                params![message_id, revision_id, source_id],
            )
            .map_err(|_| MessageError::Database)?;
        refresh_search(&transaction, &message_id)?;
        transaction
            .execute(
                "UPDATE conversation SET root_message_id = ?2 WHERE id = ?1",
                params![conversation_id, message_id],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms) VALUES (?1, 'message', ?2, 'message-created', ?3, ?4)",
                params![
                    outbox_id,
                    message_id,
                    serde_json::json!({ "messageId": encode_hex(&message_id) }).to_string(),
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)?;
        Ok(CreatedRoot {
            conversation_id: encode_hex(&conversation_id),
            message_id: encode_hex(&message_id),
            revision_id: encode_hex(&revision_id),
        })
    }

    pub fn conversation(&self, id: &str) -> Result<ConversationView, MessageError> {
        let conversation_id = decode_hex::<16>(id)?;
        let connection = self.connect()?;
        let edition_id: Vec<u8> = connection
            .query_row(
                "SELECT edition_id FROM conversation WHERE id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownConversation,
                _ => MessageError::Database,
            })?;
        let mut statement = connection
            .prepare(
                "SELECT m.id, r.id, r.kind, r.plain_text, m.reply_to_message_id, a.id, a.original_locator_json, a.current_locator_json, a.section_id, a.selected_text, a.prefix_text, a.suffix_text, a.content_hash, m.deleted_at_ms
                 FROM message m
                 JOIN message_revision r ON r.id = m.current_revision_id AND r.message_id = m.id
                 LEFT JOIN source_anchor a ON a.id = m.current_source_anchor_id AND a.message_id = m.id
                 WHERE m.conversation_id = ?1
                 ORDER BY m.ordinal",
            )
            .map_err(|_| MessageError::Database)?;
        let mut messages = statement
            .query_map(params![conversation_id], |row| {
                let message_id: Vec<u8> = row.get(0)?;
                let revision_id: Vec<u8> = row.get(1)?;
                let reply_to: Option<Vec<u8>> = row.get(4)?;
                let source_id: Option<Vec<u8>> = row.get(5)?;
                let content_hash: Option<Vec<u8>> = row.get(12)?;
                let deleted = row.get::<_, Option<i64>>(13)?.is_some();
                Ok(MessageView {
                    id: encode_hex(&message_id),
                    revision_id: encode_hex(&revision_id),
                    kind: if deleted {
                        "deleted".into()
                    } else {
                        row.get(2)?
                    },
                    text: if deleted { String::new() } else { row.get(3)? },
                    reply_to_message_id: reply_to.as_deref().map(encode_hex),
                    reference_ids: Vec::new(),
                    source: (!deleted).then_some(source_id).flatten().map(|source_id| {
                        MessageSourceView {
                            id: encode_hex(&source_id),
                            original_locator: row.get(6).unwrap_or_default(),
                            canonical_locator: row.get(7).unwrap_or_default(),
                            section: row.get(8).unwrap_or_default(),
                            selected_text: row.get(9).unwrap_or_default(),
                            prefix_text: row.get(10).unwrap_or_default(),
                            suffix_text: row.get(11).unwrap_or_default(),
                            content_hash: content_hash
                                .as_deref()
                                .map_or_else(String::new, encode_hex),
                        }
                    }),
                    deleted,
                })
            })
            .map_err(|_| MessageError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MessageError::Database)?;
        drop(statement);
        for message in &mut messages {
            message.reference_ids = relationship_ids(&connection, &message.id, true)?;
        }
        Ok(ConversationView {
            id: id.to_owned(),
            edition_id: encode_hex(&edition_id),
            messages,
        })
    }

    pub fn reply(&self, draft: ReplyDraft) -> Result<CreatedMessage, MessageError> {
        if draft.text.trim().is_empty()
            || draft.text.chars().count() > 8_000
            || draft.reference_ids.len() > 32
        {
            return Err(MessageError::InvalidInput);
        }
        let conversation = decode_hex::<16>(&draft.conversation_id)?;
        let parent = decode_hex::<16>(&draft.reply_to_message_id)?;
        let mut references = draft
            .reference_ids
            .iter()
            .map(|id| decode_hex::<16>(id))
            .collect::<Result<Vec<_>, _>>()?;
        references.sort();
        references.dedup();
        if references.iter().any(|id| id == &parent) {
            return Err(MessageError::InvalidInput);
        }
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let edition: Vec<u8> = transaction
            .query_row(
                "SELECT edition_id FROM conversation WHERE id = ?1",
                params![conversation],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownConversation,
                _ => MessageError::Database,
            })?;
        let parent_valid: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM message WHERE id = ?1 AND conversation_id = ?2 AND deleted_at_ms IS NULL)",
                params![parent, conversation],
                |row| row.get(0),
            )
            .map_err(|_| MessageError::Database)?;
        if !parent_valid {
            return Err(MessageError::InvalidInput);
        }
        for target in &references {
            let valid: bool = transaction
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM message target
                        JOIN conversation target_conversation ON target_conversation.id = target.conversation_id
                        WHERE target.id = ?1 AND target.deleted_at_ms IS NULL AND target_conversation.edition_id = ?2
                    )",
                    params![target, edition],
                    |row| row.get(0),
                )
                .map_err(|_| MessageError::Database)?;
            if !valid {
                return Err(MessageError::InvalidInput);
            }
        }
        let message = random_id(&transaction)?;
        let revision = random_id(&transaction)?;
        let outbox = random_id(&transaction)?;
        let now = now_millis()?;
        let text = draft.text.trim();
        let ordinal: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM message WHERE conversation_id = ?1",
                params![conversation],
                |row| row.get(0),
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO message (id, conversation_id, ordinal, author_type, reply_to_message_id, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?5)",
                params![message, conversation, ordinal, parent, now],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO message_revision (id, message_id, schema_version, kind, content_json, plain_text, created_at_ms) VALUES (?1, ?2, 1, 'text', ?3, ?4, ?5)",
                params![
                    revision,
                    message,
                    serde_json::json!({ "schema": 1, "kind": "text", "text": text }).to_string(),
                    text,
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "UPDATE message SET current_revision_id = ?2 WHERE id = ?1",
                params![message, revision],
            )
            .map_err(|_| MessageError::Database)?;
        refresh_search(&transaction, &message)?;
        for target in references {
            transaction
                .execute(
                    "INSERT INTO message_reference (source_message_id, target_message_id, kind, created_at_ms) VALUES (?1, ?2, 'quote', ?3)",
                    params![message, target, now],
                )
                .map_err(|_| MessageError::Database)?;
        }
        transaction
            .execute(
                "UPDATE conversation SET updated_at_ms = ?2 WHERE id = ?1",
                params![conversation, now],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms) VALUES (?1, 'message', ?2, 'message-created', ?3, ?4)",
                params![
                    outbox,
                    message,
                    serde_json::json!({ "messageId": encode_hex(&message) }).to_string(),
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)?;
        Ok(CreatedMessage {
            message_id: encode_hex(&message),
            revision_id: encode_hex(&revision),
        })
    }

    pub fn relationships(&self, message_id: &str) -> Result<MessageRelationships, MessageError> {
        let message = decode_hex::<16>(message_id)?;
        let connection = self.connect()?;
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM message WHERE id = ?1)",
                params![message],
                |row| row.get(0),
            )
            .map_err(|_| MessageError::Database)?;
        if !exists {
            return Err(MessageError::UnknownMessage);
        }
        Ok(MessageRelationships {
            references: relationship_ids(&connection, message_id, true)?,
            referenced_by: relationship_ids(&connection, message_id, false)?,
        })
    }

    pub fn search(&self, search: MessageSearch) -> Result<Vec<MessageSearchHit>, MessageError> {
        let edition = decode_hex::<32>(&search.edition_id)?;
        let text = search.text.trim();
        if text.is_empty()
            || text.chars().count() > 256
            || search
                .section
                .as_ref()
                .is_some_and(|section| section.is_empty() || section.len() > 256)
        {
            return Err(MessageError::InvalidInput);
        }
        let connection = self.connect()?;
        let section = search.section.as_deref();
        let (sql, term) = if text.chars().count() < 3 {
            (
                "SELECT message_id, conversation_id, section_id, selected_text, plain_text
                 FROM message_search
                 WHERE edition_id = ?2 AND (?3 IS NULL OR section_id = ?3)
                   AND (selected_text LIKE ?1 ESCAPE '\\' OR plain_text LIKE ?1 ESCAPE '\\')
                 ORDER BY message_id LIMIT 200",
                format!("%{}%", escape_like(text)),
            )
        } else {
            (
                "SELECT message_id, conversation_id, section_id, selected_text, plain_text
                 FROM message_search
                 WHERE message_search MATCH ?1 AND edition_id = ?2
                   AND (?3 IS NULL OR section_id = ?3)
                 ORDER BY rank, message_id LIMIT 200",
                format!("\"{}\"", text.replace('"', "\"\"")),
            )
        };
        let mut statement = connection
            .prepare(sql)
            .map_err(|_| MessageError::Database)?;
        statement
            .query_map(params![term, encode_hex(&edition), section], |row| {
                Ok(MessageSearchHit {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    section: row.get(2)?,
                    selected_text: row.get(3)?,
                    text: row.get(4)?,
                })
            })
            .map_err(|_| MessageError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MessageError::Database)
    }

    pub fn reselect(&self, draft: ReselectDraft) -> Result<CreatedSource, MessageError> {
        validate_source(&draft.anchor, &draft.snapshot)?;
        let message = decode_hex::<16>(&draft.message_id)?;
        let expected = decode_hex::<16>(&draft.expected_source_id)?;
        let content_hash = decode_hex::<32>(&draft.anchor.content_hash)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let current: Option<Vec<u8>> = transaction
            .query_row(
                "SELECT current_source_anchor_id FROM message WHERE id = ?1 AND deleted_at_ms IS NULL",
                params![message],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownMessage,
                _ => MessageError::Database,
            })?;
        if current.as_deref() != Some(expected.as_slice()) {
            return Err(MessageError::RevisionConflict);
        }
        let resources = self.prepare_resources(&draft.snapshot.resources)?;
        let source = random_id(&transaction)?;
        let snapshot = random_id(&transaction)?;
        let outbox = random_id(&transaction)?;
        let now = now_millis()?;
        transaction
            .execute(
                "INSERT INTO source_snapshot (id, fragment_html, reader_css, book_css, user_css, presentation_json, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    snapshot,
                    draft.snapshot.fragment_html,
                    draft.snapshot.reader_css,
                    draft.snapshot.book_css,
                    draft.snapshot.user_css,
                    draft.snapshot.presentation_json,
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        insert_snapshot_resources(&transaction, &snapshot, &resources)?;
        transaction
            .execute(
                "INSERT INTO source_anchor (id, message_id, snapshot_id, original_locator_json, current_locator_json, section_id, selected_text, prefix_text, suffix_text, content_hash, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    source,
                    message,
                    snapshot,
                    draft.anchor.canonical_locator,
                    draft.anchor.section,
                    draft.anchor.selected_text,
                    draft.anchor.prefix_text,
                    draft.anchor.suffix_text,
                    content_hash,
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        let changed = transaction
            .execute(
                "UPDATE message SET current_source_anchor_id = ?2, updated_at_ms = ?3
                 WHERE id = ?1 AND current_source_anchor_id = ?4 AND deleted_at_ms IS NULL",
                params![message, source, now, expected],
            )
            .map_err(|_| MessageError::Database)?;
        if changed != 1 {
            return Err(MessageError::RevisionConflict);
        }
        refresh_conversation_search(&transaction, &message)?;
        transaction
            .execute(
                "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms) VALUES (?1, 'message', ?2, 'message-source-reselected', ?3, ?4)",
                params![
                    outbox,
                    message,
                    serde_json::json!({ "messageId": draft.message_id, "sourceId": encode_hex(&source) }).to_string(),
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)?;
        Ok(CreatedSource {
            source_id: encode_hex(&source),
        })
    }

    pub fn delete(&self, message_id: &str, expected_revision_id: &str) -> Result<(), MessageError> {
        let message = decode_hex::<16>(message_id)?;
        let expected = decode_hex::<16>(expected_revision_id)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let current: Vec<u8> = transaction
            .query_row(
                "SELECT current_revision_id FROM message WHERE id = ?1 AND deleted_at_ms IS NULL",
                params![message],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownMessage,
                _ => MessageError::Database,
            })?;
        if current != expected {
            return Err(MessageError::RevisionConflict);
        }
        let now = now_millis()?;
        let outbox = random_id(&transaction)?;
        transaction
            .execute(
                "UPDATE message SET deleted_at_ms = ?2, updated_at_ms = ?2 WHERE id = ?1",
                params![message, now],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "DELETE FROM message_search WHERE message_id = ?1",
                params![message_id],
            )
            .map_err(|_| MessageError::Database)?;
        transaction
            .execute(
                "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms) VALUES (?1, 'message', ?2, 'message-deleted', ?3, ?4)",
                params![
                    outbox,
                    message,
                    serde_json::json!({ "messageId": message_id }).to_string(),
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)
    }

    pub fn revise(
        &self,
        message_id: &str,
        expected_revision_id: &str,
        text: Option<&str>,
    ) -> Result<CreatedRevision, MessageError> {
        let message = decode_hex::<16>(message_id)?;
        let expected = decode_hex::<16>(expected_revision_id)?;
        let (kind, plain_text) = match text {
            Some(value) if !value.trim().is_empty() && value.chars().count() <= 8_000 => {
                ("text", value.trim())
            }
            None => ("source-only", ""),
            _ => return Err(MessageError::InvalidInput),
        };
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let current: Vec<u8> = transaction
            .query_row(
                "SELECT current_revision_id FROM message WHERE id = ?1 AND deleted_at_ms IS NULL",
                params![message],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownMessage,
                _ => MessageError::Database,
            })?;
        if current != expected {
            return Err(MessageError::RevisionConflict);
        }
        let revision = random_id(&transaction)?;
        let outbox = random_id(&transaction)?;
        let now = now_millis()?;
        let content_json =
            serde_json::json!({ "schema": 1, "kind": kind, "text": plain_text }).to_string();
        transaction
            .execute(
                "INSERT INTO message_revision (id, message_id, schema_version, kind, content_json, plain_text, created_at_ms) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
                params![revision, message, kind, content_json, plain_text, now],
            )
            .map_err(|_| MessageError::Database)?;
        let changed = transaction
            .execute(
                "UPDATE message SET current_revision_id = ?2, updated_at_ms = ?3 WHERE id = ?1 AND current_revision_id = ?4 AND deleted_at_ms IS NULL",
                params![message, revision, now, expected],
            )
            .map_err(|_| MessageError::Database)?;
        if changed != 1 {
            return Err(MessageError::RevisionConflict);
        }
        refresh_search(&transaction, &message)?;
        transaction
            .execute(
                "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms) VALUES (?1, 'message', ?2, 'message-revised', ?3, ?4)",
                params![
                    outbox,
                    message,
                    serde_json::json!({ "messageId": message_id, "revisionId": encode_hex(&revision) }).to_string(),
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)?;
        Ok(CreatedRevision {
            message_id: message_id.to_owned(),
            revision_id: encode_hex(&revision),
        })
    }

    pub fn revisions(&self, message_id: &str) -> Result<Vec<RevisionView>, MessageError> {
        let message = decode_hex::<16>(message_id)?;
        let connection = self.connect()?;
        if !connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM message WHERE id = ?1)",
                params![message],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| MessageError::Database)?
        {
            return Err(MessageError::UnknownMessage);
        }
        let mut statement = connection
            .prepare(
                "SELECT id, kind, plain_text, created_at_ms FROM message_revision WHERE message_id = ?1 ORDER BY created_at_ms, id",
            )
            .map_err(|_| MessageError::Database)?;
        statement
            .query_map(params![message], |row| {
                let id: Vec<u8> = row.get(0)?;
                Ok(RevisionView {
                    id: encode_hex(&id),
                    kind: row.get(1)?,
                    text: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|_| MessageError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MessageError::Database)
    }

    pub fn source_captures(
        &self,
        message_id: &str,
    ) -> Result<Vec<SourceCaptureView>, MessageError> {
        let message = decode_hex::<16>(message_id)?;
        let connection = self.connect()?;
        let current: Option<Vec<u8>> = connection
            .query_row(
                "SELECT current_source_anchor_id FROM message WHERE id = ?1",
                params![message],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownMessage,
                _ => MessageError::Database,
            })?;
        let mut statement = connection
            .prepare(
                "SELECT a.id, a.original_locator_json, a.current_locator_json, a.section_id,
                        a.selected_text, a.prefix_text, a.suffix_text, a.content_hash, a.created_at_ms,
                        s.id, s.fragment_html, s.reader_css, s.book_css, s.user_css, s.presentation_json
                 FROM source_anchor a
                 JOIN source_snapshot s ON s.id = a.snapshot_id
                 WHERE a.message_id = ?1
                 ORDER BY a.created_at_ms, a.id",
            )
            .map_err(|_| MessageError::Database)?;
        let rows = statement
            .query_map(params![message], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    MessageSourceView {
                        id: String::new(),
                        original_locator: row.get(1)?,
                        canonical_locator: row.get(2)?,
                        section: row.get(3)?,
                        selected_text: row.get(4)?,
                        prefix_text: row.get(5)?,
                        suffix_text: row.get(6)?,
                        content_hash: encode_hex(&row.get::<_, Vec<u8>>(7)?),
                    },
                    row.get::<_, i64>(8)?,
                    row.get::<_, Vec<u8>>(9)?,
                    SourceSnapshotView {
                        fragment_html: row.get(10)?,
                        reader_css: row.get(11)?,
                        book_css: row.get(12)?,
                        user_css: row.get(13)?,
                        presentation_json: row.get(14)?,
                        resources: Vec::new(),
                    },
                ))
            })
            .map_err(|_| MessageError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MessageError::Database)?;
        drop(statement);
        rows.into_iter()
            .map(
                |(source_id, mut source, created_at, snapshot_id, mut snapshot)| {
                    source.id = encode_hex(&source_id);
                    snapshot.resources = snapshot_resources(&connection, &snapshot_id)?;
                    Ok(SourceCaptureView {
                        current: current.as_deref() == Some(source_id.as_slice()),
                        source,
                        snapshot,
                        created_at,
                    })
                },
            )
            .collect()
    }

    pub fn read_snapshot_resource(
        &self,
        source_id: &str,
        source_path: &str,
    ) -> Result<SnapshotResourceData, MessageError> {
        validate_resource_path(source_path)?;
        let source = decode_hex::<16>(source_id)?;
        let connection = self.connect()?;
        let (media_type, expected_hash, asset_name): (String, Vec<u8>, String) = connection
            .query_row(
                "SELECT r.media_type, r.content_hash, r.asset_name
                 FROM source_anchor a
                 JOIN snapshot_resource r ON r.snapshot_id = a.snapshot_id
                 WHERE a.id = ?1 AND r.source_path = ?2",
                params![source, source_path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => MessageError::InvalidInput,
                _ => MessageError::Database,
            })?;
        let bytes =
            fs::read(self.assets.join(&asset_name)).map_err(|_| MessageError::CorruptData)?;
        let actual = Sha256::digest(&bytes);
        if actual.as_slice() != expected_hash || encode_hex(&actual) != asset_name {
            return Err(MessageError::CorruptData);
        }
        Ok(SnapshotResourceData {
            media_type,
            content_hash: encode_hex(&expected_hash),
            bytes,
        })
    }

    fn prepare_resources(
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

    fn connect(&self) -> Result<Connection, MessageError> {
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

impl MessageError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid-message-root",
            Self::InvalidInput => "invalid-message-input",
            Self::UnknownConversation => "unknown-conversation",
            Self::UnknownMessage => "unknown-message",
            Self::RevisionConflict => "message-revision-conflict",
            Self::FutureDatabase => "future-message-database",
            Self::CorruptData => "corrupt-message-data",
            Self::Database => "message-database",
        }
    }
}

impl fmt::Display for MessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for MessageError {}

fn validate_root(draft: &RootMessageDraft) -> Result<(), MessageError> {
    decode_hex::<32>(&draft.edition.content_version)?;
    if draft.edition.title.trim().is_empty()
        || draft.edition.title.chars().count() > 512
        || draft.edition.authors.len() > 16
        || draft
            .edition
            .authors
            .iter()
            .any(|value| value.trim().is_empty() || value.chars().count() > 512)
        || draft
            .text
            .as_ref()
            .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 8_000)
    {
        return Err(MessageError::InvalidInput);
    }
    validate_source(&draft.anchor, &draft.snapshot)
}

fn validate_source(
    anchor: &SourceAnchorInput,
    snapshot: &SourceSnapshotInput,
) -> Result<(), MessageError> {
    decode_hex::<32>(&anchor.content_hash)?;
    if anchor.canonical_locator.len() > 8192
        || serde_json::from_str::<serde_json::Value>(&anchor.canonical_locator).is_err()
        || anchor.section.is_empty()
        || anchor.section.len() > 256
        || anchor.selected_text.trim().is_empty()
        || anchor.selected_text.encode_utf16().count() > 4096
        || anchor.prefix_text.encode_utf16().count() > 32
        || anchor.suffix_text.encode_utf16().count() > 32
        || snapshot.fragment_html.is_empty()
        || snapshot.fragment_html.len() > 262_144
        || snapshot.reader_css.len() > 1_048_576
        || snapshot.book_css.len() > 1_048_576
        || snapshot.user_css.len() > 32_768
        || serde_json::from_str::<serde_json::Value>(&snapshot.presentation_json).is_err()
    {
        return Err(MessageError::InvalidInput);
    }
    validate_resources(&snapshot.resources)
}

fn validate_resources(resources: &[SnapshotResourceInput]) -> Result<(), MessageError> {
    if resources.len() > 64 {
        return Err(MessageError::InvalidInput);
    }
    let mut paths = HashSet::with_capacity(resources.len());
    let mut total = 0usize;
    for resource in resources {
        validate_resource_path(&resource.path)?;
        if !paths.insert(resource.path.as_str())
            || resource.bytes.is_empty()
            || resource.bytes.len() > 16 * 1024 * 1024
            || !(resource.media_type.starts_with("image/")
                || resource.media_type.starts_with("font/")
                || matches!(
                    resource.media_type.as_str(),
                    "application/font-woff"
                        | "application/font-sfnt"
                        | "application/vnd.ms-fontobject"
                ))
        {
            return Err(MessageError::InvalidInput);
        }
        total = total
            .checked_add(resource.bytes.len())
            .ok_or(MessageError::InvalidInput)?;
    }
    if total > 32 * 1024 * 1024 {
        return Err(MessageError::InvalidInput);
    }
    Ok(())
}

fn validate_resource_path(path: &str) -> Result<(), MessageError> {
    if path.is_empty()
        || path.len() > 2048
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(MessageError::InvalidInput);
    }
    Ok(())
}

fn random_id(transaction: &Transaction<'_>) -> Result<Vec<u8>, MessageError> {
    transaction
        .query_row("SELECT randomblob(16)", [], |row| row.get(0))
        .map_err(|_| MessageError::Database)
}

fn now_millis() -> Result<i64, MessageError> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| MessageError::Database)?
        .as_millis();
    i64::try_from(value).map_err(|_| MessageError::Database)
}

fn decode_hex<const N: usize>(value: &str) -> Result<Vec<u8>, MessageError> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MessageError::InvalidInput);
    }
    (0..N)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::InvalidInput)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}

fn refresh_search(transaction: &Transaction<'_>, message: &[u8]) -> Result<(), MessageError> {
    let row = transaction
        .query_row(
            "SELECT c.edition_id, m.conversation_id, r.plain_text,
                    COALESCE(a.section_id, root_a.section_id, ''),
                    COALESCE(a.selected_text, root_a.selected_text, '')
             FROM message m
             JOIN conversation c ON c.id = m.conversation_id
             JOIN message_revision r ON r.id = m.current_revision_id AND r.message_id = m.id
             LEFT JOIN source_anchor a ON a.id = m.current_source_anchor_id AND a.message_id = m.id
             LEFT JOIN message root ON root.id = c.root_message_id
             LEFT JOIN source_anchor root_a ON root_a.id = root.current_source_anchor_id
             WHERE m.id = ?1 AND m.deleted_at_ms IS NULL",
            params![message],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .map_err(|_| MessageError::Database)?;
    let message_id = encode_hex(message);
    transaction
        .execute(
            "DELETE FROM message_search WHERE message_id = ?1",
            params![message_id],
        )
        .map_err(|_| MessageError::Database)?;
    transaction
        .execute(
            "INSERT INTO message_search (message_id, conversation_id, edition_id, section_id, selected_text, plain_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                message_id,
                encode_hex(&row.1),
                encode_hex(&row.0),
                row.3,
                row.4,
                row.2
            ],
        )
        .map_err(|_| MessageError::Database)?;
    Ok(())
}

fn refresh_conversation_search(
    transaction: &Transaction<'_>,
    message: &[u8],
) -> Result<(), MessageError> {
    let conversation: Vec<u8> = transaction
        .query_row(
            "SELECT conversation_id FROM message WHERE id = ?1",
            params![message],
            |row| row.get(0),
        )
        .map_err(|_| MessageError::Database)?;
    let mut statement = transaction
        .prepare("SELECT id FROM message WHERE conversation_id = ?1 AND deleted_at_ms IS NULL")
        .map_err(|_| MessageError::Database)?;
    let messages = statement
        .query_map(params![conversation], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|_| MessageError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::Database)?;
    drop(statement);
    for message in messages {
        refresh_search(transaction, &message)?;
    }
    Ok(())
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn insert_snapshot_resources(
    transaction: &Transaction<'_>,
    snapshot: &[u8],
    resources: &[PreparedResource],
) -> Result<(), MessageError> {
    for resource in resources {
        transaction
            .execute(
                "INSERT INTO snapshot_resource (snapshot_id, source_path, media_type, content_hash, byte_length, asset_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    snapshot,
                    resource.path,
                    resource.media_type,
                    resource.content_hash,
                    resource.byte_length,
                    resource.asset_name
                ],
            )
            .map_err(|_| MessageError::Database)?;
    }
    Ok(())
}

fn snapshot_resources(
    connection: &Connection,
    snapshot: &[u8],
) -> Result<Vec<SnapshotResourceView>, MessageError> {
    let mut statement = connection
        .prepare(
            "SELECT source_path, media_type, content_hash, byte_length
             FROM snapshot_resource WHERE snapshot_id = ?1 ORDER BY source_path",
        )
        .map_err(|_| MessageError::Database)?;
    statement
        .query_map(params![snapshot], |row| {
            let hash: Vec<u8> = row.get(2)?;
            let byte_length: i64 = row.get(3)?;
            Ok(SnapshotResourceView {
                path: row.get(0)?,
                media_type: row.get(1)?,
                content_hash: encode_hex(&hash),
                byte_length: byte_length as u64,
            })
        })
        .map_err(|_| MessageError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::Database)
}

const SCHEMA_V1: &str = r#"
CREATE TABLE work (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    title TEXT NOT NULL CHECK(title <> ''),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE edition (
    id BLOB PRIMARY KEY CHECK(length(id) = 32),
    work_id BLOB NOT NULL REFERENCES work(id),
    title TEXT NOT NULL CHECK(title <> ''),
    authors_json TEXT NOT NULL CHECK(json_valid(authors_json)),
    imported_at_ms INTEGER NOT NULL
);
CREATE TABLE conversation (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    edition_id BLOB NOT NULL REFERENCES edition(id),
    root_message_id BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE message (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    conversation_id BLOB NOT NULL REFERENCES conversation(id),
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    author_type TEXT NOT NULL CHECK(author_type IN ('user', 'assistant', 'system')),
    reply_to_message_id BLOB REFERENCES message(id),
    current_revision_id BLOB,
    current_source_anchor_id BLOB,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    deleted_at_ms INTEGER,
    UNIQUE(conversation_id, ordinal),
    FOREIGN KEY (id, current_revision_id)
        REFERENCES message_revision(message_id, id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (id, current_source_anchor_id)
        REFERENCES source_anchor(message_id, id)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE message_revision (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    message_id BLOB NOT NULL REFERENCES message(id),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    kind TEXT NOT NULL CHECK(kind IN ('source-only', 'text')),
    content_json TEXT NOT NULL CHECK(json_valid(content_json)),
    plain_text TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    UNIQUE(message_id, id)
);
CREATE TABLE source_snapshot (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    fragment_html TEXT NOT NULL CHECK(fragment_html <> ''),
    reader_css TEXT NOT NULL,
    book_css TEXT NOT NULL,
    user_css TEXT NOT NULL,
    presentation_json TEXT NOT NULL CHECK(json_valid(presentation_json)),
    created_at_ms INTEGER NOT NULL
);
CREATE TABLE source_anchor (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    message_id BLOB NOT NULL REFERENCES message(id),
    snapshot_id BLOB NOT NULL REFERENCES source_snapshot(id),
    original_locator_json TEXT NOT NULL CHECK(json_valid(original_locator_json)),
    current_locator_json TEXT NOT NULL CHECK(json_valid(current_locator_json)),
    section_id TEXT NOT NULL CHECK(section_id <> ''),
    selected_text TEXT NOT NULL CHECK(selected_text <> ''),
    prefix_text TEXT NOT NULL,
    suffix_text TEXT NOT NULL,
    content_hash BLOB NOT NULL CHECK(length(content_hash) = 32),
    created_at_ms INTEGER NOT NULL,
    UNIQUE(message_id, id)
);
CREATE TABLE message_reference (
    source_message_id BLOB NOT NULL REFERENCES message(id),
    target_message_id BLOB NOT NULL REFERENCES message(id),
    kind TEXT NOT NULL CHECK(kind = 'quote'),
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY(source_message_id, target_message_id),
    CHECK(source_message_id <> target_message_id)
);
CREATE TABLE snapshot_resource (
    snapshot_id BLOB NOT NULL REFERENCES source_snapshot(id),
    source_path TEXT NOT NULL,
    media_type TEXT NOT NULL,
    content_hash BLOB NOT NULL CHECK(length(content_hash) = 32),
    byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
    asset_name TEXT NOT NULL,
    PRIMARY KEY(snapshot_id, source_path)
);
CREATE TABLE outbox_event (
    id BLOB PRIMARY KEY CHECK(length(id) = 16),
    aggregate_type TEXT NOT NULL,
    aggregate_id BLOB NOT NULL CHECK(length(aggregate_id) = 16),
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
    created_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
    next_attempt_ms INTEGER
);
CREATE VIRTUAL TABLE message_search USING fts5(
    message_id UNINDEXED,
    conversation_id UNINDEXED,
    edition_id UNINDEXED,
    section_id UNINDEXED,
    selected_text,
    plain_text,
    tokenize = 'trigram'
);
CREATE INDEX conversation_edition_created ON conversation(edition_id, created_at_ms DESC, id);
CREATE INDEX source_anchor_section ON source_anchor(section_id, message_id);
CREATE INDEX message_reference_target ON message_reference(target_message_id, source_message_id);
"#;

fn relationship_ids(
    connection: &Connection,
    message_id: &str,
    outgoing: bool,
) -> Result<Vec<String>, MessageError> {
    let message = decode_hex::<16>(message_id)?;
    let sql = if outgoing {
        "SELECT target_message_id FROM message_reference WHERE source_message_id = ?1 ORDER BY target_message_id"
    } else {
        "SELECT source_message_id FROM message_reference WHERE target_message_id = ?1 ORDER BY source_message_id"
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(|_| MessageError::Database)?;
    statement
        .query_map(params![message], |row| {
            let id: Vec<u8> = row.get(0)?;
            Ok(encode_hex(&id))
        })
        .map_err(|_| MessageError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::Database)
}
