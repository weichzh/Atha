use std::time::Instant;

use atha_backend::messages::{
    ConversationView, CreatedMessage, CreatedRevision, CreatedRoot, CreatedSource, EditionInput,
    LegacyImport, LegacyImportResult, MessageError, MessageRelationships, MessageSearch,
    MessageSearchHit, ReadingMemoryHit, ReplyDraft, ReselectDraft, RevisionView, RichTextInput,
    RootMessageDraft, RootMessageView, SnapshotResourceData, SourceCaptureView,
};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::{
    ReaderRuntime, begin_local_data_operation, is_reader_url,
    message_maintenance::require_library_window, platform_file::PickerOutput,
};

#[tauri::command]
pub(crate) async fn reading_memory_search(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    query: String,
) -> Result<Vec<ReadingMemoryHit>, String> {
    require_library_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .reading_memory_search(&query)
        .map_err(message_error("reading-memory-search"))
}

#[tauri::command]
pub(crate) async fn reading_memory_source_captures(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    root_message_id: String,
) -> Result<Vec<SourceCaptureView>, String> {
    require_library_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .source_captures(&root_message_id)
        .map_err(message_error("reading-memory-source-captures"))
}

#[tauri::command]
pub(crate) async fn reading_memory_snapshot_resource(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    source_id: String,
    source_path: String,
) -> Result<SnapshotResourceData, String> {
    require_library_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .read_snapshot_resource(&source_id, &source_path)
        .map_err(message_error("reading-memory-snapshot-resource"))
}

#[tauri::command]
pub(crate) async fn message_roots(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    edition_id: String,
    section: Option<String>,
) -> Result<Vec<RootMessageView>, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .roots(&edition_id, section.as_deref())
        .map_err(message_error("roots"))
}

#[tauri::command]
pub(crate) async fn message_edition_context(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    content_version: String,
) -> Result<EditionInput, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    let current = runtime
        .current_edition
        .read()
        .map_err(|_| message_state_error("edition-context"))?;
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
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .conversation(&conversation_id)
        .map_err(message_error("conversation"))
}

#[tauri::command]
pub(crate) async fn message_conversations(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    edition_id: String,
    section: Option<String>,
) -> Result<Vec<ConversationView>, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .conversations(&edition_id, section.as_deref())
        .map_err(message_error("conversations"))
}

#[tauri::command]
pub(crate) async fn message_create_root(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: RootMessageDraft,
) -> Result<CreatedRoot, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .create_root(draft)
        .map_err(message_error("create-root"))
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
    let _operation = begin_local_data_operation(&runtime)?;
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
    .map_err(message_error("revise"))
}

#[tauri::command]
pub(crate) async fn message_reply(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: ReplyDraft,
) -> Result<CreatedMessage, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .reply(draft)
        .map_err(message_error("reply"))
}

#[tauri::command]
pub(crate) async fn message_delete(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
    expected_revision_id: String,
) -> Result<(), String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .delete(&message_id, &expected_revision_id)
        .map_err(message_error("delete"))
}

#[tauri::command]
pub(crate) async fn message_search(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    search: MessageSearch,
) -> Result<Vec<MessageSearchHit>, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .search(search)
        .map_err(message_error("search"))
}

#[tauri::command]
pub(crate) async fn message_relationships(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
) -> Result<MessageRelationships, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .relationships(&message_id)
        .map_err(message_error("relationships"))
}

#[tauri::command]
pub(crate) async fn message_revisions(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
) -> Result<Vec<RevisionView>, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .revisions(&message_id)
        .map_err(message_error("revisions"))
}

#[tauri::command]
pub(crate) async fn message_source_captures(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
) -> Result<Vec<SourceCaptureView>, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .source_captures(&message_id)
        .map_err(message_error("source-captures"))
}

#[tauri::command]
pub(crate) async fn message_snapshot_resource(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    source_id: String,
    source_path: String,
) -> Result<SnapshotResourceData, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .read_snapshot_resource(&source_id, &source_path)
        .map_err(message_error("snapshot-resource"))
}

#[tauri::command]
pub(crate) async fn message_reselect(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: ReselectDraft,
) -> Result<CreatedSource, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .reselect(draft)
        .map_err(message_error("reselect"))
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
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .reanchor_source(&source_id, &expected_locator, &current_locator)
        .map_err(message_error("reanchor"))
}

#[tauri::command]
pub(crate) async fn message_import_legacy(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    input: LegacyImport,
) -> Result<LegacyImportResult, String> {
    require_reader_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    runtime
        .messages
        .import_legacy_annotations(input)
        .map_err(message_error("import-legacy"))
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
    let _operation = begin_local_data_operation(&runtime)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("Atha 消息归档", &["zip"])
        .set_file_name("Atha-消息归档.zip")
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let started = Instant::now();
    let messages = runtime.messages.clone();
    let result: Result<(), (&'static str, String)> =
        tauri::async_runtime::spawn_blocking(move || {
            let output = PickerOutput::new(&app, selected, "zip")
                .map_err(|_| ("picker-prepare", "message-export".to_owned()))?;
            match conversation_id {
                Some(conversation) => messages.export_conversation(&conversation, output.path()),
                None => messages.export_edition(&edition_id, output.path()),
            }
            .map_err(|error| ("backend", error.code().to_owned()))?;
            output
                .commit()
                .map_err(|_| ("picker-commit", "message-export".to_owned()))
        })
        .await
        .map_err(|_| ("task", "message-export".to_owned()))
        .and_then(|result| result);
    match &result {
        Ok(()) => log::info!(
            target: "atha::messages",
            "operation=export outcome=success duration_ms={}",
            started.elapsed().as_millis()
        ),
        Err((stage, code)) => log::warn!(
            target: "atha::messages",
            "operation=export stage={stage} outcome=failed code={} duration_ms={}",
            code,
            started.elapsed().as_millis()
        ),
    }
    result.map_err(|(_, code)| code)?;
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

fn message_error(operation: &'static str) -> impl FnOnce(MessageError) -> String {
    move |error| {
        let code = error.code();
        if is_internal_message_error(error) {
            log::error!(
                target: "atha::messages",
                "operation={operation} outcome=failed code={code}"
            );
        }
        code.into()
    }
}

fn message_state_error(operation: &'static str) -> String {
    log::error!(
        target: "atha::messages",
        "operation={operation} outcome=failed code=reader-state"
    );
    "reader-state".into()
}

const fn is_internal_message_error(error: MessageError) -> bool {
    matches!(
        error,
        MessageError::InvalidRoot
            | MessageError::FutureDatabase
            | MessageError::CorruptData
            | MessageError::Database
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_internal_message_failures_are_log_worthy() {
        for error in [
            MessageError::InvalidRoot,
            MessageError::FutureDatabase,
            MessageError::CorruptData,
            MessageError::Database,
        ] {
            assert!(is_internal_message_error(error));
        }
        for error in [
            MessageError::InvalidInput,
            MessageError::UnknownConversation,
            MessageError::UnknownEdition,
            MessageError::UnknownMessage,
            MessageError::RevisionConflict,
            MessageError::LegacyConflict,
            MessageError::Backup,
            MessageError::InvalidBackup,
            MessageError::Restore,
            MessageError::Export,
            MessageError::InvalidExport,
        ] {
            assert!(!is_internal_message_error(error));
        }
    }
}
