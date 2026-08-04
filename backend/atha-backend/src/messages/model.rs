use std::{collections::HashSet, error::Error, fmt};

use dom_query::Document;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::util::decode_hex;

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotPresentation {
    schema: u8,
    theme: String,
    brightness: u8,
    font_size: u8,
    font_family: String,
    density: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacySnapshotPresentation {
    schema: u8,
    legacy: bool,
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
pub struct RootMessageView {
    pub conversation_id: String,
    pub message_id: String,
    pub revision_id: String,
    pub kind: String,
    pub text: String,
    pub source: MessageSourceView,
    pub updated_at: i64,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInspection {
    pub edition_id: String,
    pub conversations: usize,
    pub messages: usize,
    pub revisions: usize,
    pub sources: usize,
    pub snapshots: usize,
    pub relationships: usize,
    pub resources: usize,
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
    UnknownEdition,
    UnknownMessage,
    RevisionConflict,
    FutureDatabase,
    LegacyConflict,
    CorruptData,
    Export,
    InvalidExport,
    Database,
}

impl MessageError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid-message-root",
            Self::InvalidInput => "invalid-message-input",
            Self::UnknownConversation => "unknown-conversation",
            Self::UnknownEdition => "unknown-edition",
            Self::UnknownMessage => "unknown-message",
            Self::RevisionConflict => "message-revision-conflict",
            Self::FutureDatabase => "future-message-database",
            Self::LegacyConflict => "legacy-message-conflict",
            Self::CorruptData => "corrupt-message-data",
            Self::Export => "message-export",
            Self::InvalidExport => "invalid-message-export",
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
    validate_source(&draft.anchor, &draft.snapshot)?;
    if locator_content_version(&draft.anchor.canonical_locator)? != draft.edition.content_version {
        return Err(MessageError::InvalidInput);
    }
    Ok(())
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
    if validate_range_locator(
        &anchor.canonical_locator,
        &anchor.section,
        anchor.selected_text.encode_utf16().count(),
    )
    .is_err()
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
        || snapshot.presentation_json.len() > 4_096
        || validate_snapshot_presentation(&snapshot.presentation_json).is_err()
    {
        return Err(MessageError::InvalidInput);
    }
    if Sha256::digest(anchor.selected_text.as_bytes()).as_slice() != content_hash {
        return Err(MessageError::InvalidInput);
    }
    validate_snapshot_markup(
        &snapshot.fragment_html,
        &anchor.selected_text,
        &snapshot.resources,
    )?;
    validate_snapshot_css(&snapshot.reader_css)?;
    validate_snapshot_css(&snapshot.book_css)?;
    validate_snapshot_css(&snapshot.user_css)?;
    validate_resources(&snapshot.resources)
}

fn validate_snapshot_presentation(value: &str) -> Result<(), MessageError> {
    if let Ok(legacy) = serde_json::from_str::<LegacySnapshotPresentation>(value) {
        return if legacy.schema == 1 && legacy.legacy {
            Ok(())
        } else {
            Err(MessageError::InvalidInput)
        };
    }
    let presentation = serde_json::from_str::<SnapshotPresentation>(value)
        .map_err(|_| MessageError::InvalidInput)?;
    if presentation.schema != 1
        || !matches!(presentation.theme.as_str(), "light" | "paper" | "dark")
        || !(70..=120).contains(&presentation.brightness)
        || !matches!(presentation.font_size, 24 | 32 | 40)
        || !matches!(presentation.font_family.as_str(), "book" | "serif" | "sans")
        || !matches!(
            presentation.density.as_str(),
            "compact" | "standard" | "comfortable"
        )
    {
        return Err(MessageError::InvalidInput);
    }
    Ok(())
}

fn locator_content_version(value: &str) -> Result<String, MessageError> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|locator| locator.get("contentVersion")?.as_str().map(str::to_owned))
        .ok_or(MessageError::InvalidInput)
}

fn validate_snapshot_markup(
    fragment: &str,
    selected_text: &str,
    resources: &[SnapshotResourceInput],
) -> Result<(), MessageError> {
    let document = Document::fragment(fragment);
    if !document.errors.borrow().is_empty()
        || document.text().as_ref() != selected_text
        || document
            .try_select(
                "script,iframe,object,embed,form,input,button,select,textarea,video,audio,source,track,style,link,meta,base",
            )
            .is_some()
    {
        return Err(MessageError::InvalidInput);
    }
    let declared = resources
        .iter()
        .map(|resource| resource.path.clone())
        .collect::<HashSet<_>>();
    let mut referenced = HashSet::new();
    for node in document
        .root()
        .descendants()
        .into_iter()
        .filter(|node| node.is_element())
    {
        let name = node.node_name().ok_or(MessageError::InvalidInput)?;
        for attribute in node.attrs() {
            let attribute_name = attribute.name.local.as_ref();
            if attribute_name.starts_with("on")
                || matches!(attribute_name, "style" | "srcset")
                || attribute_name == "href" && name.as_ref() != "image"
                || attribute_name == "src" && name.as_ref() != "img"
            {
                return Err(MessageError::InvalidInput);
            }
            if matches!(
                (name.as_ref(), attribute_name),
                ("img", "src") | ("image", "href")
            ) {
                let path = attribute.value.as_ref();
                validate_resource_path(path)?;
                if !declared.contains(path) {
                    return Err(MessageError::InvalidInput);
                }
                referenced.insert(path.to_owned());
            }
        }
    }
    if referenced == declared {
        Ok(())
    } else {
        Err(MessageError::InvalidInput)
    }
}

fn validate_snapshot_css(css: &str) -> Result<(), MessageError> {
    let lower = css.to_ascii_lowercase();
    if lower.contains("@import")
        || contains_css_function(&lower, "url")
        || contains_css_function(&lower, "image-set")
        || lower.contains(":host")
        || lower.contains("::part")
        || lower.contains("::slotted")
        || lower.contains("javascript:")
        || lower.contains("data:")
        || lower.contains("http:")
        || lower.contains("https:")
        || lower.contains("//")
        || css.contains('\\')
    {
        Err(MessageError::InvalidInput)
    } else {
        Ok(())
    }
}

fn contains_css_function(css: &str, name: &str) -> bool {
    css.match_indices(name)
        .any(|(index, _)| css[index + name.len()..].trim_start().starts_with('('))
}

pub(crate) fn validate_range_locator(
    value: &str,
    expected_section: &str,
    expected_length: usize,
) -> Result<(), MessageError> {
    if value.is_empty() || value.len() > 2048 {
        return Err(MessageError::InvalidInput);
    }
    let locator =
        serde_json::from_str::<serde_json::Value>(value).map_err(|_| MessageError::InvalidInput)?;
    let object = locator.as_object().ok_or(MessageError::InvalidInput)?;
    if object.len() != 4
        || object.get("schema").and_then(serde_json::Value::as_u64) != Some(1)
        || object
            .get("contentVersion")
            .and_then(serde_json::Value::as_str)
            .is_none_or(|version| decode_hex::<32>(version).is_err())
    {
        return Err(MessageError::InvalidInput);
    }
    let start = locator_point(object.get("start"))?;
    let end = locator_point(object.get("end"))?;
    if start.0 != expected_section
        || end.0 != start.0
        || end.1 < start.1
        || usize::try_from(end.1 - start.1).ok() != Some(expected_length)
    {
        return Err(MessageError::InvalidInput);
    }
    Ok(())
}

fn locator_point(value: Option<&serde_json::Value>) -> Result<(&str, i64), MessageError> {
    let object = value
        .and_then(serde_json::Value::as_object)
        .ok_or(MessageError::InvalidInput)?;
    if object.len() != 2 {
        return Err(MessageError::InvalidInput);
    }
    let section = object
        .get("section")
        .and_then(serde_json::Value::as_str)
        .filter(|section| {
            !section.is_empty()
                && section.len() <= 64
                && section.bytes().enumerate().all(|(index, byte)| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || index > 0 && matches!(byte, b'.' | b'_' | b'-')
                })
        })
        .ok_or(MessageError::InvalidInput)?;
    let offset = object
        .get("offset")
        .and_then(serde_json::Value::as_i64)
        .filter(|offset| (0..=i64::from(i32::MAX)).contains(offset))
        .ok_or(MessageError::InvalidInput)?;
    Ok((section, offset))
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
