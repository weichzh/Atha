use std::error::Error;
#[cfg(windows)]
use std::{process, time::Instant};

use atha_backend::reader::{
    READER_PAGE,
    telemetry::{MetricStage, ReaderEvent, parse_reader_event, safe_event},
};
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    ReaderRuntime, is_reader_url,
    runtime_diagnostics::{
        DiagnosticError as RuntimeDiagnosticError, Diagnostics as RuntimeDiagnostics,
        ReadyDisposition,
    },
};

#[tauri::command]
pub(crate) async fn reader_event(
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
    #[cfg(any(windows, target_os = "linux"))]
    if !crate::desktop_open::is_active_reader(&window, &url) {
        return Err("invalid-origin".into());
    }
    let diagnostics = runtime
        .reader
        .read()
        .map_err(|_| diagnostic_state_error())?
        .diagnostics
        .clone();
    let mut diagnostic_state = diagnostics.lock().map_err(|_| diagnostic_state_error())?;
    let diagnostics = diagnostic_state
        .as_mut()
        .ok_or_else(diagnostic_state_error)?;
    #[cfg(any(windows, target_os = "linux"))]
    let terminal = matches!(event, ReaderEvent::Ready(_));
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
                drop(diagnostic_state);
                #[cfg(any(windows, target_os = "linux"))]
                crate::desktop_open::reader_settled(&window, &url, false);
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
            drop(diagnostic_state);
            #[cfg(any(windows, target_os = "linux"))]
            crate::desktop_open::reader_settled(&window, &url, false);
            return Err(failure.code.into());
        }
    }
    #[cfg(any(windows, target_os = "linux"))]
    if terminal {
        drop(diagnostic_state);
        crate::desktop_open::reader_settled(&window, &url, true);
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn interactive_diagnostics() -> Result<RuntimeDiagnostics, Box<dyn Error>> {
    RuntimeDiagnostics::new(Instant::now(), false, None)
}

#[cfg(not(windows))]
pub(crate) fn interactive_diagnostics() -> Result<RuntimeDiagnostics, Box<dyn Error>> {
    Ok(RuntimeDiagnostics)
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
