use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use atha_backend::{
    local_data::LocalDataOperationGuard,
    messages::EditionInput,
    reader::{
        MAX_SOURCE_BYTES,
        epub::READER_MANIFEST,
        library::{
            BookOrganization, LibraryBook, LibraryError, LocalLibrary, MAX_CUSTOM_COVER_BYTES,
            OpenedBook, PendingBookDeletion,
        },
        resources::ResourceError,
    },
};
use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

#[cfg(desktop)]
use crate::desktop_files::dropped_library_paths;
use crate::{
    ReaderRuntime, message_maintenance, platform_file, telemetry::interactive_diagnostics,
};

pub(crate) const MAX_IMPORT_FILES: usize = 32;
pub(crate) const BOOK_EXTENSIONS: [&str; 10] = [
    "epub", "cbz", "fb2", "fbz", "mobi", "azw", "azw3", "md", "markdown", "txt",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportFailure {
    code: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportReport {
    books: Vec<LibraryBook>,
    failures: Vec<ImportFailure>,
}

pub(crate) struct StagedLibraryFiles {
    pub(crate) report: ImportReport,
    pub(crate) first_opened: Option<OpenedBook>,
}

impl StagedLibraryFiles {
    pub(crate) fn into_startup_import(self) -> StartupImport {
        StartupImport {
            book_id: self.first_opened.map(|opened| opened.book.id),
            failed: !self.report.failures.is_empty(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartupImport {
    pub(crate) book_id: Option<String>,
    pub(crate) failed: bool,
}

#[derive(Serialize)]
pub(crate) struct ReaderLaunch {
    pub(crate) href: String,
}

#[tauri::command]
pub(crate) fn list_library_books(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Vec<LibraryBook>, String> {
    let result = (|| {
        let _operation = begin_local_data_operation(&runtime)?;
        let started = Instant::now();
        let _ = window.set_title("Atha");
        runtime
            .library
            .list()
            .map_err(|error| library_command_error("list", "backend", &started, error))
    })();
    #[cfg(any(windows, target_os = "linux"))]
    if message_maintenance::require_library_window(&window).is_ok() {
        crate::desktop_open::observe_shelf_startup(&window, |id| {
            result
                .as_ref()
                .is_ok_and(|books| books.iter().any(|book| book.id == id))
        });
    }
    result
}

#[tauri::command]
pub(crate) async fn import_library_books(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Option<ImportReport>, String> {
    message_maintenance::require_library_window(&window)?;
    let Some(paths) = app
        .dialog()
        .file()
        .add_filter(
            "EPUB / CBZ / FB2 / FBZ / MOBI / AZW / AZW3 / Markdown / TXT",
            &BOOK_EXTENSIONS,
        )
        .blocking_pick_files()
    else {
        return Ok(None);
    };
    if paths.is_empty() {
        return Ok(None);
    }
    if paths.len() > MAX_IMPORT_FILES {
        return Err("invalid-library-import".into());
    }
    let _operation = begin_local_data_operation(&runtime)?;
    let staged = stage_library_files_async(app, runtime.library.clone(), paths).await?;
    Ok(Some(staged.report))
}

#[tauri::command]
#[cfg(desktop)]
pub(crate) async fn import_library_paths(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    paths: Vec<String>,
) -> Result<ImportReport, String> {
    message_maintenance::require_library_window(&window)?;
    let paths = dropped_library_paths(paths)?;
    let _operation = begin_local_data_operation(&runtime)?;
    let staged = stage_library_files_async(app, runtime.library.clone(), paths).await?;
    Ok(staged.report)
}

#[tauri::command]
pub(crate) fn take_startup_import(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Option<StartupImport>, String> {
    message_maintenance::require_library_window(&window)?;
    let startup = runtime
        .startup_import
        .lock()
        .map_err(|_| "reader-state".to_owned())?
        .take();
    #[cfg(any(windows, target_os = "linux"))]
    if startup.is_none()
        && let Some(id) = crate::desktop_open::take_shelf_handoff(&window)
    {
        return Ok(Some(StartupImport {
            book_id: Some(id),
            failed: false,
        }));
    }
    Ok(startup)
}

pub(crate) async fn stage_library_files_async(
    app: AppHandle,
    library: LocalLibrary,
    paths: Vec<FilePath>,
) -> Result<StagedLibraryFiles, String> {
    let started = Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        stage_library_files(&app, &library, paths, started)
    })
    .await
    .map_err(|_| {
        log::error!(
            target: "atha::library",
            "operation=import stage=task outcome=failed code=library-import-task duration_ms={}",
            started.elapsed().as_millis()
        );
        "library-import-task".to_owned()
    })?
}

pub(crate) fn stage_library_files(
    app: &AppHandle,
    library: &LocalLibrary,
    paths: Vec<FilePath>,
    started: Instant,
) -> Result<StagedLibraryFiles, String> {
    let selected_count = paths.len();
    let mut failures = Vec::new();
    let mut first_opened = None;
    for selected in paths {
        let input = match platform_file::PickerInput::open_book(app, selected, MAX_SOURCE_BYTES) {
            Ok(input) => input,
            Err(_) => {
                log::warn!(
                    target: "atha::library",
                    "operation=import stage=picker-input outcome=failed code=invalid-library-source"
                );
                failures.push(ImportFailure {
                    code: "invalid-library-source",
                });
                continue;
            }
        };
        match library.stage_with_title_hint(input.path(), input.title_hint()) {
            Ok(book) => match library.open_book(&book.id) {
                Ok(opened) => {
                    first_opened.get_or_insert(opened);
                }
                Err(error) => {
                    log::warn!(
                        target: "atha::library",
                        "operation=import stage=prepare outcome=failed code={}",
                        error.code()
                    );
                    failures.push(ImportFailure { code: error.code() });
                }
            },
            Err(error) => {
                log::warn!(
                    target: "atha::library",
                    "operation=import stage=backend outcome=failed code={}",
                    error.code()
                );
                failures.push(ImportFailure { code: error.code() });
                continue;
            }
        };
    }
    let books = library.list().map_err(|error| {
        log::error!(
            target: "atha::library",
            "operation=import stage=list outcome=failed code={} duration_ms={}",
            error.code(),
            started.elapsed().as_millis()
        );
        error.code().to_owned()
    })?;
    log::info!(
        target: "atha::library",
        "operation=import outcome={} count={} failure_count={} duration_ms={}",
        if failures.is_empty() { "ok" } else { "partial" },
        selected_count - failures.len(),
        failures.len(),
        started.elapsed().as_millis()
    );
    Ok(StagedLibraryFiles {
        report: ImportReport { books, failures },
        first_opened,
    })
}

#[tauri::command]
pub(crate) async fn set_library_book_cover(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<Option<Vec<LibraryBook>>, String> {
    message_maintenance::require_library_window(&window)?;
    let Some(selected) = app
        .dialog()
        .file()
        .add_filter("JPEG / PNG / WebP", &["jpg", "jpeg", "png", "webp"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let _operation = begin_local_data_operation(&runtime)?;
    let started = Instant::now();
    let library = runtime.library.clone();
    let books = tauri::async_runtime::spawn_blocking(move || {
        let input =
            platform_file::PickerInput::open(&app, selected, "cover", MAX_CUSTOM_COVER_BYTES)
                .map_err(|_| "invalid-library-cover".to_owned())?;
        library
            .set_custom_cover(&id, input.path())
            .map_err(|error| library_command_error("cover", "write", &started, error))?;
        library
            .list()
            .map_err(|error| library_command_error("cover", "list", &started, error))
    })
    .await
    .map_err(|_| "library-cover-task".to_owned())??;
    Ok(Some(books))
}

#[tauri::command]
pub(crate) fn reset_library_book_cover(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<Vec<LibraryBook>, String> {
    message_maintenance::require_library_window(&window)?;
    let _operation = begin_local_data_operation(&runtime)?;
    let started = Instant::now();
    runtime
        .library
        .reset_custom_cover(&id)
        .map_err(|error| library_command_error("cover", "reset", &started, error))?;
    runtime
        .library
        .list()
        .map_err(|error| library_command_error("cover", "list", &started, error))
}

#[tauri::command]
pub(crate) fn update_library_book_organization(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
    organization: BookOrganization,
) -> Result<Vec<LibraryBook>, String> {
    message_maintenance::require_library_window(&window)?;
    let _operation = runtime
        .local_data
        .exclusive_operation_guard()
        .map_err(|error| error.code().to_owned())?;
    let started = Instant::now();
    runtime
        .library
        .update_book_organization(&id, organization)
        .map_err(|error| library_command_error("organization", "write", &started, error))?;
    runtime
        .library
        .list()
        .map_err(|error| library_command_error("organization", "list", &started, error))
}

#[tauri::command]
pub(crate) async fn open_library_book(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<ReaderLaunch, String> {
    #[cfg(any(windows, target_os = "linux"))]
    let mut opening = crate::desktop_open::begin_library_open(&window, &id)?;
    let _operation = begin_local_data_operation(&runtime)?;
    #[cfg(any(windows, target_os = "linux"))]
    if let Some(opening) = opening.as_mut()
        && let Some(prepared) = opening.prepared.take()
    {
        return opening.expect_reader(prepared);
    }
    let started = Instant::now();
    let library = runtime.library.clone();
    let open_id = id.clone();
    let opened = tauri::async_runtime::spawn_blocking(move || library.open_book(&open_id))
        .await
        .map_err(|_| {
            log::error!(
                target: "atha::library",
                "operation=open stage=task outcome=failed code=library-open-task duration_ms={}",
                started.elapsed().as_millis()
            );
            "library-open-task".to_owned()
        })?
        .map_err(|error| {
            log::error!(
                target: "atha::library",
                "operation=open outcome=failed code={} duration_ms={}",
                error.code(),
                started.elapsed().as_millis()
            );
            error.code().to_owned()
        })?;
    let activation = ReaderActivation::new(opened, &started)?;
    #[cfg(any(windows, target_os = "linux"))]
    if let Some(opening) = opening.as_mut() {
        return opening.expect_reader(activation);
    }
    let launch = activation.launch();
    activation.activate(&runtime, &window)?;
    Ok(launch)
}

pub(crate) struct ReaderActivation {
    opened: OpenedBook,
    diagnostics: crate::RuntimeDiagnostics,
    started: Instant,
}

impl ReaderActivation {
    pub(crate) fn new(opened: OpenedBook, started: &Instant) -> Result<Self, String> {
        let diagnostics = interactive_diagnostics().map_err(|_| {
            log::error!(
                target: "atha::library",
                "operation=open outcome=failed code=reader-diagnostics duration_ms={}",
                started.elapsed().as_millis()
            );
            "reader-diagnostics".to_owned()
        })?;
        Ok(Self {
            opened,
            diagnostics,
            started: *started,
        })
    }

    #[cfg(any(windows, target_os = "linux"))]
    pub(crate) fn book_id(&self) -> &str {
        &self.opened.book.id
    }

    fn state(&self) -> &str {
        &self.opened.book.id[..16]
    }

    pub(crate) fn launch(&self) -> ReaderLaunch {
        ReaderLaunch {
            href: format!(
                "index.html?manifest={READER_MANIFEST}&state={}",
                self.state()
            ),
        }
    }

    pub(crate) fn activate(
        self,
        runtime: &ReaderRuntime,
        window: &WebviewWindow,
    ) -> Result<(), String> {
        let Self {
            opened,
            diagnostics,
            started,
        } = self;
        let mut reader = runtime
            .reader
            .write()
            .map_err(|_| "reader-state".to_owned())?;
        let title = format!("{} — Atha", opened.book.title);
        *reader = crate::ReaderSession {
            root: Some(opened.root),
            edition: Some(EditionInput {
                content_version: opened.book.id,
                title: opened.book.title,
                authors: opened.book.authors,
            }),
            diagnostics: Arc::new(Mutex::new(Some(diagnostics))),
        };
        drop(reader);
        let _ = window.set_title(&title);
        log::info!(
            target: "atha::library",
            "operation=open outcome=ok duration_ms={}",
            started.elapsed().as_millis()
        );
        Ok(())
    }
}

#[tauri::command]
pub(crate) fn remove_library_book(
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<Vec<LibraryBook>, String> {
    let _operation = begin_local_data_operation(&runtime)?;
    let started = Instant::now();
    runtime
        .library
        .remove(&id)
        .map_err(|error| library_command_error("remove", "record", &started, error))?;
    runtime
        .library
        .list()
        .map_err(|error| library_command_error("remove", "list", &started, error))
}

#[tauri::command]
pub(crate) async fn delete_library_book_data(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<PendingBookDeletion, String> {
    message_maintenance::require_library_window(&window)?;
    let _operation = runtime
        .local_data
        .deletion_guard()
        .map_err(|error| error.code().to_owned())?;
    let started = Instant::now();
    let library = runtime.library.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let pending = library
            .prepare_local_data_deletion(&id)
            .map_err(|error| library_command_error("delete", "data", &started, error))?;
        Ok(pending)
    })
    .await
    .map_err(|_| "library-delete-task".to_owned())?
}

#[tauri::command]
pub(crate) fn pending_library_book_deletions(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Vec<PendingBookDeletion>, String> {
    message_maintenance::require_library_window(&window)?;
    let result = (|| {
        let _operation = runtime
            .local_data
            .coordination_guard()
            .map_err(|error| error.code().to_owned())?;
        let pending = runtime
            .library
            .pending_local_data_deletions()
            .map_err(|error| library_command_error("delete", "pending", &Instant::now(), error))?;
        for deletion in &pending {
            runtime
                .library
                .resume_local_data_deletion(&deletion.id)
                .map_err(|error| {
                    library_command_error("delete", "resume", &Instant::now(), error)
                })?;
        }
        Ok(pending)
    })();
    #[cfg(any(windows, target_os = "linux"))]
    crate::desktop_open::observe_shelf_startup(&window, |_| {
        result.as_ref().is_ok_and(Vec::is_empty)
    });
    result
}

#[tauri::command]
pub(crate) fn finish_library_book_deletion(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<Vec<LibraryBook>, String> {
    message_maintenance::require_library_window(&window)?;
    let _operation = runtime
        .local_data
        .coordination_guard()
        .map_err(|error| error.code().to_owned())?;
    let started = Instant::now();
    runtime
        .library
        .finish_local_data_deletion(&id)
        .map_err(|error| library_command_error("delete", "finish", &started, error))?;
    runtime
        .library
        .list()
        .map_err(|error| library_command_error("delete", "list", &started, error))
}

pub(crate) fn require_local_data_ready(runtime: &ReaderRuntime) -> Result<(), String> {
    begin_local_data_operation(runtime).map(|_| ())
}

pub(crate) fn begin_local_data_operation(
    runtime: &ReaderRuntime,
) -> Result<LocalDataOperationGuard, String> {
    runtime
        .local_data
        .operation_guard()
        .map_err(|error| error.code().to_owned())
}

fn library_command_error(
    operation: &'static str,
    stage: &'static str,
    started: &Instant,
    error: LibraryError,
) -> String {
    let code = error.code();
    if is_internal_library_error(error) {
        log::error!(
            target: "atha::library",
            "operation={operation} stage={stage} outcome=failed code={code} duration_ms={}",
            started.elapsed().as_millis()
        );
    }
    code.into()
}

const fn is_internal_library_error(error: LibraryError) -> bool {
    matches!(
        error,
        LibraryError::InvalidRoot
            | LibraryError::CorruptRecord
            | LibraryError::ReadFailed
            | LibraryError::WriteFailed
            | LibraryError::Resource(ResourceError::InvalidRoot | ResourceError::ReadFailed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_internal_library_failures_are_log_worthy() {
        for error in [
            LibraryError::InvalidRoot,
            LibraryError::CorruptRecord,
            LibraryError::ReadFailed,
            LibraryError::WriteFailed,
            LibraryError::Resource(ResourceError::InvalidRoot),
            LibraryError::Resource(ResourceError::ReadFailed),
        ] {
            assert!(is_internal_library_error(error));
        }
        for error in [
            LibraryError::InvalidBookId,
            LibraryError::UnknownBook,
            LibraryError::MissingSource,
            LibraryError::MissingCover,
            LibraryError::InvalidCover,
            LibraryError::UnsupportedSource,
            LibraryError::Resource(ResourceError::InvalidPath),
            LibraryError::Resource(ResourceError::NotFound),
        ] {
            assert!(!is_internal_library_error(error));
        }
    }
}
