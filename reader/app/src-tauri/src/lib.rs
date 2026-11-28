use std::{
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};

#[cfg(desktop)]
use std::env;
#[cfg(windows)]
use std::fs;

use atha_backend::{
    local_data::LocalData,
    messages::{EditionInput, MessageStore},
    reader::{dictionary::LocalDictionaries, library::LocalLibrary, resources::BookRoot},
};
#[cfg(windows)]
use atha_reader_host::launch::{
    Arguments, BookSource, MIN_WINDOW_HEIGHT_LOGICAL, MIN_WINDOW_WIDTH_LOGICAL,
    content_fingerprint, initial_window_size, reader_url, state_key,
};
#[cfg(windows)]
use tauri::LogicalSize;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, http::header, webview::NewWindowResponse};
use tauri_plugin_dialog::FilePath;

#[cfg(desktop)]
mod desktop_files;
#[cfg(any(windows, target_os = "linux"))]
mod desktop_open;
mod dictionary_commands;
mod library_commands;
#[cfg(target_os = "linux")]
mod page_curl_capture;
mod local_data_maintenance;
mod message_commands;
mod message_maintenance;
mod platform_file;
mod protocol;
mod runtime_diagnostics;
mod telemetry;

#[cfg(desktop)]
use desktop_files::associated_book_paths;
use library_commands::{StartupImport, stage_library_files};
pub(crate) use library_commands::{begin_local_data_operation, require_local_data_ready};
use runtime_diagnostics::Diagnostics as RuntimeDiagnostics;

#[cfg(any(windows, target_os = "android"))]
pub(crate) const TAURI_LIBRARY_PAGE: &str = "https://tauri.localhost/";
#[cfg(not(any(windows, target_os = "android")))]
pub(crate) const TAURI_LIBRARY_PAGE: &str = "tauri://localhost";
#[cfg(any(windows, target_os = "android"))]
const TAURI_READER_PAGE: &str = "https://tauri.localhost/index.html";
#[cfg(not(any(windows, target_os = "android")))]
const TAURI_READER_PAGE: &str = "tauri://localhost/index.html";
#[cfg(desktop)]
const READER_GUI_GATE_SIZES: [(u32, u32); 5] = [
    (360, 760),
    (600, 760),
    (1000, 760),
    (1280, 800),
    (1600, 900),
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
    reader: RwLock<ReaderSession>,
    verify_sample: bool,
    #[cfg(windows)]
    hold_after_verify: bool,
    dictionaries: LocalDictionaries,
    library: LocalLibrary,
    local_data: LocalData,
    messages: MessageStore,
    startup_import: Mutex<Option<StartupImport>>,
}

#[derive(Clone, Default)]
struct ReaderSession {
    root: Option<BookRoot>,
    edition: Option<EditionInput>,
    diagnostics: Arc<Mutex<Option<RuntimeDiagnostics>>>,
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
    let independent_reader = associated_paths.is_none() && env::args_os().len() > 1;
    let library_launch = || LaunchState {
        book: None,
        app_path: "index.html".into(),
        window_title: "Atha",
        mode: "library",
        verify_sample: false,
        #[cfg(windows)]
        hold_after_verify: false,
        edition: None,
        diagnostics: None,
    };
    #[cfg(windows)]
    let launch = if independent_reader {
        windows_launch_state(startup, recover_windows_local_data()?)?
    } else {
        library_launch()
    };
    #[cfg(not(windows))]
    let launch = library_launch();
    let mut book = launch.book;
    let mut app_path = launch.app_path;
    let mut window_title = launch.window_title;
    let mut launch_mode = launch.mode;
    let mut verify_sample = launch.verify_sample;
    #[cfg(windows)]
    let hold_after_verify = launch.hold_after_verify;
    let mut current_edition = launch.edition;
    let mut diagnostics = launch.diagnostics;

    let context = tauri::generate_context!();
    let builder = tauri::Builder::default();
    #[cfg(any(windows, target_os = "linux"))]
    let (builder, endpoint_startup) = desktop_open::register_first(builder, &context)?;
    #[cfg(any(windows, target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_clipboard_manager::init());
    let app = builder
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
            protocol::book_response(&runtime.reader, request)
        })
        .register_uri_scheme_protocol("atha-cover", |context, request| {
            let runtime = context.app_handle().state::<ReaderRuntime>();
            protocol::cover_response(&runtime.library, request)
        })
        .invoke_handler(tauri::generate_handler![
            telemetry::reader_event,
            #[cfg(target_os = "linux")]
            page_curl_capture::page_curl_frame,
            library_commands::list_library_books,
            library_commands::import_library_books,
            #[cfg(desktop)]
            library_commands::import_library_paths,
            library_commands::take_startup_import,
            library_commands::open_library_book,
            library_commands::set_library_book_cover,
            library_commands::reset_library_book_cover,
            library_commands::update_library_book_organization,
            library_commands::remove_library_book,
            library_commands::delete_library_book_data,
            library_commands::pending_library_book_deletions,
            library_commands::finish_library_book_deletion,
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
                    book = None;
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
                        Ok(staged) => Some(staged.into_startup_import()),
                        Err(_) => Some(StartupImport {
                            book_id: None,
                            failed: true,
                        }),
                    };
                }
                #[cfg(any(windows, target_os = "linux"))]
                desktop_open::prioritize_cold_start(
                    app,
                    startup_import.as_ref().and_then(|import| import.book_id.clone()),
                );
                app.manage(ReaderRuntime {
                    reader: RwLock::new(ReaderSession {
                        root: book,
                        edition: current_edition,
                        diagnostics: Arc::new(Mutex::new(diagnostics)),
                    }),
                    verify_sample,
                    #[cfg(windows)]
                    hold_after_verify,
                    dictionaries,
                    library,
                    local_data,
                    messages,
                    startup_import: Mutex::new(startup_import),
                });
                build_main_window(app, &app_path, window_title)?;
                #[cfg(any(windows, target_os = "linux"))]
                desktop_open::start_worker(app);
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
        .build(context)?;
    #[cfg(any(windows, target_os = "linux"))]
    drop(endpoint_startup);
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let runtime = app.state::<ReaderRuntime>();
            if let Ok(reader) = runtime.reader.read()
                && let Ok(mut diagnostics) = reader.diagnostics.lock()
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
    #[cfg(any(windows, target_os = "linux"))]
    let handle = app.handle().clone();
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
        .on_navigation(move |url| {
            if !is_app_navigation_url(url.as_str()) {
                return false;
            }
            #[cfg(any(windows, target_os = "linux"))]
            if !desktop_open::allow_navigation(&handle, url) {
                return false;
            }
            true
        })
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false);

    #[cfg(any(windows, target_os = "linux"))]
    let window = window.on_page_load(desktop_open::on_page_load);

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
        window
            .inner_size(f64::from(width), f64::from(height))
            .min_inner_size(f64::from(width), f64::from(height))
    } else {
        window
    };

    let _window = window.build()?;
    #[cfg(target_os = "linux")]
    page_curl_capture::install(&_window)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::TAURI_READER_ORIGIN;

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
}
