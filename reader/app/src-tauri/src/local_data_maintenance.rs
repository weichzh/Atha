use std::time::Instant;

use atha_backend::local_data::{
    BrowserState, MAX_LOCAL_DATA_BYTES, PendingLocalDataRestore, StorageUsage,
};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::{
    ReaderRuntime,
    message_maintenance::require_library_window,
    platform_file::{PickerInput, PickerOutput},
    require_local_data_ready,
};

#[tauri::command]
pub(crate) async fn backup_local_data(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    browser_state: BrowserState,
) -> Result<bool, String> {
    require_library_window(&window)?;
    require_local_data_ready(&runtime)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("Atha 资料库", &["atha-data"])
        .set_file_name("Atha-资料库.atha-data")
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let started = Instant::now();
    let data = runtime.local_data.clone();
    let library = runtime.library.clone();
    let dictionaries = runtime.dictionaries.clone();
    let messages = runtime.messages.clone();
    let result: Result<(), (&'static str, String)> =
        tauri::async_runtime::spawn_blocking(move || {
            let output = PickerOutput::new(&app, selected, "atha-data")
                .map_err(|_| ("picker-prepare", "local-data-backup".to_owned()))?;
            data.create_backup(
                output.path(),
                &browser_state,
                &library,
                &dictionaries,
                &messages,
            )
            .map_err(|error| ("backend", error.code().to_owned()))?;
            output
                .commit()
                .map_err(|_| ("picker-commit", "local-data-backup".to_owned()))
        })
        .await
        .map_err(|_| ("task", "local-data-backup-task".to_owned()))
        .and_then(|result| result);
    log_maintenance("backup", &result, started);
    result.map_err(|(_, code)| code)?;
    Ok(true)
}

#[tauri::command]
pub(crate) async fn prepare_local_data_restore(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    previous_browser_state: BrowserState,
) -> Result<Option<PendingLocalDataRestore>, String> {
    require_library_window(&window)?;
    require_local_data_ready(&runtime)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("Atha 资料库", &["atha-data"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let started = Instant::now();
    let data = runtime.local_data.clone();
    let messages = runtime.messages.clone();
    let result: Result<PendingLocalDataRestore, (&'static str, String)> =
        tauri::async_runtime::spawn_blocking(move || {
            let input = PickerInput::open(&app, selected, "atha-data", MAX_LOCAL_DATA_BYTES)
                .map_err(|_| ("picker-prepare", "local-data-restore".to_owned()))?;
            data.prepare_restore(input.path(), &previous_browser_state, &messages)
                .map_err(|error| ("backend", error.code().to_owned()))
        })
        .await
        .map_err(|_| ("task", "local-data-restore-task".to_owned()))
        .and_then(|result| result);
    log_maintenance_result("restore-prepare", &result, started);
    result.map(Some).map_err(|(_, code)| code)
}

#[tauri::command]
pub(crate) async fn commit_local_data_restore(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    token: String,
) -> Result<PendingLocalDataRestore, String> {
    require_library_window(&window)?;
    let started = Instant::now();
    let data = runtime.local_data.clone();
    let messages = runtime.messages.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        data.commit_restore(&token, &messages)
            .map_err(|error| ("backend", error.code().to_owned()))
    })
    .await
    .map_err(|_| ("task", "local-data-restore-task".to_owned()))
    .and_then(|result| result);
    log_maintenance_result("restore-commit", &result, started);
    let pending = result.map_err(|(_, code)| code)?;
    reset_reader(&runtime)?;
    Ok(pending)
}

#[tauri::command]
pub(crate) fn pending_local_data_restore(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Option<PendingLocalDataRestore>, String> {
    require_library_window(&window)?;
    runtime
        .local_data
        .pending_restore()
        .map_err(|error| error.code().to_owned())
}

#[tauri::command]
pub(crate) fn finish_local_data_restore(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    token: String,
) -> Result<(), String> {
    require_library_window(&window)?;
    runtime
        .local_data
        .finish_restore(&token)
        .map_err(|error| error.code().to_owned())
}

#[tauri::command]
pub(crate) async fn rollback_local_data_restore(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    token: String,
) -> Result<BrowserState, String> {
    require_library_window(&window)?;
    let data = runtime.local_data.clone();
    let messages = runtime.messages.clone();
    let previous = tauri::async_runtime::spawn_blocking(move || {
        data.rollback_restore(&token, &messages)
            .map_err(|error| error.code().to_owned())
    })
    .await
    .map_err(|_| "local-data-rollback-task".to_owned())??;
    reset_reader(&runtime)?;
    Ok(previous)
}

#[tauri::command]
pub(crate) async fn abort_local_data_restore(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    token: String,
) -> Result<(), String> {
    require_library_window(&window)?;
    let data = runtime.local_data.clone();
    tauri::async_runtime::spawn_blocking(move || data.abort_restore(&token))
        .await
        .map_err(|_| "local-data-abort-task".to_owned())?
        .map_err(|error| error.code().to_owned())
}

#[tauri::command]
pub(crate) async fn local_data_storage_usage(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    browser_state: BrowserState,
) -> Result<StorageUsage, String> {
    require_library_window(&window)?;
    let data = runtime.local_data.clone();
    tauri::async_runtime::spawn_blocking(move || data.storage_usage(&browser_state))
        .await
        .map_err(|_| "local-data-storage-task".to_owned())?
        .map_err(|error| error.code().to_owned())
}

fn reset_reader(runtime: &ReaderRuntime) -> Result<(), String> {
    *runtime
        .current_book
        .write()
        .map_err(|_| "reader-state".to_owned())? = None;
    *runtime
        .current_edition
        .write()
        .map_err(|_| "reader-state".to_owned())? = None;
    *runtime
        .diagnostics
        .lock()
        .map_err(|_| "reader-state".to_owned())? = None;
    Ok(())
}

fn log_maintenance(operation: &str, result: &Result<(), (&'static str, String)>, started: Instant) {
    log_maintenance_result(operation, result, started);
}

fn log_maintenance_result<T>(
    operation: &str,
    result: &Result<T, (&'static str, String)>,
    started: Instant,
) {
    match result {
        Ok(_) => log::info!(
            target: "atha::local_data",
            "operation={operation} outcome=success duration_ms={}",
            started.elapsed().as_millis()
        ),
        Err((stage, code)) => log::warn!(
            target: "atha::local_data",
            "operation={operation} stage={stage} outcome=failed code={} duration_ms={}",
            code,
            started.elapsed().as_millis()
        ),
    }
}
