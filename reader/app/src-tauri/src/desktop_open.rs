// Native forwarding owns a FIFO, not a second reader. The existing IPC and
// native navigation both publish through ReaderActivation.
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{App, AppHandle, Manager, Url, WebviewWindow};
use tauri_plugin_dialog::FilePath;
use tokio::sync::{Notify, oneshot};

use crate::{
    ReaderRuntime, ReaderSession, TAURI_LIBRARY_PAGE, TAURI_READER_PAGE,
    begin_local_data_operation,
    desktop_files::forwarded_book_paths,
    is_reader_url,
    library_commands::{ReaderActivation, ReaderLaunch, stage_library_files_async},
    message_maintenance::is_library_url,
};

mod transport;
pub(crate) use transport::register_first;

const MAX_PENDING_LAUNCHES: usize = 32;

enum LaunchRequest {
    Focus,
    Books(Vec<PathBuf>),
}

const NAVIGATION_TIMEOUT: Duration = Duration::from_secs(60);

enum Step {
    ToShelf(ReaderActivation),
    Shelf(ReaderActivation),
    Offered(ReaderActivation),
    Opening,
    Reading,
    Returning,
}

struct Handoff {
    number: u64,
    step: Step,
    prior_key: Option<String>,
    prior: ReaderSession,
}

struct ReaderAttempt {
    number: u64,
    key: String,
    activation: Option<ReaderActivation>,
}

#[derive(Default)]
struct Gate {
    pending: VecDeque<LaunchRequest>,
    overflow: usize,
    number: u64,
    stable: bool,
    on_reader: bool,
    cold_book: Option<String>,
    library_opens: usize,
    authority: Option<String>,
    reader: Option<ReaderAttempt>,
    handoff: Option<Handoff>,
}

impl Gate {
    fn number(&mut self) -> u64 {
        self.number += 1;
        self.number
    }

    fn can_deliver(&self) -> bool {
        self.stable
            && self.cold_book.is_none()
            && self.library_opens == 0
            && self.reader.is_none()
            && self.handoff.is_none()
    }
}

#[derive(Default)]
struct DesktopOpen {
    gate: Mutex<Gate>,
    change: Notify,
}

impl DesktopOpen {
    fn gate(&self) -> MutexGuard<'_, Gate> {
        self.gate.lock().expect("desktop-open-state")
    }

    async fn next_request(&self) -> LaunchRequest {
        loop {
            {
                let mut gate = self.gate();
                if gate.overflow != 0 {
                    log::warn!(
                        target: "atha::startup",
                        "operation=desktop_open outcome=rejected code=queue-full count={}",
                        std::mem::take(&mut gate.overflow)
                    );
                }
                if gate.can_deliver()
                    && let Some(request) = gate.pending.pop_front()
                {
                    return request;
                }
            }
            self.change.notified().await;
        }
    }

    async fn wait_until_ready(&self) {
        while !self.gate().can_deliver() {
            self.change.notified().await;
        }
    }
}

fn receive(app: &AppHandle, argv: Vec<String>, cwd: String) {
    let state = app.state::<DesktopOpen>();
    let request = if argv.len() == 1 {
        LaunchRequest::Focus
    } else if let Some(paths) = forwarded_book_paths(
        argv.into_iter().skip(1).map(Into::into),
        Some(PathBuf::from(cwd).as_path()),
    ) {
        LaunchRequest::Books(paths)
    } else {
        log::warn!(
            target: "atha::startup",
            "operation=desktop_open outcome=rejected code=invalid-library-import"
        );
        return;
    };
    let mut gate = state.gate();
    if gate.pending.len() == MAX_PENDING_LAUNCHES {
        gate.overflow = gate.overflow.saturating_add(1);
    } else {
        gate.pending.push_back(request);
    }
    drop(gate);
    state.change.notify_one();
}

pub(crate) fn prioritize_cold_start(app: &App, book_id: Option<String>) {
    if let Some(state) = app.try_state::<DesktopOpen>() {
        state.gate().cold_book = book_id;
    }
}

pub(crate) fn start_worker(app: &App) {
    if app.try_state::<DesktopOpen>().is_some() {
        let handle = app.handle().clone();
        if let Some(window) = app.get_webview_window("main") {
            watch(&window, 0, Watch::Cold);
        }
        tauri::async_runtime::spawn(async move { deliver(handle).await });
    }
}

async fn deliver(app: AppHandle) {
    let state = app.state::<DesktopOpen>();
    let runtime = app.state::<ReaderRuntime>();
    loop {
        let request = state.next_request().await;
        let Some(window) = app.get_webview_window("main") else {
            report("failed", "window-unavailable");
            return;
        };
        focus(&window);
        let LaunchRequest::Books(paths) = request else {
            continue;
        };
        let started = Instant::now();
        let prepared = async {
            let _operation = begin_local_data_operation(&runtime)?;
            let selected = paths.into_iter().map(FilePath::Path).collect();
            let staged =
                stage_library_files_async(app.clone(), runtime.library.clone(), selected).await?;
            let opened = staged.first_opened.ok_or("no-prepared-book")?;
            ReaderActivation::new(opened, &started)
        }
        .await;
        let mut prepared = match prepared {
            Ok(prepared) => prepared,
            Err(code) => {
                report("failed", &code);
                continue;
            }
        };
        loop {
            // A user open or return may have started while the disk worker was
            // preparing. Keep this result ahead of all later native requests.
            state.wait_until_ready().await;
            let (tx, rx) = oneshot::channel();
            let window = window.clone();
            if app
                .run_on_main_thread(move || {
                    let _ = tx.send(deliver_on_main(&window, prepared));
                })
                .is_err()
            {
                report("failed", "window-unavailable");
                break;
            }
            match rx.await {
                Ok(Some(deferred)) => prepared = deferred,
                Ok(None) => break,
                Err(_) => {
                    report("failed", "window-unavailable");
                    break;
                }
            }
        }
    }
}

fn focus(window: &WebviewWindow) {
    let _ = window.show();
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.set_focus();
}

fn deliver_on_main(window: &WebviewWindow, prepared: ReaderActivation) -> Option<ReaderActivation> {
    let state = window.state::<DesktopOpen>();
    let runtime = window.state::<ReaderRuntime>();
    let Ok(current) = window.url() else {
        report("failed", "reader-url");
        return None;
    };
    let mut gate = state.gate();
    if !gate.can_deliver() {
        return Some(prepared);
    }
    let Ok(prior) = runtime.reader.read().map(|reader| reader.clone()) else {
        report("failed", "reader-state");
        return None;
    };
    let prior_key = reader_key(&current).filter(|key| Some(key) == gate.authority.as_ref());
    if prior_key.is_some()
        && prior
            .edition
            .as_ref()
            .is_some_and(|edition| edition.content_version == prepared.book_id())
    {
        report("unchanged", "already-reading");
        return None;
    }
    // Do not keep a shared data guard across the shelf's exclusive recovery check.
    if let Err(code) = begin_local_data_operation(&runtime) {
        report("failed", &code);
        return None;
    }
    let number = gate.number();
    gate.handoff = Some(Handoff {
        number,
        step: Step::ToShelf(prepared),
        prior_key,
        prior,
    });
    drop(gate);
    log::info!(target: "atha::startup", "operation=desktop_open stage=prepared");
    if window.navigate(shelf_url()).is_err() {
        finish_handoff(window, number, "failed", "reader-navigation");
    } else {
        watch(window, number, Watch::Handoff);
    }
    None
}

fn shelf_url() -> Url {
    Url::parse(TAURI_LIBRARY_PAGE).expect("canonical shelf URL")
}

fn reader_url(key: &str) -> Url {
    let mut url = Url::parse(TAURI_READER_PAGE).expect("canonical reader URL");
    url.query_pairs_mut()
        .append_pair("manifest", atha_backend::reader::epub::READER_MANIFEST)
        .append_pair("state", key);
    url
}

fn reader_key(url: &Url) -> Option<String> {
    if !is_reader_url(url.as_str()) {
        return None;
    }
    let (mut key, mut manifest) = (None, false);
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "state"
                if key.is_none()
                    && value.len() == 16
                    && value.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
            {
                key = Some(value.into_owned());
            }
            "manifest" if !manifest && value == atha_backend::reader::epub::READER_MANIFEST => {
                manifest = true;
            }
            "state" | "manifest" | "entry" | "book" => return None,
            _ => {}
        }
    }
    manifest.then_some(key).flatten()
}

pub(crate) fn allow_navigation(app: &AppHandle, url: &Url) -> bool {
    let Some(state) = app.try_state::<DesktopOpen>() else {
        return true;
    };
    let gate = state.gate();
    if is_library_url(url.as_str()) {
        return true;
    }
    let Some(key) = reader_key(url) else {
        return false;
    };
    if let Some(handoff) = gate.handoff.as_ref() {
        return match handoff.step {
            Step::Reading => gate.reader.as_ref().is_some_and(|reader| reader.key == key),
            Step::Returning => handoff.prior_key.as_ref() == Some(&key),
            _ => false,
        };
    }
    if gate.reader.as_ref().map_or_else(
        || gate.authority.as_ref() == Some(&key) && published_book(app, &key),
        |reader| reader.key == key,
    ) {
        return true;
    }
    // An old reader must never be displayed against the active book's root.
    // If Back came from a reader (e.g. after rollback), send it to the shelf.
    let return_to_shelf = gate.on_reader && gate.stable;
    drop(gate);
    if return_to_shelf {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.navigate(shelf_url());
            }
        });
    }
    false
}

fn published_book(app: &AppHandle, key: &str) -> bool {
    app.try_state::<ReaderRuntime>().is_some_and(|runtime| {
        runtime.reader.read().is_ok_and(|reader| {
            reader.root.is_some()
                && reader
                    .edition
                    .as_ref()
                    .is_some_and(|edition| edition.content_version.get(..16) == Some(key))
        })
    })
}

pub(crate) fn is_active_reader(window: &WebviewWindow, url: &Url) -> bool {
    window.try_state::<DesktopOpen>().is_none_or(|state| {
        let gate = state.gate();
        gate.on_reader
            && reader_key(url).is_some_and(|key| {
                gate.authority.as_ref() == Some(&key) && published_book(window.app_handle(), &key)
            })
    })
}

pub(crate) fn on_page_load(window: WebviewWindow, page: PageLoadPayload<'_>) {
    let Some(state) = window.try_state::<DesktopOpen>() else {
        return;
    };
    let shelf = is_library_url(page.url().as_str());
    let key = reader_key(page.url());
    let mut gate = state.gate();
    if page.event() == PageLoadEvent::Finished {
        if let Some(handoff) = gate.handoff.as_ref()
            && matches!(handoff.step, Step::Returning)
            && shelf
            && handoff.prior_key.is_none()
        {
            gate.handoff = None;
            gate.stable = true;
        } else if gate.reader.is_none() && gate.handoff.is_none() {
            gate.stable = shelf
                || key
                    .as_ref()
                    .is_some_and(|key| gate.authority.as_ref() == Some(key));
        }
        state.change.notify_one();
        return;
    }
    gate.stable = false;
    gate.on_reader = !shelf;
    if let Some(handoff) = gate.handoff.as_ref()
        && matches!(handoff.step, Step::Returning)
        && (shelf && handoff.prior_key.is_none() || key.is_some() && key == handoff.prior_key)
    {
        let authority = handoff.prior_key.clone();
        let prior = handoff.prior.clone();
        if let Ok(mut reader) = window.state::<ReaderRuntime>().reader.write() {
            *reader = prior;
            gate.authority = authority;
        }
        return;
    }
    if shelf {
        if let Some(handoff) = gate.handoff.as_mut() {
            if matches!(handoff.step, Step::ToShelf(_)) {
                let Step::ToShelf(prepared) = std::mem::replace(&mut handoff.step, Step::Opening)
                else {
                    unreachable!();
                };
                handoff.step = Step::Shelf(prepared);
            } else if matches!(handoff.step, Step::Reading) {
                let number = handoff.number;
                drop(gate);
                dispatch_abort(&window, number, "navigation-cancelled");
            }
        } else if gate.reader.take().is_some() {
            state.change.notify_one();
        }
        return;
    }
    let Some(reader) = gate
        .reader
        .as_mut()
        .filter(|reader| key.as_ref() == Some(&reader.key))
    else {
        return;
    };
    if let Some(activation) = reader.activation.take() {
        let runtime = window.state::<ReaderRuntime>();
        // The IPC that offered this book already released its guard. Recheck
        // at actual publication, without keeping anything across navigation.
        let result = begin_local_data_operation(&runtime)
            .and_then(|_operation| activation.activate(&runtime, &window));
        if let Err(code) = result {
            let number = gate.handoff.as_ref().map(|handoff| handoff.number);
            gate.reader = None;
            gate.authority = None;
            // The new page has committed but is not authorized: never let it
            // fetch the previous edition while rollback is being dispatched.
            if let Ok(mut active) = runtime.reader.write() {
                *active = ReaderSession::default();
            }
            drop(gate);
            if let Some(number) = number {
                dispatch_abort(&window, number, "reader-activation-rejected");
            } else {
                report("failed", &code);
                let _ = window.navigate(shelf_url());
            }
        } else {
            gate.authority = key;
            if gate.handoff.is_some() {
                log::info!(target: "atha::startup", "operation=desktop_open stage=activated");
            }
        }
    }
}

pub(crate) fn observe_shelf_startup(window: &WebviewWindow, can_open: impl FnOnce(&str) -> bool) {
    let Some(state) = window.try_state::<DesktopOpen>() else {
        return;
    };
    let mut gate = state.gate();
    if let Some(handoff) = gate.handoff.as_ref() {
        if let Step::Shelf(prepared) = &handoff.step
            && !can_open(prepared.book_id())
        {
            let number = handoff.number;
            drop(gate);
            dispatch_abort(window, number, "shelf-not-ready");
        }
    } else if let Some(cold) = gate.cold_book.take() {
        if can_open(&cold) {
            gate.cold_book = Some(cold);
        } else {
            drop(gate);
            clear_cold_start(window);
            state.change.notify_one();
        }
    }
}

fn clear_cold_start(window: &WebviewWindow) {
    if let Ok(mut startup) = window.state::<ReaderRuntime>().startup_import.lock() {
        startup.take();
    }
    log::warn!(target: "atha::startup", "operation=cold_file_open outcome=failed code=shelf-not-ready");
}

pub(crate) fn take_shelf_handoff(window: &WebviewWindow) -> Option<String> {
    let state = window.try_state::<DesktopOpen>()?;
    let mut gate = state.gate();
    let handoff = gate.handoff.as_mut()?;
    if !matches!(handoff.step, Step::Shelf(_)) {
        return None;
    }
    let Step::Shelf(prepared) = std::mem::replace(&mut handoff.step, Step::Opening) else {
        unreachable!();
    };
    let id = prepared.book_id().to_owned();
    handoff.step = Step::Offered(prepared);
    Some(id)
}

pub(crate) fn reader_settled(window: &WebviewWindow, url: &Url, ready: bool) {
    let Some(state) = window.try_state::<DesktopOpen>() else {
        return;
    };
    let mut gate = state.gate();
    let key = reader_key(url);
    if !gate.on_reader || key.is_none() || key != gate.authority {
        return;
    }
    if let Some(handoff) = gate.handoff.as_ref() {
        let number = handoff.number;
        match handoff.step {
            Step::Reading
                if gate.reader.as_ref().is_some_and(|reader| {
                    key.as_ref() == Some(&reader.key) && reader.activation.is_none()
                }) =>
            {
                drop(gate);
                if ready {
                    finish_handoff(window, number, "ok", "reader-ready");
                } else {
                    dispatch_abort(window, number, "first-read-failed");
                }
            }
            Step::Returning if key == handoff.prior_key => {
                gate.handoff = None;
                gate.stable = true;
                state.change.notify_one();
                log::info!(target: "atha::startup", "operation=desktop_open stage=rollback outcome={}",
                    if ready { "ok" } else { "failed" });
            }
            _ => {}
        }
    } else if gate
        .reader
        .as_ref()
        .is_none_or(|reader| key.as_ref() == Some(&reader.key) && reader.activation.is_none())
    {
        gate.reader = None;
        gate.stable = true;
        state.change.notify_one();
    }
}

pub(crate) struct LibraryOpen<'a> {
    state: tauri::State<'a, DesktopOpen>,
    window: &'a WebviewWindow,
    id: &'a str,
    handoff: Option<u64>,
    completed: bool,
    pub(crate) prepared: Option<ReaderActivation>,
}

impl LibraryOpen<'_> {
    pub(crate) fn expect_reader(
        &mut self,
        activation: ReaderActivation,
    ) -> Result<ReaderLaunch, String> {
        let launch = activation.launch();
        let target = Url::parse(TAURI_READER_PAGE)
            .and_then(|root| root.join(&launch.href))
            .map_err(|_| "reader-url".to_owned())?;
        let key = reader_key(&target).ok_or("reader-url")?;
        let mut gate = self.state.gate();
        if let Some(number) = self.handoff {
            let Some(handoff) = gate.handoff.as_mut().filter(|handoff| {
                handoff.number == number && matches!(handoff.step, Step::Opening)
            }) else {
                return Err("reader-open-cancelled".into());
            };
            handoff.step = Step::Reading;
        }
        let number = gate.number();
        gate.reader = Some(ReaderAttempt {
            number,
            key,
            activation: Some(activation),
        });
        self.completed = true;
        drop(gate);
        if self.handoff.is_none() {
            watch(self.window, number, Watch::Reader);
        }
        Ok(launch)
    }
}

impl Drop for LibraryOpen<'_> {
    fn drop(&mut self) {
        let mut gate = self.state.gate();
        gate.library_opens -= 1;
        if gate.cold_book.as_deref() == Some(self.id) {
            gate.cold_book = None;
        }
        drop(gate);
        if !self.completed
            && let Some(number) = self.handoff
        {
            dispatch_abort(self.window, number, "reader-open-rejected");
        }
        self.state.change.notify_one();
    }
}

pub(crate) fn begin_library_open<'a>(
    window: &'a WebviewWindow,
    id: &'a str,
) -> Result<Option<LibraryOpen<'a>>, String> {
    let Some(state) = window.try_state::<DesktopOpen>() else {
        return Ok(None);
    };
    let mut gate = state.gate();
    if gate.library_opens != 0 || gate.reader.is_some() {
        return Err("reader-open-busy".into());
    }
    let mut native = None;
    let prepared = if let Some(handoff) = gate.handoff.as_mut() {
        let Step::Offered(prepared) = &handoff.step else {
            return Err("reader-open-busy".into());
        };
        if prepared.book_id() != id {
            return Err("reader-open-busy".into());
        }
        native = Some(handoff.number);
        let Step::Offered(prepared) = std::mem::replace(&mut handoff.step, Step::Opening) else {
            unreachable!();
        };
        Some(prepared)
    } else {
        None
    };
    gate.library_opens += 1;
    drop(gate);
    Ok(Some(LibraryOpen {
        state,
        window,
        id,
        handoff: native,
        completed: false,
        prepared,
    }))
}

fn finish_handoff(window: &WebviewWindow, number: u64, outcome: &str, code: &str) {
    let state = window.state::<DesktopOpen>();
    let mut gate = state.gate();
    if gate
        .handoff
        .as_ref()
        .is_some_and(|handoff| handoff.number == number && !matches!(handoff.step, Step::Returning))
    {
        gate.handoff = None;
        gate.reader = None;
        gate.stable = true;
        drop(gate);
        report(outcome, code);
        state.change.notify_one();
    }
}

fn dispatch_abort(window: &WebviewWindow, number: u64, code: &'static str) {
    let main = window.clone();
    let _ = window.run_on_main_thread(move || abort(&main, number, code));
}

fn abort(window: &WebviewWindow, number: u64, code: &'static str) {
    let state = window.state::<DesktopOpen>();
    let mut gate = state.gate();
    let Some(handoff) = gate
        .handoff
        .as_mut()
        .filter(|handoff| handoff.number == number && !matches!(handoff.step, Step::Returning))
    else {
        return;
    };
    handoff.step = Step::Returning;
    let target = handoff
        .prior_key
        .as_ref()
        .map_or_else(shelf_url, |key| reader_url(key));
    gate.reader = None;
    gate.stable = false;
    drop(gate);
    report("failed", code);
    if window.navigate(target).is_err() {
        release_rollback(window, number, "rollback-navigation");
    } else {
        watch(window, number, Watch::Rollback);
    }
}

fn release_rollback(window: &WebviewWindow, number: u64, code: &str) {
    let state = window.state::<DesktopOpen>();
    let mut gate = state.gate();
    if gate
        .handoff
        .as_ref()
        .is_some_and(|handoff| handoff.number == number && matches!(handoff.step, Step::Returning))
    {
        gate.handoff = None;
        gate.stable = true;
        drop(gate);
        log::warn!(target: "atha::startup", "operation=desktop_open stage=rollback outcome=failed code={code}");
        state.change.notify_one();
    }
}

#[derive(Clone, Copy)]
enum Watch {
    Cold,
    Reader,
    Handoff,
    Rollback,
}

fn watch(window: &WebviewWindow, number: u64, phase: Watch) {
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(NAVIGATION_TIMEOUT).await;
        let main = window.clone();
        let _ = window.run_on_main_thread(move || {
            let state = main.state::<DesktopOpen>();
            match phase {
                Watch::Cold => {
                    if state.gate().cold_book.take().is_some() {
                        clear_cold_start(&main);
                        state.change.notify_one();
                    }
                }
                Watch::Reader => {
                    let mut gate = state.gate();
                    if gate
                        .reader
                        .as_ref()
                        .is_some_and(|reader| reader.number == number)
                    {
                        gate.reader = None;
                        gate.stable = true;
                        state.change.notify_one();
                    }
                }
                Watch::Handoff => abort(&main, number, "navigation-timeout"),
                Watch::Rollback => release_rollback(&main, number, "rollback-timeout"),
            }
        });
    });
}

fn report(outcome: &str, code: &str) {
    log::info!(target: "atha::startup", "operation=desktop_open outcome={outcome} code={code}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_waits_for_cold_and_manual_navigation() {
        let mut gate = Gate::default();
        assert!(!gate.can_deliver());
        gate.stable = true;
        gate.cold_book = Some("cold".into());
        assert!(!gate.can_deliver());
        gate.library_opens = 1;
        gate.cold_book = None;
        assert!(!gate.can_deliver());
        gate.reader = Some(ReaderAttempt {
            number: 1,
            key: "next".into(),
            activation: None,
        });
        gate.library_opens = 0;
        assert!(!gate.can_deliver());
        gate.reader = None;
        assert!(gate.can_deliver());
    }

    #[test]
    fn library_authority_excludes_mixed_or_repeated_book_sources() {
        let key = "fedcba9876543210";
        let url = reader_url(key);
        assert_eq!(reader_key(&url).as_deref(), Some(key));
        for query in [
            "state=1111111111111111",
            "manifest=bogus",
            "entry=wrong.xhtml",
            "book=other",
        ] {
            assert!(reader_key(&Url::parse(&format!("{url}&{query}")).unwrap()).is_none());
        }
    }
}
