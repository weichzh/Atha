use std::{collections::HashSet, error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::util::decode_hex;

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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyAnnotationInput {
    pub id: String,
    pub anchor: SourceAnchorInput,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImport {
    pub edition: EditionInput,
    pub source_key: String,
    pub record_hash: String,
    pub items: Vec<LegacyAnnotationInput>,
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
pub struct LegacyImportedItem {
    pub legacy_id: String,
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportResult {
    pub imported: usize,
    pub already_complete: bool,
    pub record_hash: String,
    pub items: Vec<LegacyImportedItem>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreHealth {
    pub schema_version: i64,
    pub sqlite_version: String,
    pub foreign_keys: bool,
    pub fts5: bool,
    pub integrity: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedResource {
    pub(crate) path: String,
    pub(crate) media_type: String,
    pub(crate) content_hash: Vec<u8>,
    pub(crate) byte_length: i64,
    pub(crate) asset_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageError {
    InvalidRoot,
    InvalidInput,
    UnknownConversation,
    UnknownMessage,
    RevisionConflict,
    FutureDatabase,
    LegacyConflict,
    CorruptData,
    Database,
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
            Self::LegacyConflict => "legacy-message-conflict",
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

pub(crate) fn validate_root(draft: &RootMessageDraft) -> Result<(), MessageError> {
    validate_edition(&draft.edition)?;
    if draft
        .text
        .as_ref()
        .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 8_000)
    {
        return Err(MessageError::InvalidInput);
    }
    validate_source(&draft.anchor, &draft.snapshot)
}

pub(crate) fn validate_edition(edition: &EditionInput) -> Result<(), MessageError> {
    decode_hex::<32>(&edition.content_version)?;
    if edition.title.trim().is_empty()
        || edition.title.chars().count() > 512
        || edition.authors.len() > 16
        || edition
            .authors
            .iter()
            .any(|value| value.trim().is_empty() || value.chars().count() > 512)
    {
        return Err(MessageError::InvalidInput);
    }
    Ok(())
}

pub(crate) fn validate_source(
    anchor: &SourceAnchorInput,
    snapshot: &SourceSnapshotInput,
) -> Result<(), MessageError> {
    let content_hash = decode_hex::<32>(&anchor.content_hash)?;
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
    if Sha256::digest(anchor.selected_text.as_bytes()).as_slice() != content_hash {
        return Err(MessageError::InvalidInput);
    }
    validate_resources(&snapshot.resources)
}

pub(crate) fn validate_resources(resources: &[SnapshotResourceInput]) -> Result<(), MessageError> {
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

pub(crate) fn validate_resource_path(path: &str) -> Result<(), MessageError> {
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
