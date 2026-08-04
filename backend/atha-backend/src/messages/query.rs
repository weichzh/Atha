use std::fs;

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

use super::{
    model::*,
    store::MessageStore,
    util::{decode_hex, encode_hex},
};

impl MessageStore {
    pub fn roots(
        &self,
        edition_id: &str,
        section: Option<&str>,
    ) -> Result<Vec<RootMessageView>, MessageError> {
        let edition = decode_hex::<32>(edition_id)?;
        if section.is_some_and(|value| value.is_empty() || value.len() > 256) {
            return Err(MessageError::InvalidInput);
        }
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT c.id, m.id, r.id, r.kind, r.plain_text,
                        a.id, a.original_locator_json, a.current_locator_json, a.section_id,
                        a.selected_text, a.prefix_text, a.suffix_text, a.content_hash, c.updated_at_ms
                 FROM conversation c
                 JOIN message m ON m.id = c.root_message_id AND m.conversation_id = c.id
                 JOIN message_revision r ON r.id = m.current_revision_id AND r.message_id = m.id
                 JOIN source_anchor a ON a.id = m.current_source_anchor_id AND a.message_id = m.id
                 WHERE c.edition_id = ?1 AND m.deleted_at_ms IS NULL
                   AND (?2 IS NULL OR a.section_id = ?2)
                 ORDER BY c.updated_at_ms DESC, m.id LIMIT 1000",
            )
            .map_err(|_| MessageError::Database)?;
        statement
            .query_map(params![edition, section], |row| {
                let conversation: Vec<u8> = row.get(0)?;
                let message: Vec<u8> = row.get(1)?;
                let revision: Vec<u8> = row.get(2)?;
                let source: Vec<u8> = row.get(5)?;
                let content_hash: Vec<u8> = row.get(12)?;
                Ok(RootMessageView {
                    conversation_id: encode_hex(&conversation),
                    message_id: encode_hex(&message),
                    revision_id: encode_hex(&revision),
                    kind: row.get(3)?,
                    text: row.get(4)?,
                    source: MessageSourceView {
                        id: encode_hex(&source),
                        original_locator: row.get(6)?,
                        canonical_locator: row.get(7)?,
                        section: row.get(8)?,
                        selected_text: row.get(9)?,
                        prefix_text: row.get(10)?,
                        suffix_text: row.get(11)?,
                        content_hash: encode_hex(&content_hash),
                    },
                    updated_at: row.get(13)?,
                })
            })
            .map_err(|_| MessageError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MessageError::Database)
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
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
