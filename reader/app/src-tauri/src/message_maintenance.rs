use std::time::Instant;

use atha_backend::messages::MAX_BACKUP_BYTES;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::{
    ReaderRuntime,
    platform_file::{PickerInput, PickerOutput},
};

#[tauri::command]
pub(crate) async fn backup_message_store(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<bool, String> {
    require_library_window(&window)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("Atha 消息备份", &["atha-backup"])
        .set_file_name("Atha-消息备份.atha-backup")
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let started = Instant::now();
    let messages = runtime.messages.clone();
    let result: Result<(), (&'static str, String)> =
        tauri::async_runtime::spawn_blocking(move || {
            let output = PickerOutput::new(&app, selected, "atha-backup")
                .map_err(|_| ("picker-prepare", "message-backup".to_owned()))?;
            messages
                .create_backup(output.path())
                .map_err(|error| ("backend", error.code().to_owned()))?;
            output
                .commit()
                .map_err(|_| ("picker-commit", "message-backup".to_owned()))
        })
        .await
        .map_err(|_| ("task", "message-backup-task".to_owned()))
        .and_then(|result| result);
    log_maintenance("backup", &result, started);
    result.map_err(|(_, code)| code)?;
    Ok(true)
}

#[tauri::command]
pub(crate) async fn restore_message_store(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<bool, String> {
    require_library_window(&window)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("Atha 消息备份", &["atha-backup"])
        .blocking_pick_file()
    else {
        return Ok(false);
    };
    let started = Instant::now();
    let messages = runtime.messages.clone();
    let result: Result<(), (&'static str, String)> =
        tauri::async_runtime::spawn_blocking(move || {
            let input = PickerInput::open(&app, selected, "atha-backup", MAX_BACKUP_BYTES)
                .map_err(|_| ("picker-prepare", "message-restore".to_owned()))?;
            messages
                .restore_backup(input.path())
                .map_err(|error| ("backend", error.code().to_owned()))
        })
        .await
        .map_err(|_| ("task", "message-restore-task".to_owned()))
        .and_then(|result| result);
    log_maintenance("restore", &result, started);
    result.map_err(|(_, code)| code)?;
    Ok(true)
}

fn log_maintenance(operation: &str, result: &Result<(), (&'static str, String)>, started: Instant) {
    match result {
        Ok(()) => log::info!(
            target: "atha::messages",
            "operation={operation} outcome=success duration_ms={}",
            started.elapsed().as_millis()
        ),
        Err((stage, code)) => log::warn!(
            target: "atha::messages",
            "operation={operation} stage={stage} outcome=failed code={} duration_ms={}",
            code,
            started.elapsed().as_millis()
        ),
    }
}

pub(crate) fn is_library_url(url: &str) -> bool {
    url == crate::TAURI_LIBRARY_PAGE
}

fn require_library_window(window: &WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|_| "reader-url")?;
    if window.label() == "main" && is_library_url(url.as_str()) {
        Ok(())
    } else {
        Err("invalid-origin".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_library_root_can_run_maintenance_commands() {
        assert!(is_library_url(crate::TAURI_LIBRARY_PAGE));
        assert!(!is_library_url(&format!(
            "{}/index.html",
            crate::TAURI_LIBRARY_PAGE.trim_end_matches('/')
        )));
        assert!(!is_library_url("https://example.com/"));
    }
}
