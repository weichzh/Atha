mod diagnostics;
mod launch;
mod protocol;

use std::{error::Error, process, time::Instant};

use atha_backend::reader::{
    resources::BookRoot,
    telemetry::{ReaderEvent, TelemetryError, parse_reader_event},
};
use diagnostics::{DiagnosticError, Diagnostics, ReadyDisposition, safe_event};
use launch::{Arguments, initial_window_size, reader_url};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Window, WindowBuilder},
};
use wry::{NewWindowResponse, PermissionResponse, WebViewBuilder, WebViewBuilderExtWindows};

enum UserEvent {
    Reader(Result<ReaderEvent, TelemetryError>),
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let startup = Instant::now();
    let arguments = Arguments::parse()?;
    let book_root = BookRoot::new(&arguments.book_root)?;
    book_root.read(&format!("/{}", arguments.entry))?;
    let mut diagnostics = Diagnostics::new(
        startup,
        arguments.verify_sample,
        arguments.benchmark.as_ref(),
    )?;

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window_size = event_loop
        .primary_monitor()
        .map(|monitor| {
            let scale_factor = monitor.scale_factor();
            let screen = monitor.size().to_logical::<f64>(scale_factor);
            initial_window_size(screen, scale_factor)
        })
        .unwrap_or_else(|| tao::dpi::LogicalSize::new(900.0, 900.0));
    let window = WindowBuilder::new()
        .with_title("Atha Reader")
        .with_inner_size(window_size)
        .build(&event_loop)?;
    let proxy = event_loop.create_proxy();
    let url = reader_url(&arguments, diagnostics.network_probe());
    let book_resources = book_root.clone();

    let builder = WebViewBuilder::new()
        .with_custom_protocol("atha".into(), move |_id, request| {
            protocol::app_response(request)
        })
        .with_custom_protocol("atha-book".into(), move |_id, request| {
            protocol::book_response(&book_resources, request)
        })
        .with_url(url)
        .with_incognito(true)
        .with_devtools(false)
        .with_clipboard(false)
        .with_general_autofill_enabled(false)
        .with_hotkeys_zoom(false)
        .with_drag_drop_handler(|_| true)
        .with_navigation_handler(protocol::is_reader_url)
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_ipc_handler(move |request| {
            let result = parse_reader_event(&request.uri().to_string(), request.body());
            let _ = proxy.send_event(UserEvent::Reader(result));
        })
        .with_browser_accelerator_keys(false)
        .with_default_context_menus(false)
        .with_https_scheme(true);
    let _webview = builder.build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Reader(Ok(ReaderEvent::Metric(metric)))) => {
                if let Err(error) = diagnostics.record_metric(metric) {
                    handle_diagnostic_error(
                        &mut diagnostics,
                        &window,
                        error,
                        arguments.verify_sample,
                    );
                }
            }
            Event::UserEvent(UserEvent::Reader(Ok(ReaderEvent::Ready(ready)))) => {
                match diagnostics.record_ready(ready) {
                    Ok(ReadyDisposition::VerificationComplete) => process::exit(0),
                    Ok(ReadyDisposition::Interactive) => window.set_title("Atha Reader"),
                    Err(error) => handle_diagnostic_error(
                        &mut diagnostics,
                        &window,
                        error,
                        arguments.verify_sample,
                    ),
                }
            }
            Event::UserEvent(UserEvent::Reader(Ok(ReaderEvent::Error(code)))) => {
                fail_run(&mut diagnostics, &window, code, arguments.verify_sample);
            }
            Event::UserEvent(UserEvent::Reader(Err(error))) => {
                fail_run(
                    &mut diagnostics,
                    &window,
                    error.code(),
                    arguments.verify_sample,
                );
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                diagnostics.flush();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn handle_diagnostic_error(
    diagnostics: &mut Diagnostics,
    window: &Window,
    error: DiagnosticError,
    exit: bool,
) {
    match error {
        DiagnosticError::Reader(code) => fail_run(diagnostics, window, code, exit),
        DiagnosticError::Recorder(message, error) => {
            eprintln!("{message}: {error}");
            process::exit(1);
        }
    }
}

fn fail_run(diagnostics: &mut Diagnostics, window: &Window, code: &str, exit: bool) {
    diagnostics.record_failure(code);
    window.set_title("Atha Reader - Error");
    if exit {
        eprintln!("reader self-check failed: {}", safe_event(code));
        process::exit(1);
    }
}
