use std::{borrow::Cow, error::Error, fs, process, sync::Mutex, time::Instant};

use atha_backend::reader::{
    READER_PAGE,
    resources::{BookRoot, ResourceError},
    telemetry::{ReaderEvent, parse_reader_event},
};
use atha_reader_host::{
    diagnostics::{DiagnosticError, Diagnostics, ReadyDisposition, safe_event},
    launch::{
        Arguments, BookSource, content_fingerprint, initial_window_size, reader_url, state_key,
    },
};
use tauri::{
    AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    http::{Request, Response, StatusCode, header},
    webview::NewWindowResponse,
};

const TAURI_READER_PAGE: &str = "https://tauri.localhost/index.html";
const TAURI_READER_ORIGIN: &str = "https://tauri.localhost";
const PERMISSIONS_POLICY: &str = "accelerometer=(), autoplay=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-create=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), storage-access=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";

struct ReaderRuntime {
    diagnostics: Mutex<Diagnostics>,
    verify_sample: bool,
    hold_after_verify: bool,
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
    match event {
        ReaderEvent::Metric(metric) => diagnostics
            .record_metric(metric)
            .map_err(|error| handle_diagnostic_error(&mut diagnostics, &window, error))?,
        ReaderEvent::Ready(ready) => match diagnostics.record_ready(ready) {
            Ok(ReadyDisposition::VerificationComplete) if runtime.hold_after_verify => {
                let _ = window.set_title("Atha Reader Verification Complete");
            }
            Ok(ReadyDisposition::VerificationComplete) => app.exit(0),
            Ok(ReadyDisposition::Interactive) => {
                let _ = window.set_title("Atha Reader");
            }
            Err(error) => {
                let code = handle_diagnostic_error(&mut diagnostics, &window, error);
                if runtime.verify_sample {
                    eprintln!("reader self-check failed: {code}");
                    app.exit(1);
                }
                return Err(code.into());
            }
        },
        ReaderEvent::Error(code) => {
            fail_run(&mut diagnostics, &window, code);
            if runtime.verify_sample {
                eprintln!("reader self-check failed: {}", safe_event(code));
                app.exit(1);
            }
            return Err(code.into());
        }
    }
    Ok(())
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let startup = Instant::now();
    let arguments = Arguments::parse()?;
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
        &arguments,
        &book.source,
        diagnostics.network_probe(),
        &book_state_key,
        content_version.as_deref(),
    );
    let query = legacy_url.split_once('?').map_or("", |(_, query)| query);
    let app_path = format!("index.html?{query}");
    let verify_sample = arguments.verify_sample;
    let hold_after_verify = arguments.hold_after_verify;
    let protocol_root = book_root.clone();

    tauri::Builder::default()
        .manage(ReaderRuntime {
            diagnostics: Mutex::new(diagnostics),
            verify_sample,
            hold_after_verify,
        })
        .register_uri_scheme_protocol("atha-book", move |_context, request| {
            book_response(&protocol_root, request)
        })
        .invoke_handler(tauri::generate_handler![reader_event])
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
                    initial_window_size(screen, scale)
                })
                .unwrap_or_else(|| tao::dpi::LogicalSize::new(900.0, 900.0));
            let data_directory = app.path().app_local_data_dir()?.join("WebView2");

            WebviewWindowBuilder::new(app, "main", WebviewUrl::App(app_path.clone().into()))
                .title("Atha Reader")
                .inner_size(size.width, size.height)
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
                .on_navigation(|url| is_reader_url(url.as_str()))
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .on_download(|_, _| false)
                .prevent_overflow()
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event
                && let Ok(mut diagnostics) = app.state::<ReaderRuntime>().diagnostics.lock()
            {
                diagnostics.flush();
            }
        });
    Ok(())
}

fn is_reader_url(url: &str) -> bool {
    let Some(suffix) = url.strip_prefix(TAURI_READER_PAGE) else {
        return false;
    };
    suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
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

fn book_response(root: &BookRoot, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    if request.method() != "GET" {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    match root.read(request.uri().path()) {
        Ok(resource) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, resource.content_type)
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, TAURI_READER_ORIGIN)
            .header("x-content-type-options", "nosniff")
            .body(Cow::Owned(resource.bytes))
            .expect("valid book response"),
        Err(error) => empty_response(resource_status(error)),
    }
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
