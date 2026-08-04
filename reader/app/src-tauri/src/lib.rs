use std::{
    borrow::Cow,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process,
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};

use atha_backend::{
    messages::{
        ConversationView, CreatedMessage, CreatedRevision, CreatedRoot, CreatedSource,
        EditionInput, LegacyImport, LegacyImportResult, MessageRelationships, MessageSearch,
        MessageSearchHit, MessageStore, ReplyDraft, ReselectDraft, RevisionView, RootMessageDraft,
        RootMessageView, SnapshotResourceData, SourceCaptureView,
    },
    reader::{
        READER_PAGE,
        epub::{ImportError, READER_MANIFEST},
        library::{LibraryBook, LibraryError, LocalLibrary},
        resources::{BookRoot, Resource, ResourceError},
        telemetry::{ReaderEvent, parse_reader_event},
    },
};
use atha_reader_host::{
    diagnostics::{DiagnosticError, Diagnostics, ReadyDisposition, safe_event},
    launch::{
        Arguments, BookSource, MIN_WINDOW_HEIGHT_LOGICAL, MIN_WINDOW_WIDTH_LOGICAL,
        content_fingerprint, initial_window_size, reader_url, state_key,
    },
};
use serde::Serialize;
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    http::{Request, Response, StatusCode, header},
    webview::NewWindowResponse,
};
use tauri_plugin_dialog::DialogExt;

const TAURI_READER_PAGE: &str = "https://tauri.localhost/index.html";
const TAURI_READER_ORIGIN: &str = "https://tauri.localhost";
const MAX_IMPORT_FILES: usize = 32;
const PERMISSIONS_POLICY: &str = "accelerometer=(), autoplay=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), storage-access=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";

struct ReaderRuntime {
    diagnostics: Mutex<Option<Diagnostics>>,
    verify_sample: bool,
    hold_after_verify: bool,
    current_book: Arc<RwLock<Option<BookRoot>>>,
    current_edition: RwLock<Option<EditionInput>>,
    library: LocalLibrary,
    messages: MessageStore,
}

struct PreparedReader {
    root: BookRoot,
    app_path: String,
    diagnostics: Diagnostics,
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
    let mut diagnostics = runtime.diagnostics.lock().map_err(|_| "reader-state")?;
    let diagnostics = diagnostics.as_mut().ok_or("reader-state")?;
    match event {
        ReaderEvent::Metric(metric) => diagnostics
            .record_metric(metric)
            .map_err(|error| handle_diagnostic_error(diagnostics, &window, error))?,
        ReaderEvent::Ready(ready) => match diagnostics.record_ready(ready) {
            Ok(ReadyDisposition::VerificationComplete) if runtime.hold_after_verify => {
                let _ = window.set_title("Atha Reader Verification Complete");
            }
            Ok(ReadyDisposition::VerificationComplete) => app.exit(0),
            Ok(ReadyDisposition::Interactive) => {
                let _ = window.set_title("Atha Reader");
            }
            Err(error) => {
                let code = handle_diagnostic_error(diagnostics, &window, error);
                if runtime.verify_sample {
                    eprintln!("reader self-check failed: {code}");
                    app.exit(1);
                }
                return Err(code.into());
            }
        },
        ReaderEvent::Error(code) => {
            fail_run(diagnostics, &window, code);
            if runtime.verify_sample {
                eprintln!("reader self-check failed: {}", safe_event(code));
                app.exit(1);
            }
            return Err(code.into());
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
    let library = runtime.library.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut failures = Vec::new();
        for selected in paths {
            let path = match selected.into_path() {
                Ok(path) => path,
                Err(_) => {
                    failures.push(ImportFailure {
                        name: "EPUB".into(),
                        code: ImportError::InvalidSource.code(),
                    });
                    continue;
                }
            };
            if let Err(error) = library.import(&path) {
                failures.push(ImportFailure {
                    name: display_name(&path),
                    code: error.code(),
                });
            }
        }
        Ok(Some(ImportReport {
            books: library.list().map_err(|error| error.code().to_owned())?,
            failures,
        }))
    })
    .await
    .map_err(|_| "library-import-task".to_owned())?
}

#[tauri::command]
fn open_library_book(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    id: String,
) -> Result<ReaderLaunch, String> {
    let opened = runtime
        .library
        .open_book(&id)
        .map_err(|error| error.code().to_owned())?;
    let diagnostics = Diagnostics::new(Instant::now(), false, None)
        .map_err(|_| "reader-diagnostics".to_owned())?;
    *runtime
        .current_book
        .write()
        .map_err(|_| "reader-state".to_owned())? = Some(opened.root);
    *runtime
        .current_edition
        .write()
        .map_err(|_| "reader-state".to_owned())? = Some(EditionInput {
        content_version: opened.book.id.clone(),
        title: opened.book.title.clone(),
        authors: opened.book.authors.clone(),
    });
    *runtime
        .diagnostics
        .lock()
        .map_err(|_| "reader-state".to_owned())? = Some(diagnostics);
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

#[tauri::command]
async fn message_roots(
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
async fn message_edition_context(
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
async fn message_conversation(
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
async fn message_create_root(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: RootMessageDraft,
) -> Result<CreatedRoot, String> {
    require_reader_window(&window)?;
    runtime.messages.create_root(draft).map_err(message_error)
}

#[tauri::command]
async fn message_revise(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    message_id: String,
    expected_revision_id: String,
    text: Option<String>,
) -> Result<CreatedRevision, String> {
    require_reader_window(&window)?;
    runtime
        .messages
        .revise(&message_id, &expected_revision_id, text.as_deref())
        .map_err(message_error)
}

#[tauri::command]
async fn message_reply(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: ReplyDraft,
) -> Result<CreatedMessage, String> {
    require_reader_window(&window)?;
    runtime.messages.reply(draft).map_err(message_error)
}

#[tauri::command]
async fn message_delete(
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
async fn message_search(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    search: MessageSearch,
) -> Result<Vec<MessageSearchHit>, String> {
    require_reader_window(&window)?;
    runtime.messages.search(search).map_err(message_error)
}

#[tauri::command]
async fn message_relationships(
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
async fn message_revisions(
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
async fn message_source_captures(
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
async fn message_snapshot_resource(
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
async fn message_reselect(
    window: WebviewWindow,
    runtime: State<'_, ReaderRuntime>,
    draft: ReselectDraft,
) -> Result<CreatedSource, String> {
    require_reader_window(&window)?;
    runtime.messages.reselect(draft).map_err(message_error)
}

#[tauri::command]
async fn message_reanchor(
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
async fn message_import_legacy(
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
async fn message_export(
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

fn message_error(error: atha_backend::messages::MessageError) -> String {
    error.code().into()
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let startup = Instant::now();
    let arguments = if env::args_os().len() > 1 {
        Some(Arguments::parse()?)
    } else {
        None
    };
    let prepared = arguments
        .as_ref()
        .map(|arguments| prepare_reader(arguments, startup))
        .transpose()?;
    let data_root =
        PathBuf::from(env::var_os("LOCALAPPDATA").ok_or("missing LOCALAPPDATA")?).join("Atha");
    let library = LocalLibrary::open(&data_root)?;
    let messages = MessageStore::open(&data_root)?;
    let current_book = Arc::new(RwLock::new(
        prepared.as_ref().map(|reader| reader.root.clone()),
    ));
    let app_path = prepared
        .as_ref()
        .map_or_else(|| "index.html".to_owned(), |reader| reader.app_path.clone());
    let window_title = if prepared.is_some() {
        "Atha Reader"
    } else {
        "Atha"
    };
    let verify_sample = arguments
        .as_ref()
        .is_some_and(|arguments| arguments.verify_sample);
    let hold_after_verify = arguments
        .as_ref()
        .is_some_and(|arguments| arguments.hold_after_verify);
    let diagnostics = prepared.map(|reader| reader.diagnostics);
    let protocol_book = Arc::clone(&current_book);
    let cover_library = library.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ReaderRuntime {
            diagnostics: Mutex::new(diagnostics),
            verify_sample,
            hold_after_verify,
            current_book,
            current_edition: RwLock::new(None),
            library,
            messages,
        })
        .register_uri_scheme_protocol("atha-book", move |_context, request| {
            book_response(&protocol_book, request)
        })
        .register_uri_scheme_protocol("atha-cover", move |_context, request| {
            cover_response(&cover_library, request)
        })
        .invoke_handler(tauri::generate_handler![
            reader_event,
            list_library_books,
            import_library_books,
            open_library_book,
            remove_library_book,
            message_roots,
            message_edition_context,
            message_conversation,
            message_create_root,
            message_revise,
            message_reply,
            message_delete,
            message_search,
            message_relationships,
            message_revisions,
            message_source_captures,
            message_snapshot_resource,
            message_reselect,
            message_reanchor,
            message_import_legacy,
            message_export
        ])
        .setup(move |app| {
            let size = app
                .primary_monitor()?
                .map(|monitor| {
                    let scale = monitor.scale_factor();
                    let physical = monitor.size();
                    let screen = tao::dpi::LogicalSize::new(
                        f64::from(physical.width) / scale,
                        f64::from(physical.height) / scale,
                    );
                    initial_window_size(screen)
                })
                .unwrap_or_else(|| tao::dpi::LogicalSize::new(900.0, 900.0));
            let data_directory = app.path().app_local_data_dir()?.join("WebView2");

            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App(app_path.clone().into()))
                    .title(window_title)
                    .inner_size(size.width, size.height)
                    .min_inner_size(MIN_WINDOW_WIDTH_LOGICAL, MIN_WINDOW_HEIGHT_LOGICAL)
                    .resizable(true)
                    .maximizable(true)
                    .data_directory(data_directory)
                    .devtools(false)
                    .general_autofill_enabled(false)
                    .zoom_hotkeys_enabled(false)
                    .use_https_scheme(true)
                    .on_web_resource_request(|_, response| {
                        response.headers_mut().insert(
                            header::HeaderName::from_static("permissions-policy"),
                            header::HeaderValue::from_static(PERMISSIONS_POLICY),
                        );
                    })
                    .on_navigation(|url| is_app_navigation_url(url.as_str()))
                    .on_new_window(|_, _| NewWindowResponse::Deny)
                    .on_download(|_, _| false)
                    .prevent_overflow();
            window.build()?;
            Ok(())
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
    let diagnostics = Diagnostics::new(
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
    })
}

fn is_reader_url(url: &str) -> bool {
    let Some(suffix) = url.strip_prefix(TAURI_READER_PAGE) else {
        return false;
    };
    suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
}

fn is_app_navigation_url(url: &str) -> bool {
    url == "https://tauri.localhost/" || is_reader_url(url)
}

fn handle_diagnostic_error(
    diagnostics: &mut Diagnostics,
    window: &WebviewWindow,
    error: DiagnosticError,
) -> &'static str {
    match error {
        DiagnosticError::Reader(code) => {
            fail_run(diagnostics, window, code);
            code
        }
        DiagnosticError::Recorder(message, error) => {
            eprintln!("{message}: {error}");
            process::exit(1);
        }
    }
}

fn fail_run(diagnostics: &mut Diagnostics, window: &WebviewWindow, code: &str) {
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
        Err(_) => return empty_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let Some(root) = root else {
        return empty_response(StatusCode::NOT_FOUND);
    };
    match root.read(request.uri().path()) {
        Ok(resource) => resource_response(resource, "no-store", true),
        Err(error) => empty_response(resource_status(error)),
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
        Err(LibraryError::Resource(error)) => empty_response(resource_status(error)),
        Err(_) => empty_response(StatusCode::INTERNAL_SERVER_ERROR),
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
}
