use atha_backend::messages::{
    ConversationView, CreatedMessage, CreatedRevision, CreatedRoot, CreatedSource, EditionInput,
    LegacyImport, LegacyImportResult, MessageError, MessageRelationships, MessageSearch,
    MessageSearchHit, ReplyDraft, ReselectDraft, RevisionView, RichTextInput, RootMessageDraft,
    RootMessageView, SnapshotResourceData, SourceCaptureView,
};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::{ReaderRuntime, is_reader_url};

#[tauri::command]
pub(crate) async fn message_roots(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    edition_id: String,
    section: Option<String>,
) -> Result<Vec<RootMessageView>, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .roots(&edition_id, section.as_deref())
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_edition_context(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    content_version: String,
) -> Result<EditionInput, String> {
    require_reader_window(&window)?;
    let current = runtime.current_edition.read().map_err(|_| "reader-state")?;
    Ok(current
        .as_ref()
        .filter(|edition| edition.content_version == content_version)
        .cloned()
        .unwrap_or(EditionInput {
            content_version,
            title: "未命名书籍".into(),
            authors: Vec::new(),
        }))
}

#[tauri::command]
pub(crate) async fn message_conversation(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    conversation_id: String,
) -> Result<ConversationView, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .conversation(&conversation_id)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_conversations(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    edition_id: String,
    section: Option<String>,
) -> Result<Vec<ConversationView>, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .conversations(&edition_id, section.as_deref())
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_create_root(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: RootMessageDraft,
) -> Result<CreatedRoot, String> {
    require_reader_window(&window)?;
    runtime.messages.create_root(draft).map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_revise(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
    expected_revision_id: String,
    text: Option<String>,
    rich_text: Option<RichTextInput>,
) -> Result<CreatedRevision, String> {
    require_reader_window(&window)?;
    match rich_text {
        Some(rich_text) => {
            runtime
                .messages
                .revise_rich(&message_id, &expected_revision_id, rich_text)
        }
        None => runtime
            .messages
            .revise(&message_id, &expected_revision_id, text.as_deref()),
    }
    .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_reply(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: ReplyDraft,
) -> Result<CreatedMessage, String> {
    require_reader_window(&window)?;
    runtime.messages.reply(draft).map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_delete(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
    expected_revision_id: String,
) -> Result<(), String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .delete(&message_id, &expected_revision_id)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_search(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    search: MessageSearch,
) -> Result<Vec<MessageSearchHit>, String> {
    require_reader_window(&window)?;
    runtime.messages.search(search).map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_relationships(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
) -> Result<MessageRelationships, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .relationships(&message_id)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_revisions(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
) -> Result<Vec<RevisionView>, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .revisions(&message_id)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_source_captures(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
) -> Result<Vec<SourceCaptureView>, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .source_captures(&message_id)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_snapshot_resource(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    source_id: String,
    source_path: String,
) -> Result<SnapshotResourceData, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .read_snapshot_resource(&source_id, &source_path)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_reselect(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: ReselectDraft,
) -> Result<CreatedSource, String> {
    require_reader_window(&window)?;
    runtime.messages.reselect(draft).map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_reanchor(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    source_id: String,
    expected_locator: String,
    current_locator: String,
) -> Result<(), String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .reanchor_source(&source_id, &expected_locator, &current_locator)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_import_legacy(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    input: LegacyImport,
) -> Result<LegacyImportResult, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .import_legacy_annotations(input)
        .map_err(message_error)
}

#[tauri::command]
pub(crate) async fn message_export(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    edition_id: String,
    conversation_id: Option<String>,
) -> Result<bool, String> {
    require_reader_window(&window)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("Atha 消息归档", &["zip"])
        .set_file_name("Atha-消息归档.zip")
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let path = selected.into_path().map_err(|_| "message-export")?;
    match conversation_id {
        Some(conversation) => runtime.messages.export_conversation(&conversation, path),
        None => runtime.messages.export_edition(&edition_id, path),
    }
    .map_err(message_error)?;
    Ok(true)
}

fn require_reader_window(window: &WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|_| "reader-url")?;
    if window.label() == "main" && is_reader_url(url.as_str()) {
        Ok(())
    } else {
        Err("invalid-origin".into())
    }
}

fn message_error(error: MessageError) -> String {
    error.code().into()
}
