use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{
    model::*,
    store::MessageStore,
    util::{decode_hex, encode_hex, now_millis, random_id},
};

impl MessageStore {
    pub fn create_root(&self, draft: RootMessageDraft) -> Result<CreatedRoot, MessageError> {
        validate_root(&draft)?;
        let edition_id = decode_hex::<32>(&draft.edition.content_version)?;
        let content_hash = decode_hex::<32>(&draft.anchor.content_hash)?;
        let now = now_millis()?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let resources = self.prepare_resources(&transaction, &draft.snapshot.resources)?;
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

    pub fn reply(&self, draft: ReplyDraft) -> Result<CreatedMessage, MessageError> {
        let content = match draft.rich_text.as_ref() {
            Some(rich_text) => rich_message_content(rich_text)?,
            None => plain_message_content(Some(&draft.text))?,
        };
        if draft.reference_ids.len() > 32 {
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
                    content.content_json,
                    content.plain_text,
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
        let resources = self.prepare_resources(&transaction, &draft.snapshot.resources)?;
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
        transaction
            .execute(
                "UPDATE conversation SET updated_at_ms = ?2 WHERE id = (SELECT conversation_id FROM message WHERE id = ?1)",
                params![message, now],
            )
            .map_err(|_| MessageError::Database)?;
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

    pub fn reanchor_source(
        &self,
        source_id: &str,
        expected_locator: &str,
        current_locator: &str,
    ) -> Result<(), MessageError> {
        let source = decode_hex::<16>(source_id)?;
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| MessageError::Database)?;
        let (stored_locator, section, selected_text, message): (String, String, String, Vec<u8>) =
            transaction
                .query_row(
                    "SELECT a.current_locator_json, a.section_id, a.selected_text, a.message_id
                     FROM source_anchor a JOIN message m ON m.current_source_anchor_id = a.id
                     WHERE a.id = ?1 AND m.deleted_at_ms IS NULL",
                    params![source],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => MessageError::UnknownMessage,
                    _ => MessageError::Database,
                })?;
        if stored_locator != expected_locator {
            return Err(MessageError::RevisionConflict);
        }
        let selected_length = selected_text.encode_utf16().count();
        validate_range_locator(expected_locator, &section, selected_length)?;
        validate_range_locator(current_locator, &section, selected_length)?;
        let now = now_millis()?;
        let outbox = random_id(&transaction)?;
        let changed = transaction
            .execute(
                "UPDATE source_anchor SET current_locator_json = ?2
                 WHERE id = ?1 AND current_locator_json = ?3
                   AND EXISTS(SELECT 1 FROM message m WHERE m.id = ?4 AND m.current_source_anchor_id = ?1)",
                params![source, current_locator, expected_locator, message],
            )
            .map_err(|_| MessageError::Database)?;
        if changed != 1 {
            return Err(MessageError::RevisionConflict);
        }
        transaction
            .execute(
                "INSERT INTO outbox_event (id, aggregate_type, aggregate_id, event_type, payload_json, created_at_ms)
                 VALUES (?1, 'message', ?2, 'message-source-reanchored', ?3, ?4)",
                params![
                    outbox,
                    message,
                    serde_json::json!({ "messageId": encode_hex(&message), "sourceId": source_id }).to_string(),
                    now
                ],
            )
            .map_err(|_| MessageError::Database)?;
        transaction.commit().map_err(|_| MessageError::Database)
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
                "UPDATE conversation SET updated_at_ms = ?2 WHERE id = (SELECT conversation_id FROM message WHERE id = ?1)",
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
        self.revise_content(
            message_id,
            expected_revision_id,
            plain_message_content(text)?,
        )
    }

    pub fn revise_rich(
        &self,
        message_id: &str,
        expected_revision_id: &str,
        rich_text: RichTextInput,
    ) -> Result<CreatedRevision, MessageError> {
        self.revise_content(
            message_id,
            expected_revision_id,
            rich_message_content(&rich_text)?,
        )
    }

    fn revise_content(
        &self,
        message_id: &str,
        expected_revision_id: &str,
        content: ValidatedMessageContent,
    ) -> Result<CreatedRevision, MessageError> {
        let message = decode_hex::<16>(message_id)?;
        let expected = decode_hex::<16>(expected_revision_id)?;
        let kind = if content.plain_text.is_empty() {
            "source-only"
        } else {
            "text"
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
        transaction
            .execute(
                "INSERT INTO message_revision (id, message_id, schema_version, kind, content_json, plain_text, created_at_ms) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)",
                params![revision, message, kind, content.content_json, content.plain_text, now],
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
        transaction
            .execute(
                "UPDATE conversation SET updated_at_ms = ?2 WHERE id = (SELECT conversation_id FROM message WHERE id = ?1)",
                params![message, now],
            )
            .map_err(|_| MessageError::Database)?;
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
