use std::{
    borrow::Cow,
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};

#[cfg(desktop)]
use std::{env, ffi::OsString};
#[cfg(windows)]
use std::{fs, process};

use atha_backend::{
    local_data::{LocalData, LocalDataOperationGuard},
    messages::{EditionInput, MessageStore},
    reader::{
        MAX_SOURCE_BYTES, READER_PAGE,
        dictionary::LocalDictionaries,
        epub::READER_MANIFEST,
        library::{
            LibraryBook, LibraryError, LocalLibrary, MAX_CUSTOM_COVER_BYTES, PendingBookDeletion,
        },
        resources::{BookRoot, Resource, ResourceError},
        telemetry::{MetricStage, ReaderEvent, parse_reader_event, safe_event},
    },
};
#[cfg(windows)]
use atha_reader_host::launch::{
    Arguments, BookSource, MIN_WINDOW_HEIGHT_LOGICAL, MIN_WINDOW_WIDTH_LOGICAL,
    content_fingerprint, initial_window_size, reader_url, state_key,
};
use serde::Serialize;
#[cfg(windows)]
use tauri::LogicalSize;
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    http::{Request, Response, StatusCode, header},
    webview::NewWindowResponse,
};
use tauri_plugin_dialog::{DialogExt, FilePath};

mod dictionary_commands;
mod local_data_maintenance;
mod message_commands;
mod message_maintenance;
mod platform_file;
mod runtime_diagnostics;

use runtime_diagnostics::{
    DiagnosticError as RuntimeDiagnosticError, Diagnostics as RuntimeDiagnostics, ReadyDisposition,
};

#[cfg(any(windows, target_os = "android"))]
pub(crate) const TAURI_LIBRARY_PAGE: &str = "https://tauri.localhost/";
#[cfg(not(any(windows, target_os = "android")))]
pub(crate) const TAURI_LIBRARY_PAGE: &str = "tauri://localhost";
#[cfg(any(windows, target_os = "android"))]
const TAURI_READER_PAGE: &str = "https://tauri.localhost/index.html";
#[cfg(not(any(windows, target_os = "android")))]
const TAURI_READER_PAGE: &str = "tauri://localhost/index.html";
#[cfg(any(windows, target_os = "android"))]
const TAURI_READER_ORIGIN: &str = "https://tauri.localhost";
#[cfg(not(any(windows, target_os = "android")))]
const TAURI_READER_ORIGIN: &str = "tauri://localhost";
const MAX_IMPORT_FILES: usize = 32;
#[cfg(desktop)]
const MAX_IMPORT_PATH_CHARS: usize = 32_768;
#[cfg(desktop)]
const READER_GUI_GATE_SIZES: [(u32, u32); 5] = [
    (360, 760),
    (600, 760),
    (1000, 760),
    (1280, 800),
    (1600, 900),
];
const BOOK_EXTENSIONS: [&str; 10] = [
    "epub", "cbz", "fb2", "fbz", "mobi", "azw", "azw3", "md", "markdown", "txt",
];
const PERMISSIONS_POLICY: &str = "accelerometer=(), autoplay=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), storage-access=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";

#[cfg(desktop)]
fn parse_reader_gui_gate_size(value: &str) -> Option<(u32, u32)> {
    let (width, height) = value.split_once('x')?;
    let size = (width.parse().ok()?, height.parse().ok()?);
    READER_GUI_GATE_SIZES.contains(&size).then_some(size)
}

#[cfg(desktop)]
fn reader_gui_gate_size() -> Option<(u32, u32)> {
    (env::var_os("ATHA_READER_GUI_GATE").as_deref() == Some(std::ffi::OsStr::new("1")))
        .then(|| env::var("ATHA_READER_GUI_VIEWPORT").ok())
        .flatten()
        .and_then(|value| parse_reader_gui_gate_size(&value))
}

struct ReaderRuntime {
    diagnostics: Mutex<Option<RuntimeDiagnostics>>,
    verify_sample: bool,
    #[cfg(windows)]
    hold_after_verify: bool,
    current_book: Arc<RwLock<Option<BookRoot>>>,
    current_edition: RwLock<Option<EditionInput>>,
    dictionaries: LocalDictionaries,
    library: LocalLibrary,
    local_data: LocalData,
    messages: MessageStore,
    startup_import: Mutex<Option<StartupImport>>,
}

#[cfg(windows)]
struct PreparedReader {
    root: BookRoot,
    app_path: String,
    diagnostics: RuntimeDiagnostics,
    edition: Option<EditionInput>,
}

struct LaunchState {
    book: Option<BookRoot>,
    app_path: String,
    window_title: &'static str,
    mode: &'static str,
    verify_sample: bool,
    #[cfg(windows)]
    hold_after_verify: bool,
    edition: Option<EditionInput>,
    diagnostics: Option<RuntimeDiagnostics>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportFailure {
    code: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportReport {
    books: Vec<LibraryBook>,
    failures: Vec<ImportFailure>,
}

struct StagedLibraryFiles {
    report: ImportReport,
    first_book_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupImport {
    book_id: Option<String>,
    failed: bool,
}

#[derive(Serialize)]
struct ReaderLaunch {
    href: String,
}

#[tauri::command]
async fn reader_event(
    app: AppHandle,
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message: String,
) -> Result<(), String> {
    let url = window.url().map_err(|_| "reader-url")?;
    if window.label() != "main" || !is_reader_url(url.as_str()) {
        return Err("invalid-origin".into());
    }

    let event = parse_reader_event(READER_PAGE, &message).map_err(|error| error.code())?;
    let diagnostic_state_error = || {
        log::error!(
            target: "atha::reader",
            "event=reader_failure stage=diagnostics code=reader-state"
        );
        "reader-state"
    };
    let mut diagnostics = runtime
        .diagnostics
        .lock()
        .map_err(|_| diagnostic_state_error())?;
    let diagnostics = diagnostics.as_mut().ok_or_else(diagnostic_state_error)?;
    match event {
        ReaderEvent::Metric(metric) => {
            diagnostics
                .record_metric(metric)
                .map_err(|error| handle_diagnostic_error(diagnostics, &window, error))?;
            if metric.stage == MetricStage::FirstStable {
                log::info!(
                    target: "atha::reader",
                    "event=reader_metric stage={} duration_ms={:.3} pages={} page_width={} page_height={} font_size={}",
                    metric.stage.as_str(),
                    metric.duration_ms,
                    metric.pages,
                    metric.page_width,
                    metric.page_height,
                    metric.font_size
                );
            }
        }
        ReaderEvent::Ready(ready) => match diagnostics.record_ready(ready) {
            Ok(disposition) => {
                log::info!(
                    target: "atha::reader",
                    "event=reader_ready pages={} inline_formulas={} display_formulas={} cuts={}",
                    ready.pages,
                    ready.inline_formulas,
                    ready.display_formulas,
                    ready.cuts
                );
                match disposition {
                    #[cfg(windows)]
                    ReadyDisposition::VerificationComplete if runtime.hold_after_verify => {
                        let _ = window.set_title("Atha Reader Verification Complete");
                    }
                    #[cfg(windows)]
                    ReadyDisposition::VerificationComplete => app.exit(0),
                    ReadyDisposition::Interactive => {
                        let _ = window.set_title("Atha Reader");
                    }
                }
            }
            Err(error) => {
                let code = handle_diagnostic_error(diagnostics, &window, error);
                if runtime.verify_sample {
                    log::error!(
                        target: "atha::reader",
                        "event=reader_verification outcome=failed code={}",
                        safe_event(code)
                    );
                    app.exit(1);
                }
                return Err(code.into());
            }
        },
        ReaderEvent::Search(search) => {
            log::info!(
                target: "atha::reader",
                "event=reader_search search_results={} search_truncated={} sections_scanned={} duration_ms={:.3}",
                search.results,
                search.truncated,
                search.sections_scanned,
                search.duration_ms
            );
        }
        ReaderEvent::ImageLoadTerminal(image_load) => {
            let [b1, b2, b3, b4] = image_load.batches;
            log::error!(
                target: "atha::reader",
                "event=reader_image_load_terminal passes={} remaining_current={} remaining_current_or_next={} generation_changed={} b1_selected={} b1_success={} b1_failure={} b1_layout={} b2_selected={} b2_success={} b2_failure={} b2_layout={} b3_selected={} b3_success={} b3_failure={} b3_layout={} b4_selected={} b4_success={} b4_failure={} b4_layout={}",
                image_load.passes,
                image_load.remaining_current,
                image_load.remaining_current_or_next,
                image_load.generation_changed,
                b1.selected,
                b1.success,
                b1.failure,
                b1.layout_changed,
                b2.selected,
                b2.success,
                b2.failure,
                b2.layout_changed,
                b3.selected,
                b3.success,
                b3.failure,
                b3.layout_changed,
                b4.selected,
                b4.success,
                b4.failure,
                b4.layout_changed
            );
        }
        ReaderEvent::Error(failure) => {
            log::error!(
                target: "atha::reader",
                "event=reader_failure stage={} code={}",
                failure.stage.as_str(),
                failure.code
            );
            fail_run(diagnostics, &window, failure.code);
            if runtime.verify_sample {
                log::error!(
                    target: "atha::reader",
                    "event=reader_verification outcome=failed code={}",
                    safe_event(failure.code)
                );
                app.exit(1);
            }
            return Err(failure.code.into());
        }
    }
    Ok(())
}

#[tauri::command]
fn list_library_books(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Vec<LibraryBook>, String> {
    let _operation = begin_local_data_operation(&runtime)?;
    let started = Instant::now();
    let _ = window.set_title("Atha");
    runtime
        .library
        .list()
        .map_err(|error| library_command_error("list", "backend", &started, error))
}

#[tauri::command]
async fn import_library_books(
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
async fn import_library_paths(
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
fn take_startup_import(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Option<StartupImport>, String> {
    message_maintenance::require_library_window(&window)?;
    runtime
        .startup_import
        .lock()
        .map_err(|_| "reader-state".to_owned())
        .map(|mut startup| startup.take())
}

#[cfg(desktop)]
fn dropped_library_paths(paths: Vec<String>) -> Result<Vec<FilePath>, String> {
    if paths.is_empty()
        || paths.len() > MAX_IMPORT_FILES
        || paths.iter().any(|path| {
            path.is_empty() || path.chars().count() > MAX_IMPORT_PATH_CHARS || path.contains('\0')
        })
    {
        return Err("invalid-library-import".into());
    }
    Ok(paths
        .into_iter()
        .map(|path| FilePath::Path(PathBuf::from(path)))
        .collect())
}

async fn stage_library_files_async(
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

fn stage_library_files(
    app: &AppHandle,
    library: &LocalLibrary,
    paths: Vec<FilePath>,
    started: Instant,
) -> Result<StagedLibraryFiles, String> {
    let selected_count = paths.len();
    let mut failures = Vec::new();
    let mut first_book_id = None;
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
                Ok(_) => {
                    first_book_id.get_or_insert(book.id);
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
        first_book_id,
    })
}

#[tauri::command]
async fn set_library_book_cover(
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
fn reset_library_book_cover(
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
async fn open_library_book(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<ReaderLaunch, String> {
    let _operation = begin_local_data_operation(&runtime)?;
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
    let diagnostics = interactive_diagnostics().map_err(|_| {
        log::error!(
            target: "atha::library",
            "operation=open outcome=failed code=reader-diagnostics duration_ms={}",
            started.elapsed().as_millis()
        );
        "reader-diagnostics".to_owned()
    })?;
    let state_error = || {
        log::error!(
            target: "atha::library",
            "operation=open outcome=failed code=reader-state duration_ms={}",
            started.elapsed().as_millis()
        );
        "reader-state".to_owned()
    };
    *runtime.current_book.write().map_err(|_| state_error())? = Some(opened.root);
    *runtime.current_edition.write().map_err(|_| state_error())? = Some(EditionInput {
        content_version: opened.book.id.clone(),
        title: opened.book.title.clone(),
        authors: opened.book.authors.clone(),
    });
    *runtime.diagnostics.lock().map_err(|_| state_error())? = Some(diagnostics);
    log::info!(
        target: "atha::library",
        "operation=open outcome=ok duration_ms={}",
        started.elapsed().as_millis()
    );
    let _ = window.set_title(&format!("{} — Atha", opened.book.title));
    Ok(ReaderLaunch {
        href: format!(
            "index.html?manifest={READER_MANIFEST}&state={}",
            &opened.book.id[..16]
        ),
    })
}

#[tauri::command]
fn remove_library_book(
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
async fn delete_library_book_data(
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
fn pending_library_book_deletions(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Vec<PendingBookDeletion>, String> {
    message_maintenance::require_library_window(&window)?;
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
            .map_err(|error| library_command_error("delete", "resume", &Instant::now(), error))?;
    }
    Ok(pending)
}

#[tauri::command]
fn finish_library_book_deletion(
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

#[cfg(desktop)]
fn associated_book_paths(values: impl IntoIterator<Item = OsString>) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for value in values {
        if paths.len() == MAX_IMPORT_FILES {
            return None;
        }
        let path = match value.to_str() {
            Some(value) if value.starts_with("file:") => {
                tauri::Url::parse(value).ok()?.to_file_path().ok()?
            }
            _ => PathBuf::from(value),
        };
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        if !BOOK_EXTENSIONS.contains(&extension.as_str()) {
            return None;
        }
        paths.push(path);
    }
    (!paths.is_empty()).then_some(paths)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    try_run().expect("Atha Tauri reader failed");
}

fn try_run() -> Result<(), Box<dyn Error>> {
    let startup = Instant::now();
    #[cfg(desktop)]
    let mut associated_paths = associated_book_paths(env::args_os().skip(1));
    #[cfg(not(desktop))]
    let mut associated_paths: Option<Vec<PathBuf>> = None;
    #[cfg(windows)]
    let launch = windows_launch_state(
        startup,
        recover_windows_local_data()? || associated_paths.is_some(),
    )?;
    #[cfg(not(windows))]
    let launch = LaunchState {
        book: None,
        app_path: "index.html".into(),
        window_title: "Atha",
        mode: "library",
        verify_sample: false,
        edition: None,
        diagnostics: None,
    };
    let current_book = Arc::new(RwLock::new(launch.book));
    let mut app_path = launch.app_path;
    let mut window_title = launch.window_title;
    let mut launch_mode = launch.mode;
    let mut verify_sample = launch.verify_sample;
    #[cfg(windows)]
    let hold_after_verify = launch.hold_after_verify;
    let mut current_edition = launch.edition;
    let mut diagnostics = launch.diagnostics;

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .filter(|metadata| metadata.target().starts_with("atha::"))
                .max_file_size(1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(2))
                .build(),
        )
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("atha-book", |context, request| {
            let runtime = context.app_handle().state::<ReaderRuntime>();
            book_response(&runtime.current_book, request)
        })
        .register_uri_scheme_protocol("atha-cover", |context, request| {
            let runtime = context.app_handle().state::<ReaderRuntime>();
            cover_response(&runtime.library, request)
        })
        .invoke_handler(tauri::generate_handler![
            reader_event,
            list_library_books,
            import_library_books,
            #[cfg(desktop)]
            import_library_paths,
            take_startup_import,
            open_library_book,
            set_library_book_cover,
            reset_library_book_cover,
            remove_library_book,
            delete_library_book_data,
            pending_library_book_deletions,
            finish_library_book_deletion,
            dictionary_commands::list_local_dictionaries,
            dictionary_commands::import_local_dictionary,
            dictionary_commands::lookup_local_dictionary,
            dictionary_commands::remove_local_dictionary,
            message_maintenance::backup_message_store,
            message_maintenance::restore_message_store,
            local_data_maintenance::backup_local_data,
            local_data_maintenance::prepare_local_data_restore,
            local_data_maintenance::commit_local_data_restore,
            local_data_maintenance::pending_local_data_restore,
            local_data_maintenance::finish_local_data_restore,
            local_data_maintenance::rollback_local_data_restore,
            local_data_maintenance::abort_local_data_restore,
            local_data_maintenance::local_data_storage_usage,
            message_commands::reading_memory_search,
            message_commands::reading_memory_source_captures,
            message_commands::reading_memory_snapshot_resource,
            message_commands::message_roots,
            message_commands::message_edition_context,
            message_commands::message_conversation,
            message_commands::message_conversations,
            message_commands::message_create_root,
            message_commands::message_revise,
            message_commands::message_reply,
            message_commands::message_delete,
            message_commands::message_search,
            message_commands::message_relationships,
            message_commands::message_revisions,
            message_commands::message_source_captures,
            message_commands::message_snapshot_resource,
            message_commands::message_reselect,
            message_commands::message_reanchor,
            message_commands::message_import_legacy,
            message_commands::message_export
        ])
        .setup(move |app| {
            log::info!(
                target: "atha::startup",
                "event=application_start stage=setup mode={launch_mode}"
            );
            let setup = (|| -> Result<(), Box<dyn Error>> {
                let data_root = product_data_root(app).inspect_err(|_| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code=local-data-root"
                    );
                })?;
                if platform_file::cleanup(app.handle()).is_err() {
                    log::warn!(
                        target: "atha::startup",
                        "event=application_start stage=cache outcome=failed code=picker-cache-cleanup"
                    );
                }
                let messages = MessageStore::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                let local_data = LocalData::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                local_data.recover(&messages).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=recovery outcome=failed code={}",
                        error.code()
                    );
                })?;
                let pending_restore = local_data.pending_restore()?.is_some();
                drop(messages);
                let messages = MessageStore::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                let library = LocalLibrary::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                let dictionaries = LocalDictionaries::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                let pending_deletion = !library.pending_local_data_deletions()?.is_empty();
                let mut startup_import = None;
                if pending_restore || pending_deletion {
                    app_path = "index.html".into();
                    window_title = "Atha";
                    launch_mode = "library";
                    verify_sample = false;
                    current_edition = None;
                    diagnostics = None;
                    *current_book.write().map_err(|_| "reader-state")? = None;
                    if associated_paths.is_some() {
                        startup_import = Some(StartupImport {
                            book_id: None,
                            failed: true,
                        });
                    }
                } else if let Some(paths) = associated_paths.take() {
                    let selected = paths.into_iter().map(FilePath::Path).collect();
                    startup_import = match stage_library_files(
                        app.handle(),
                        &library,
                        selected,
                        Instant::now(),
                    ) {
                        Ok(staged) => Some(StartupImport {
                            book_id: staged.first_book_id,
                            failed: !staged.report.failures.is_empty(),
                        }),
                        Err(_) => Some(StartupImport {
                            book_id: None,
                            failed: true,
                        }),
                    };
                }
                app.manage(ReaderRuntime {
                    diagnostics: Mutex::new(diagnostics),
                    verify_sample,
                    #[cfg(windows)]
                    hold_after_verify,
                    current_book,
                    current_edition: RwLock::new(current_edition),
                    dictionaries,
                    library,
                    local_data,
                    messages,
                    startup_import: Mutex::new(startup_import),
                });
                build_main_window(app, &app_path, window_title)?;
                Ok(())
            })();

            match setup {
                Ok(()) => {
                    log::info!(
                        target: "atha::startup",
                        "event=application_start stage=ready mode={} duration_ms={}",
                        launch_mode,
                        startup.elapsed().as_millis()
                    );
                    Ok(())
                }
                Err(error) => {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=setup outcome=failed code=startup-setup duration_ms={}",
                        startup.elapsed().as_millis()
                    );
                    Err(error)
                }
            }
        })
        .build(tauri::generate_context!())?
        .run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let runtime = app.state::<ReaderRuntime>();
                if let Ok(mut diagnostics) = runtime.diagnostics.lock()
                    && let Some(diagnostics) = diagnostics.as_mut()
                {
                    diagnostics.flush();
                }
            }
        });
    Ok(())
}

#[cfg(windows)]
fn windows_launch_state(
    startup: Instant,
    force_library: bool,
) -> Result<LaunchState, Box<dyn Error>> {
    let arguments = if !force_library && env::args_os().len() > 1 {
        Some(Arguments::parse()?)
    } else {
        None
    };
    let prepared = arguments
        .as_ref()
        .map(|arguments| prepare_reader(arguments, startup))
        .transpose()?;
    let reader_mode = prepared.is_some();
    let verify_sample = arguments
        .as_ref()
        .is_some_and(|arguments| arguments.verify_sample);
    let hold_after_verify = arguments
        .as_ref()
        .is_some_and(|arguments| arguments.hold_after_verify);
    Ok(LaunchState {
        book: prepared.as_ref().map(|reader| reader.root.clone()),
        app_path: prepared
            .as_ref()
            .map_or_else(|| "index.html".to_owned(), |reader| reader.app_path.clone()),
        window_title: if reader_mode { "Atha Reader" } else { "Atha" },
        mode: if reader_mode { "reader" } else { "library" },
        verify_sample,
        hold_after_verify,
        edition: prepared.as_ref().and_then(|reader| reader.edition.clone()),
        diagnostics: prepared.map(|reader| reader.diagnostics),
    })
}

#[cfg(windows)]
fn recover_windows_local_data() -> Result<bool, Box<dyn Error>> {
    let root =
        PathBuf::from(env::var_os("LOCALAPPDATA").ok_or("missing LOCALAPPDATA")?).join("Atha");
    let messages = MessageStore::open(&root)?;
    let data = LocalData::open(&root)?;
    data.recover(&messages)?;
    let pending_restore = data.pending_restore()?.is_some();
    let pending_deletion = !LocalLibrary::open(&root)?
        .pending_local_data_deletions()?
        .is_empty();
    Ok(pending_restore || pending_deletion)
}

#[cfg(windows)]
fn product_data_root(_app: &tauri::App) -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env::var_os("LOCALAPPDATA").ok_or("missing LOCALAPPDATA")?).join("Atha"))
}

#[cfg(not(windows))]
fn product_data_root(app: &tauri::App) -> Result<PathBuf, Box<dyn Error>> {
    Ok(app.path().app_local_data_dir()?)
}

fn build_main_window(
    app: &tauri::App,
    app_path: &str,
    window_title: &str,
) -> Result<(), Box<dyn Error>> {
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(app_path.into()))
        .title(window_title)
        .devtools(cfg!(all(target_os = "android", debug_assertions)))
        .use_https_scheme(true)
        .on_web_resource_request(|_, response| {
            response.headers_mut().insert(
                header::HeaderName::from_static("permissions-policy"),
                header::HeaderValue::from_static(PERMISSIONS_POLICY),
            );
        })
        .on_navigation(|url| is_app_navigation_url(url.as_str()))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false);

    #[cfg(windows)]
    let window = {
        let size = app
            .primary_monitor()?
            .map(|monitor| {
                let scale = monitor.scale_factor();
                let physical = monitor.size();
                initial_window_size(LogicalSize::new(
                    f64::from(physical.width) / scale,
                    f64::from(physical.height) / scale,
                ))
            })
            .unwrap_or_else(|| LogicalSize::new(900.0, 900.0));
        window
            .inner_size(size.width, size.height)
            .min_inner_size(MIN_WINDOW_WIDTH_LOGICAL, MIN_WINDOW_HEIGHT_LOGICAL)
            .resizable(true)
            .maximizable(true)
            .data_directory(app.path().app_local_data_dir()?.join("WebView2"))
            .general_autofill_enabled(false)
            .zoom_hotkeys_enabled(false)
            .prevent_overflow()
    };

    #[cfg(desktop)]
    let window = if let Some((width, height)) = reader_gui_gate_size() {
        window.inner_size(f64::from(width), f64::from(height))
    } else {
        window
    };

    window.build()?;
    Ok(())
}

#[cfg(windows)]
fn interactive_diagnostics() -> Result<RuntimeDiagnostics, Box<dyn Error>> {
    RuntimeDiagnostics::new(Instant::now(), false, None)
}

#[cfg(not(windows))]
fn interactive_diagnostics() -> Result<RuntimeDiagnostics, Box<dyn Error>> {
    Ok(RuntimeDiagnostics)
}

#[cfg(windows)]
fn prepare_reader(
    arguments: &Arguments,
    startup: Instant,
) -> Result<PreparedReader, Box<dyn Error>> {
    let book = arguments.resolve_book()?;
    let book_root = BookRoot::new(&book.book_root)?;
    let source_resource = book_root.read(&format!("/{}", book.source.path()))?;
    let canonical_source = fs::canonicalize(book.book_root.join(book.source.path()))?;
    let mut book_state_key = state_key(&canonical_source);
    if arguments.state_probe.is_some() {
        book_state_key.push_str("-probe");
    }
    let content_version = match &book.source {
        BookSource::Entry(_) => Some(content_fingerprint(&source_resource.bytes)),
        BookSource::Manifest(_) => None,
    };
    let edition = book
        .content_version
        .as_ref()
        .or(content_version.as_ref())
        .map(|content_version| EditionInput {
            content_version: content_version.clone(),
            title: book.title.clone().unwrap_or_else(|| "未命名书籍".into()),
            authors: book.authors.clone(),
        });
    let diagnostics = RuntimeDiagnostics::new(
        startup,
        arguments.verify_sample,
        arguments.benchmark.as_ref(),
    )?;
    let legacy_url = reader_url(
        arguments,
        &book.source,
        diagnostics.network_probe(),
        &book_state_key,
        content_version.as_deref(),
    );
    let query = legacy_url.split_once('?').map_or("", |(_, query)| query);
    Ok(PreparedReader {
        root: book_root,
        app_path: format!("index.html?{query}"),
        diagnostics,
        edition,
    })
}

fn is_reader_url(url: &str) -> bool {
    let Some(suffix) = url.strip_prefix(TAURI_READER_PAGE) else {
        return false;
    };
    suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
}

fn is_app_navigation_url(url: &str) -> bool {
    message_maintenance::is_library_url(url) || is_reader_url(url)
}

#[cfg(windows)]
fn handle_diagnostic_error(
    diagnostics: &mut RuntimeDiagnostics,
    window: &WebviewWindow,
    error: RuntimeDiagnosticError,
) -> &'static str {
    match error {
        RuntimeDiagnosticError::Reader(code) => {
            log::error!(
                target: "atha::reader",
                "event=reader_failure stage=diagnostics code={}",
                safe_event(code)
            );
            fail_run(diagnostics, window, code);
            code
        }
        RuntimeDiagnosticError::Recorder(_, _) => {
            log::error!(
                target: "atha::reader",
                "event=diagnostic_recorder outcome=failed code=write-failed"
            );
            process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn handle_diagnostic_error(
    _diagnostics: &mut RuntimeDiagnostics,
    _window: &WebviewWindow,
    error: RuntimeDiagnosticError,
) -> &'static str {
    match error {}
}

fn fail_run(diagnostics: &mut RuntimeDiagnostics, window: &WebviewWindow, code: &str) {
    diagnostics.record_failure(code);
    let _ = window.set_title("Atha Reader - Error");
}

fn book_response(
    current: &RwLock<Option<BookRoot>>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    let current = match current.read() {
        Ok(value) => value,
        Err(_) => {
            log::error!(
                target: "atha::protocol",
                "operation=book_resource outcome=failed code=reader-state"
            );
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let Some(root) = current.as_ref() else {
        return empty_response(StatusCode::NOT_FOUND);
    };
    match root.read(request.uri().path()) {
        Ok(resource) => resource_response(resource, "no-store", true),
        Err(error) => {
            if is_internal_resource_error(error) {
                log::error!(
                    target: "atha::protocol",
                    "operation=book_resource outcome=failed code={}",
                    error.code()
                );
            }
            empty_response(resource_status(error))
        }
    }
}

fn cover_response(
    library: &LocalLibrary,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    let id = request.uri().path().strip_prefix('/').unwrap_or_default();
    if id.contains('/') {
        return empty_response(StatusCode::BAD_REQUEST);
    }
    match library.cover(id) {
        Ok(resource) => resource_response(resource, "no-store", false),
        Err(LibraryError::InvalidBookId) => empty_response(StatusCode::BAD_REQUEST),
        Err(LibraryError::UnknownBook | LibraryError::MissingCover) => {
            empty_response(StatusCode::NOT_FOUND)
        }
        Err(LibraryError::Resource(error)) => {
            if is_internal_resource_error(error) {
                log::error!(
                    target: "atha::protocol",
                    "operation=cover_resource outcome=failed code={}",
                    error.code()
                );
            }
            empty_response(resource_status(error))
        }
        Err(error) => {
            log::error!(
                target: "atha::protocol",
                "operation=cover_resource outcome=failed code={}",
                error.code()
            );
            empty_response(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn resource_response(
    resource: Resource,
    cache_control: &'static str,
    cors: bool,
) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, resource.content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .header("x-content-type-options", "nosniff");
    if cors {
        response = response.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, TAURI_READER_ORIGIN);
    }
    response
        .body(Cow::Owned(resource.bytes))
        .expect("valid resource response")
}

fn empty_response(status: StatusCode) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Cow::Borrowed(&b"resource unavailable"[..]))
        .expect("valid error response")
}

const fn resource_status(error: ResourceError) -> StatusCode {
    match error {
        ResourceError::InvalidEncoding | ResourceError::InvalidPath => StatusCode::BAD_REQUEST,
        ResourceError::OutsideRoot => StatusCode::FORBIDDEN,
        ResourceError::NotFound | ResourceError::NotAFile => StatusCode::NOT_FOUND,
        ResourceError::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ResourceError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        ResourceError::InvalidRoot | ResourceError::ReadFailed => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

const fn is_internal_resource_error(error: ResourceError) -> bool {
    matches!(
        error,
        ResourceError::InvalidRoot | ResourceError::ReadFailed
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_reader_page_navigation_is_allowed() {
        assert!(is_reader_url(&format!("{TAURI_READER_PAGE}?state=test")));
        assert!(!is_reader_url(&format!("{TAURI_READER_ORIGIN}/other.html")));
        assert!(!is_reader_url("https://example.com/index.html"));
    }

    #[test]
    fn canonical_app_root_navigation_is_allowed() {
        assert!(is_app_navigation_url(TAURI_LIBRARY_PAGE));
        assert!(is_app_navigation_url(TAURI_READER_PAGE));
        assert!(!is_app_navigation_url("https://example.com/"));
    }

    #[test]
    fn gui_gate_accepts_only_declared_viewports() {
        assert_eq!(parse_reader_gui_gate_size("600x760"), Some((600, 760)));
        assert_eq!(parse_reader_gui_gate_size("1600x900"), Some((1600, 900)));
        assert_eq!(parse_reader_gui_gate_size("800x600"), None);
        assert_eq!(parse_reader_gui_gate_size("1280x760"), None);
        assert_eq!(parse_reader_gui_gate_size("wide"), None);
    }

    #[test]
    fn browser_permissions_are_disabled_by_policy() {
        for feature in [
            "camera=()",
            "clipboard-read=()",
            "display-capture=()",
            "geolocation=()",
            "local-fonts=()",
            "microphone=()",
            "midi=()",
            "window-management=()",
        ] {
            assert!(PERMISSIONS_POLICY.contains(feature));
        }
    }

    #[test]
    fn only_internal_resource_failures_are_log_worthy() {
        assert!(is_internal_resource_error(ResourceError::InvalidRoot));
        assert!(is_internal_resource_error(ResourceError::ReadFailed));
        assert!(!is_internal_resource_error(ResourceError::InvalidPath));
        assert!(!is_internal_resource_error(ResourceError::NotFound));
    }

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

    #[cfg(desktop)]
    #[test]
    fn file_association_arguments_are_only_supported_book_paths() {
        let paths = associated_book_paths([
            OsString::from("/tmp/one.EPUB"),
            OsString::from("file:///tmp/two%20words.fb2"),
        ])
        .expect("book arguments");
        assert_eq!(
            paths,
            [
                PathBuf::from("/tmp/one.EPUB"),
                PathBuf::from("/tmp/two words.fb2")
            ]
        );
        assert!(associated_book_paths([OsString::from("--epub")]).is_none());
        assert!(associated_book_paths([OsString::from("/tmp/book.pdf")]).is_none());
        assert!(associated_book_paths(std::iter::empty()).is_none());
    }

    #[cfg(desktop)]
    #[test]
    fn dropped_paths_have_a_small_explicit_boundary() {
        assert!(dropped_library_paths(vec!["/tmp/book.epub".into()]).is_ok());
        assert!(dropped_library_paths(Vec::new()).is_err());
        assert!(dropped_library_paths(vec!["bad\0path.epub".into()]).is_err());
        assert!(dropped_library_paths(vec!["x".repeat(MAX_IMPORT_PATH_CHARS + 1)]).is_err());
        assert!(dropped_library_paths(vec!["book.epub".into(); MAX_IMPORT_FILES + 1]).is_err());
    }
}
