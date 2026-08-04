use std::collections::HashSet;

use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{
    model::*,
    store::MessageStore,
    util::{decode_hex, encode_hex, now_millis, random_id},
};

impl MessageStore {
    pub fn import_legacy_annotations(
        &self,
        input: LegacyImport,
    ) -> Result<LegacyImportResult, MessageError> {
        validate_legacy_import(&input)?;
        let edition = decode_hex::<32>(&input.edition.content_version)?;
        let record_hash = decode_hex::<32>(&input.record_hash)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let completed = transaction
            .query_row(
                "SELECT record_hash, item_count FROM legacy_import_state
                 WHERE edition_id = ?1 AND source_key = ?2",
                params![edition, input.source_key],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(|_| MessageError::Database)?;
        if let Some((stored_hash, stored_count)) = completed {
            if stored_hash != record_hash || stored_count != input.items.len() as i64 {
                return Err(MessageError::LegacyConflict);
            }
            return Ok(LegacyImportResult {
                imported: 0,
                already_complete: true,
                record_hash: input.record_hash,
                items: imported_items(&transaction, &edition, &input.source_key)?,
            });
        }

        ensure_edition(&transaction, &edition, &input.edition)?;
        let completed_at = now_millis()?;
        transaction
            .execute(
                "INSERT INTO legacy_import_state (edition_id, source_key, record_hash, item_count, completed_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    edition,
                    input.source_key,
                    record_hash,
                    input.items.len() as i64,
                    completed_at
                ],
            )
            .map_err(|_| MessageError::Database)?;

        let mut imported = Vec::with_capacity(input.items.len());
        for item in &input.items {
            let conversation = random_id(&transaction)?;
            let message = random_id(&transaction)?;
            let revision = random_id(&transaction)?;
            let snapshot = random_id(&transaction)?;
            let source = random_id(&transaction)?;
            let outbox = random_id(&transaction)?;
            let content_hash = decode_hex::<32>(&item.anchor.content_hash)?;
            let (kind, text) = item
                .note
                .as_deref()
                .map_or(("source-only", ""), |value| ("text", value.trim()));
            transaction
                .execute(
                    "INSERT INTO conversation (id, edition_id, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![conversation, edition, item.created_at, item.updated_at],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO message (id, conversation_id, ordinal, author_type, created_at_ms, updated_at_ms, deleted_at_ms)
                     VALUES (?1, ?2, 0, 'user', ?3, ?4, ?5)",
                    params![
                        message,
                        conversation,
                        item.created_at,
                        item.updated_at,
                        item.deleted_at
                    ],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO message_revision (id, message_id, schema_version, kind, content_json, plain_text, created_at_ms)
                     VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
                    params![
                        revision,
                        message,
                        kind,
                        serde_json::json!({ "schema": 1, "kind": kind, "text": text }).to_string(),
                        text,
                        item.updated_at
                    ],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO source_snapshot (id, fragment_html, reader_css, book_css, user_css, presentation_json, created_at_ms)
                     VALUES (?1, ?2, '', '', '', '{\"schema\":1,\"legacy\":true}', ?3)",
                    params![
                        snapshot,
                        format!("<blockquote>{}</blockquote>", escape_html(&item.anchor.selected_text)),
                        item.created_at
                    ],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO source_anchor (id, message_id, snapshot_id, original_locator_json, current_locator_json, section_id, selected_text, prefix_text, suffix_text, content_hash, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        source,
                        message,
                        snapshot,
                        item.anchor.canonical_locator,
                        item.anchor.section,
                        item.anchor.selected_text,
                        item.anchor.prefix_text,
                        item.anchor.suffix_text,
                        content_hash,
                        item.created_at
                    ],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "UPDATE message SET current_revision_id = ?2, current_source_anchor_id = ?3 WHERE id = ?1",
                    params![message, revision, source],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "UPDATE conversation SET root_message_id = ?2 WHERE id = ?1",
                    params![conversation, message],
                )
                .map_err(|_| MessageError::Database)?;
            if item.deleted_at.is_none() {
                transaction
                    .execute(
                        "INSERT INTO message_search (message_id, conversation_id, edition_id, section_id, selected_text, plain_text)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            encode_hex(&message),
                            encode_hex(&conversation),
                            encode_hex(&edition),
                            item.anchor.section,
                            item.anchor.selected_text,
                            text
                        ],
                    )
                    .map_err(|_| MessageError::Database)?;
            }
            transaction
                .execute(
                    "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms)
                     VALUES (?1, 'message', ?2, 'message-imported', ?3, ?4)",
                    params![
                        outbox,
                        message,
                        serde_json::json!({ "messageId": encode_hex(&message), "legacyId": item.id }).to_string(),
                        completed_at
                    ],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO legacy_annotation_import (edition_id, source_key, legacy_id, message_id)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![edition, input.source_key, item.id, message],
                )
                .map_err(|_| MessageError::Database)?;
            imported.push(LegacyImportedItem {
                legacy_id: item.id.clone(),
                conversation_id: encode_hex(&conversation),
                message_id: encode_hex(&message),
            });
        }
        transaction.commit().map_err(|_| MessageError::Database)?;
        Ok(LegacyImportResult {
            imported: imported.len(),
            already_complete: false,
            record_hash: input.record_hash,
            items: imported,
        })
    }
}

fn validate_legacy_import(input: &LegacyImport) -> Result<(), MessageError> {
    validate_edition(&input.edition)?;
    decode_hex::<32>(&input.record_hash)?;
    if input.source_key.is_empty()
        || input.source_key.len() > 200
        || input.source_key.chars().any(char::is_control)
        || input.items.len() > 1_000
    {
        return Err(MessageError::InvalidInput);
    }
    let mut ids = HashSet::with_capacity(input.items.len());
    for item in &input.items {
        let snapshot = legacy_snapshot(&item.anchor);
        validate_root(&RootMessageDraft {
            edition: input.edition.clone(),
            anchor: item.anchor.clone(),
            snapshot,
            text: item.note.clone(),
        })?;
        if item.id.is_empty()
            || item.id.len() > 64
            || !item
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || !ids.insert(item.id.as_str())
            || item.created_at < 0
            || item.updated_at < item.created_at
            || item
                .deleted_at
                .is_some_and(|deleted| deleted < item.updated_at)
        {
            return Err(MessageError::InvalidInput);
        }
    }
    Ok(())
}

fn legacy_snapshot(anchor: &SourceAnchorInput) -> SourceSnapshotInput {
    SourceSnapshotInput {
        fragment_html: format!(
            "<blockquote>{}</blockquote>",
            escape_html(&anchor.selected_text)
        ),
        reader_css: String::new(),
        book_css: String::new(),
        user_css: String::new(),
        presentation_json: r#"{"schema":1,"legacy":true}"#.into(),
        resources: Vec::new(),
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn ensure_edition(
    transaction: &rusqlite::Transaction<'_>,
    edition_id: &[u8],
    edition: &EditionInput,
) -> Result<(), MessageError> {
    let now = now_millis()?;
    let authors =
        serde_json::to_string(&edition.authors).map_err(|_| MessageError::InvalidInput)?;
    let work_id = transaction
        .query_row(
            "SELECT work_id FROM edition WHERE id = ?1",
            params![edition_id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(|_| MessageError::Database)?;
    let work_id = match work_id {
        Some(work_id) => work_id,
        None => {
            let work_id = random_id(transaction)?;
            transaction
                .execute(
                    "INSERT INTO work (id, title, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?3)",
                    params![work_id, edition.title, now],
                )
                .map_err(|_| MessageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO edition (id, work_id, title, authors_json, imported_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![edition_id, work_id, edition.title, authors, now],
                )
                .map_err(|_| MessageError::Database)?;
            work_id
        }
    };
    transaction
        .execute(
            "UPDATE edition SET title = ?2, authors_json = ?3 WHERE id = ?1",
            params![edition_id, edition.title, authors],
        )
        .map_err(|_| MessageError::Database)?;
    transaction
        .execute(
            "UPDATE work SET title = ?2, updated_at_ms = ?3 WHERE id = ?1",
            params![work_id, edition.title, now],
        )
        .map_err(|_| MessageError::Database)?;
    Ok(())
}

fn imported_items(
    transaction: &rusqlite::Transaction<'_>,
    edition: &[u8],
    source_key: &str,
) -> Result<Vec<LegacyImportedItem>, MessageError> {
    let mut statement = transaction
        .prepare(
            "SELECT i.legacy_id, m.conversation_id, i.message_id
             FROM legacy_annotation_import i
             JOIN message m ON m.id = i.message_id
             WHERE i.edition_id = ?1 AND i.source_key = ?2
             ORDER BY i.rowid",
        )
        .map_err(|_| MessageError::Database)?;
    statement
        .query_map(params![edition, source_key], |row| {
            let conversation: Vec<u8> = row.get(1)?;
            let message: Vec<u8> = row.get(2)?;
            Ok(LegacyImportedItem {
                legacy_id: row.get(0)?,
                conversation_id: encode_hex(&conversation),
                message_id: encode_hex(&message),
            })
        })
        .map_err(|_| MessageError::Database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::Database)
}
