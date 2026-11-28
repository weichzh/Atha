//! Freeze the outgoing paper in a native GTK sibling; the sibling is not part
//! of a WebKit snapshot, so the incoming paper can be captured without a flash.
use std::{cell::RefCell, rc::Rc, time::Duration};

use gtk::{
    cairo::{Format, ImageSurface},
    glib,
    prelude::*,
};
use serde::Deserialize;
use tauri::{WebviewWindow, ipc::Response};
use webkit2gtk::{LoadEvent, WebView, WebViewExt};

const CAPTURE_LIMIT: Duration = Duration::from_millis(1250);
const HANDOFF_LIMIT: Duration = Duration::from_millis(700);
type FrameReply = tokio::sync::oneshot::Sender<Result<Vec<u8>, &'static str>>;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Frame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_width: f64,
    viewport_height: f64,
}

impl Frame {
    fn valid(self, view: &WebView) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.viewport_width,
            self.viewport_height,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width >= 1.0
            && self.height >= 1.0
            && self.x + self.width <= self.viewport_width + 0.5
            && self.y + self.height <= self.viewport_height + 0.5
            && (self.viewport_width - f64::from(view.allocated_width())).abs() <= 2.0
            && (self.viewport_height - f64::from(view.allocated_height())).abs() <= 2.0
    }

    fn take_pixels(self, view: &WebView) -> Option<ImageSurface> {
        let scale = f64::from(view.scale_factor().clamp(1, 2));
        let width = (self.width * scale).round() as i32;
        let height = (self.height * scale).round() as i32;
        if width < 1 || height < 1 || i64::from(width) * i64::from(height) > 9_000_000 {
            return None;
        }
        let crop = ImageSurface::create(Format::Rgb24, width, height).ok()?;
        let cr = gtk::cairo::Context::new(&crop).ok()?;
        cr.scale(scale, scale);
        cr.translate(-self.x, -self.y);
        // Capture the displayed WebView backing store, including already
        // composited SVG formula images. The native cover is a sibling.
        view.draw(&cr);
        cr.status().ok()?;
        Some(crop)
    }
}

#[derive(Deserialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub(crate) enum Capture {
    Outgoing { token: u32, frame: Frame },
    Incoming { token: u32 },
    Handoff { token: u32 },
    Release { token: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Capturing,
    PreparingTexture,
    Revealing,
}

struct Turn {
    token: u32,
    frame: Frame,
    frozen: Option<ImageSurface>,
    waiting_for_paint: Option<(FrameReply, Vec<u8>)>,
    phase: Phase,
}

#[derive(Clone)]
struct Layer {
    widget: gtk::DrawingArea,
    turn: Rc<RefCell<Option<Turn>>>,
}

impl Layer {
    fn release(&self, token: u32) {
        let mut current = self.turn.borrow_mut();
        if current.as_ref().is_some_and(|turn| turn.token == token) {
            current.take();
            self.widget.hide();
        }
    }

    fn expire_after(&self, token: u32, phase: Phase, delay: Duration) {
        let layer = self.clone();
        glib::timeout_add_local_once(delay, move || {
            let expired = layer
                .turn
                .borrow()
                .as_ref()
                .is_some_and(|turn| turn.token == token && turn.phase == phase);
            if expired {
                layer.release(token);
            }
        });
    }
}

thread_local! {
    static LAYER: RefCell<Option<Layer>> = const { RefCell::new(None) };
}

pub(crate) fn install(window: &WebviewWindow) -> tauri::Result<()> {
    window.with_webview(|platform| {
        let view = platform.inner();
        let Some(container) = view.parent().and_downcast::<gtk::Box>() else {
            return;
        };
        let overlay = gtk::Overlay::new();
        let widget = gtk::DrawingArea::new();
        widget.set_halign(gtk::Align::Fill);
        widget.set_valign(gtk::Align::Fill);
        overlay.add_overlay(&widget);
        overlay.set_overlay_pass_through(&widget, true);
        let turn = Rc::new(RefCell::new(None::<Turn>));
        let pixels = turn.clone();
        widget.connect_draw(move |_, cr| {
            if let Some(frozen) = pixels.borrow_mut().as_mut()
                && let Some(image) = frozen.frozen.as_ref()
            {
                let frame = frozen.frame;
                cr.rectangle(frame.x, frame.y, frame.width, frame.height);
                cr.clip();
                cr.translate(frame.x, frame.y);
                cr.scale(
                    frame.width / f64::from(image.width()),
                    frame.height / f64::from(image.height()),
                );
                if cr.set_source_surface(image, 0.0, 0.0).is_ok()
                    && cr.paint().is_ok()
                    && let Some((sender, bytes)) = frozen.waiting_for_paint.take()
                {
                    // Moving the live scrollport before this paint can flash the target.
                    glib::idle_add_local_once(move || {
                        let _ = sender.send(Ok(bytes));
                    });
                }
            }
            glib::Propagation::Proceed
        });
        // Set up once, before the reader can ask for pixels.
        container.remove(&view);
        overlay.add(&view);
        container.pack_start(&overlay, true, true, 0);
        view.show();
        overlay.show();
        widget.hide();
        let layer = Layer { widget, turn };
        let on_navigation = layer.clone();
        view.connect_load_changed(move |_, event| {
            if event == LoadEvent::Started {
                let token = on_navigation.turn.borrow().as_ref().map(|turn| turn.token);
                if let Some(token) = token {
                    on_navigation.release(token);
                }
            }
        });
        LAYER.with_borrow_mut(|slot| *slot = Some(layer));
    })
}

#[tauri::command]
pub(crate) async fn page_curl_frame(
    window: WebviewWindow,
    capture: Capture,
) -> Result<Response, &'static str> {
    let url = window.url().map_err(|_| "reader-url")?;
    if window.label() != "main"
        || !super::is_reader_url(url.as_str())
        || !url
            .query_pairs()
            .any(|(key, _)| matches!(key.as_ref(), "book" | "manifest" | "entry"))
    {
        return Err("invalid-origin");
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .with_webview(move |platform| {
            let Some(layer) = LAYER.with_borrow(Clone::clone) else {
                let _ = sender.send(Err("capture-unavailable"));
                return;
            };
            let view = platform.inner();
            let (token, frame, outgoing) = match capture {
                Capture::Release { token } => {
                    layer.release(token);
                    let _ = sender.send(Ok(Vec::new()));
                    return;
                }
                Capture::Handoff { token } => {
                    let mut turn = layer.turn.borrow_mut();
                    let result = if let Some(active) = turn
                        .as_mut()
                        .filter(|turn| turn.token == token && turn.phase == Phase::PreparingTexture)
                    {
                        active.phase = Phase::Revealing;
                        layer.expire_after(token, Phase::Revealing, HANDOFF_LIMIT);
                        Ok(Vec::new())
                    } else {
                        Err("capture-expired")
                    };
                    let _ = sender.send(result);
                    return;
                }
                Capture::Outgoing { token, frame } => {
                    if !frame.valid(&view) || !view.is_mapped() || layer.turn.borrow().is_some() {
                        let _ = sender.send(Err("capture-unavailable"));
                        return;
                    }
                    layer.turn.replace(Some(Turn {
                        token,
                        frame,
                        frozen: None,
                        waiting_for_paint: None,
                        phase: Phase::Capturing,
                    }));
                    layer.expire_after(token, Phase::Capturing, CAPTURE_LIMIT);
                    (token, frame, true)
                }
                Capture::Incoming { token } => {
                    let Some(frame) = layer
                        .turn
                        .borrow()
                        .as_ref()
                        .filter(|t| {
                            t.token == token && t.phase == Phase::Capturing && t.frozen.is_some()
                        })
                        .map(|t| t.frame)
                    else {
                        let _ = sender.send(Err("capture-expired"));
                        return;
                    };
                    if !frame.valid(&view) || !view.is_mapped() {
                        let _ = sender.send(Err("capture-unavailable"));
                        return;
                    }
                    (token, frame, false)
                }
            };
            let Some(image) = frame.take_pixels(&view) else {
                let _ = sender.send(Err("capture-failed"));
                return;
            };
            let Some(bytes) =
                gtk::gdk::pixbuf_get_from_surface(&image, 0, 0, image.width(), image.height())
                    .and_then(|im| im.save_to_bufferv("png", &[("compression", "1")]).ok())
            else {
                let _ = sender.send(Err("capture-failed"));
                return;
            };
            let mut turn = layer.turn.borrow_mut();
            let Some(active) = turn.as_mut().filter(|t| t.token == token) else {
                let _ = sender.send(Err("capture-expired"));
                return;
            };
            if outgoing {
                active.frozen = Some(image);
                active.waiting_for_paint = Some((sender, bytes));
                layer.widget.show();
                layer.widget.queue_draw();
            } else {
                // Keep the outgoing picture visible while WebKit decodes
                // and uploads both textures, including the cold GPU path.
                active.phase = Phase::PreparingTexture;
                layer.expire_after(token, Phase::PreparingTexture, CAPTURE_LIMIT);
                let _ = sender.send(Ok(bytes));
            }
        })
        .map_err(|_| "capture-unavailable")?;
    tokio::time::timeout(CAPTURE_LIMIT, receiver)
        .await
        .map_err(|_| "capture-expired")?
        .map_err(|_| "capture-expired")?
        .map(Response::new)
}
