use std::{
    borrow::Cow,
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};

#[cfg(windows)]
use std::{env, fs, process};

use atha_backend::{
    messages::{EditionInput, MessageStore},
    reader::{
        READER_PAGE,
        epub::{ImportError, MAX_SOURCE_BYTES, READER_MANIFEST},
        library::{LibraryBook, LibraryError, LocalLibrary},
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
use tauri_plugin_dialog::DialogExt;

mod message_commands;
mod message_maintenance;
mod platform_file;
mod runtime_diagnostics;

use runtime_diagnostics::{
    DiagnosticError as RuntimeDiagnosticError, Diagnostics as RuntimeDiagnostics, ReadyDisposition,
};

const TAURI_READER_PAGE: &str = "https://tauri.localhost/index.html";
const TAURI_READER_ORIGIN: &str = "https://tauri.localhost";
const MAX_IMPORT_FILES: usize = 32;
const PERMISSIONS_POLICY: &str = "accelerometer=(), autoplay=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), storage-access=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";

struct ReaderRuntime {
    diagnostics: Mutex<Option<RuntimeDiagnostics>>,
    verify_sample: bool,
    #[cfg(windows)]
    hold_after_verify: bool,
    current_book: Arc<RwLock<Option<BookRoot>>>,
    current_edition: RwLock<Option<EditionInput>>,
    library: LocalLibrary,
    messages: MessageStore,
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
    name: String,
    code: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportReport {
    books: Vec<LibraryBook>,
    failures: Vec<ImportFailure>,
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
    let _ = window.set_title("Atha");
    runtime.library.list().map_err(|error| error.code().into())
}

#[tauri::command]
async fn import_library_books(
    app: AppHandle,
    runtime: State<'_, ReaderRuntime>,
) -> Result<Option<ImportReport>, String> {
    let Some(paths) = app
        .dialog()
        .file()
        .add_filter("EPUB", &["epub"])
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
    let selected_count = paths.len();
    let started = Instant::now();
    let library = runtime.library.clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut failures = Vec::new();
        for selected in paths {
            let name = selected
                .clone()
                .into_path()
                .map_or_else(|_| "EPUB".into(), |path| display_name(&path));
            let input =
                match platform_file::PickerInput::open(&app, selected, "epub", MAX_SOURCE_BYTES) {
                    Ok(input) => input,
                    Err(_) => {
                        log::warn!(
                            target: "atha::library",
                            "operation=import stage=picker-input outcome=failed code={}",
                            ImportError::InvalidSource.code()
                        );
                        failures.push(ImportFailure {
                            name,
                            code: ImportError::InvalidSource.code(),
                        });
                        continue;
                    }
                };
            if let Err(error) = library.import(input.path()) {
                log::warn!(
                    target: "atha::library",
                    "operation=import stage=backend outcome=failed code={}",
                    error.code()
                );
                failures.push(ImportFailure {
                    name,
                    code: error.code(),
                });
            }
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
        Ok(Some(ImportReport { books, failures }))
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

#[tauri::command]
fn open_library_book(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<ReaderLaunch, String> {
    let started = Instant::now();
    let opened = runtime.library.open_book(&id).map_err(|error| {
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
    runtime
        .library
        .remove(&id)
        .map_err(|error| error.code().to_owned())?;
    runtime
        .library
        .list()
        .map_err(|error| error.code().to_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    try_run().expect("Atha Tauri reader failed");
}

fn try_run() -> Result<(), Box<dyn Error>> {
    let startup = Instant::now();
    #[cfg(windows)]
    let launch = windows_launch_state(startup)?;
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
    let app_path = launch.app_path;
    let window_title = launch.window_title;
    let launch_mode = launch.mode;
    let verify_sample = launch.verify_sample;
    #[cfg(windows)]
    let hold_after_verify = launch.hold_after_verify;
    let current_edition = launch.edition;
    let diagnostics = launch.diagnostics;

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
            open_library_book,
            remove_library_book,
            message_maintenance::backup_message_store,
            message_maintenance::restore_message_store,
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
                let library = LocalLibrary::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                let messages = MessageStore::open(&data_root).inspect_err(|error| {
                    log::error!(
                        target: "atha::startup",
                        "event=application_start stage=state outcome=failed code={}",
                        error.code()
                    );
                })?;
                app.manage(ReaderRuntime {
                    diagnostics: Mutex::new(diagnostics),
                    verify_sample,
                    #[cfg(windows)]
                    hold_after_verify,
                    current_book,
                    current_edition: RwLock::new(current_edition),
                    library,
                    messages,
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
fn windows_launch_state(startup: Instant) -> Result<LaunchState, Box<dyn Error>> {
    let arguments = if env::args_os().len() > 1 {
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

    window.build()?;
    Ok(())
}

#[cfg(windows)]
fn interactive_diagnostics() -> Result<RuntimeDiagnostics, Box<dyn Error>> {
    RuntimeDiagnostics::new(Instant::now(), false, None)
}

#[cfg(not(windows))]
fn interactive_diagnostics() -> Result<RuntimeDiagnostics, Box<dyn Error>> {
    Ok(RuntimeDiagnostics::default())
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
    let root = match current.read() {
        Ok(value) => value.clone(),
        Err(_) => {
            log::error!(
                target: "atha::protocol",
                "operation=book_resource outcome=failed code=reader-state"
            );
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let Some(root) = root else {
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
        Ok(resource) => resource_response(resource, "private, max-age=31536000, immutable", false),
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

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("EPUB")
        .chars()
        .take(256)
        .collect()
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
        assert!(is_reader_url(
            "https://tauri.localhost/index.html?state=test"
        ));
        assert!(!is_reader_url("https://tauri.localhost/other.html"));
        assert!(!is_reader_url("https://example.com/index.html"));
    }

    #[test]
    fn canonical_app_root_navigation_is_allowed() {
        assert!(is_app_navigation_url("https://tauri.localhost/"));
        assert!(is_app_navigation_url("https://tauri.localhost/index.html"));
        assert!(!is_app_navigation_url("https://example.com/"));
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
}
