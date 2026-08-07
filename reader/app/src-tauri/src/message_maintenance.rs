use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::ReaderRuntime;

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
    let path = selected.into_path().map_err(|_| "message-backup")?;
    let messages = runtime.messages.clone();
    tauri::async_runtime::spawn_blocking(move || messages.create_backup(path))
        .await
        .map_err(|_| "message-backup-task".to_owned())?
        .map_err(|error| error.code().to_owned())?;
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
    let path = selected.into_path().map_err(|_| "message-restore")?;
    let messages = runtime.messages.clone();
    tauri::async_runtime::spawn_blocking(move || messages.restore_backup(path))
        .await
        .map_err(|_| "message-restore-task".to_owned())?
        .map_err(|error| error.code().to_owned())?;
    Ok(true)
}

pub(crate) fn is_library_url(url: &str) -> bool {
    url == "https://tauri.localhost/"
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
        assert!(is_library_url("https://tauri.localhost/"));
        assert!(!is_library_url("https://tauri.localhost/index.html"));
        assert!(!is_library_url("https://example.com/"));
    }
}
