//! Validation for the reader's one-way, non-content IPC messages.

use std::{error::Error, fmt};

use super::READER_PAGE;

const MAX_MESSAGE_BYTES: usize = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricStage {
    FirstStable,
    HotOpen,
    PageTurn,
    FontReflow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metric {
    pub stage: MetricStage,
    pub sample: u8,
    pub duration_ms: f64,
    pub font_size: u16,
    pub pages: u16,
    pub page_width: u16,
    pub page_height: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ready {
    pub pages: u16,
    pub inline_formulas: u16,
    pub display_formulas: u16,
    pub cuts: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Search {
    pub results: u16,
    pub truncated: bool,
    pub sections_scanned: u16,
    pub duration_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImageLoadBatch {
    pub selected: u16,
    pub success: u16,
    pub failure: u16,
    pub layout_changed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageLoadTerminal {
    pub passes: u8,
    pub remaining_current: u16,
    pub remaining_current_or_next: u16,
    pub generation_changed: bool,
    pub batches: [ImageLoadBatch; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureStage {
    Initialization,
    Opening,
    ContentLoaded,
    LayoutStable,
    Layout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReaderFailure {
    pub code: &'static str,
    pub stage: FailureStage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReaderEvent {
    Metric(Metric),
    Ready(Ready),
    Search(Search),
    ImageLoadTerminal(ImageLoadTerminal),
    Error(ReaderFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryError {
    InvalidOrigin,
    InvalidMessage,
    OutOfRange,
}

pub fn parse_reader_event(origin: &str, message: &str) -> Result<ReaderEvent, TelemetryError> {
    if !is_reader_document(origin) {
        return Err(TelemetryError::InvalidOrigin);
    }
    if message.is_empty() || message.len() > MAX_MESSAGE_BYTES || !message.is_ascii() {
        return Err(TelemetryError::InvalidMessage);
    }
    let fields = message.split('|').collect::<Vec<_>>();
    match fields.as_slice() {
        [
            "metric",
            stage,
            sample,
            duration_ms,
            font_size,
            pages,
            page_width,
            page_height,
        ] => {
            let stage = MetricStage::parse(stage).ok_or(TelemetryError::InvalidMessage)?;
            let sample = parse_range(sample, 1_u8, 10)?;
            let duration_ms = duration_ms
                .parse::<f64>()
                .map_err(|_| TelemetryError::InvalidMessage)?;
            if !duration_ms.is_finite() || !(0.0..=600_000.0).contains(&duration_ms) {
                return Err(TelemetryError::OutOfRange);
            }
            let font_size = parse_range(font_size, 8_u16, 256)?;
            let pages = parse_range(pages, 1_u16, 10_000)?;
            let page_width = parse_range(page_width, 1_u16, 16_384)?;
            let page_height = parse_range(page_height, 1_u16, 16_384)?;
            Ok(ReaderEvent::Metric(Metric {
                stage,
                sample,
                duration_ms,
                font_size,
                pages,
                page_width,
                page_height,
            }))
        }
        ["ready", pages, inline, display, cuts] => Ok(ReaderEvent::Ready(Ready {
            pages: parse_range(pages, 1_u16, 10_000)?,
            inline_formulas: parse_range(inline, 0_u16, 10_000)?,
            display_formulas: parse_range(display, 0_u16, 10_000)?,
            cuts: parse_range(cuts, 0_u16, 10_000)?,
        })),
        ["search", results, truncated, sections_scanned, duration_ms] => {
            let duration_ms = duration_ms
                .parse::<f64>()
                .map_err(|_| TelemetryError::InvalidMessage)?;
            if !duration_ms.is_finite() || !(0.0..=600_000.0).contains(&duration_ms) {
                return Err(TelemetryError::OutOfRange);
            }
            Ok(ReaderEvent::Search(Search {
                results: parse_range(results, 0_u16, 2_000)?,
                truncated: match *truncated {
                    "0" => false,
                    "1" => true,
                    _ => return Err(TelemetryError::InvalidMessage),
                },
                sections_scanned: parse_range(sections_scanned, 1_u16, 2_000)?,
                duration_ms,
            }))
        }
        [
            "image-load",
            passes,
            remaining_current,
            remaining_current_or_next,
            generation_changed,
            b1_selected,
            b1_success,
            b1_failure,
            b1_layout,
            b2_selected,
            b2_success,
            b2_failure,
            b2_layout,
            b3_selected,
            b3_success,
            b3_failure,
            b3_layout,
            b4_selected,
            b4_success,
            b4_failure,
            b4_layout,
        ] => {
            let passes = parse_range(passes, 0_u8, 4)?;
            let remaining_current = parse_range(remaining_current, 0_u16, 10_000)?;
            let remaining_current_or_next = parse_range(remaining_current_or_next, 0_u16, 10_000)?;
            let batches = [
                parse_image_load_batch(b1_selected, b1_success, b1_failure, b1_layout)?,
                parse_image_load_batch(b2_selected, b2_success, b2_failure, b2_layout)?,
                parse_image_load_batch(b3_selected, b3_success, b3_failure, b3_layout)?,
                parse_image_load_batch(b4_selected, b4_success, b4_failure, b4_layout)?,
            ];
            if remaining_current > remaining_current_or_next
                || batches.iter().enumerate().any(|(index, batch)| {
                    (index < usize::from(passes)
                        && (batch.selected == 0
                            || u32::from(batch.success) + u32::from(batch.failure)
                                > u32::from(batch.selected)))
                        || (index >= usize::from(passes) && *batch != ImageLoadBatch::default())
                })
            {
                return Err(TelemetryError::OutOfRange);
            }
            Ok(ReaderEvent::ImageLoadTerminal(ImageLoadTerminal {
                passes,
                remaining_current,
                remaining_current_or_next,
                generation_changed: parse_flag(generation_changed)?,
                batches,
            }))
        }
        ["error", code, stage] => match (allowed_error(code), FailureStage::parse(stage)) {
            (Some(code), Some(stage)) => Ok(ReaderEvent::Error(ReaderFailure { code, stage })),
            _ => Err(TelemetryError::InvalidMessage),
        },
        _ => Err(TelemetryError::InvalidMessage),
    }
}

fn parse_image_load_batch(
    selected: &str,
    success: &str,
    failure: &str,
    layout_changed: &str,
) -> Result<ImageLoadBatch, TelemetryError> {
    Ok(ImageLoadBatch {
        selected: parse_range(selected, 0_u16, 10_000)?,
        success: parse_range(success, 0_u16, 10_000)?,
        failure: parse_range(failure, 0_u16, 10_000)?,
        layout_changed: parse_flag(layout_changed)?,
    })
}

fn parse_flag(value: &str) -> Result<bool, TelemetryError> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(TelemetryError::InvalidMessage),
    }
}

pub fn safe_event(value: &str) -> &str {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        value
    } else {
        "invalid-event"
    }
}

impl FailureStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initialization => "initialization",
            Self::Opening => "opening",
            Self::ContentLoaded => "content-loaded",
            Self::LayoutStable => "layout-stable",
            Self::Layout => "layout",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "initialization" => Some(Self::Initialization),
            "opening" => Some(Self::Opening),
            "content-loaded" => Some(Self::ContentLoaded),
            "layout-stable" => Some(Self::LayoutStable),
            "layout" => Some(Self::Layout),
            _ => None,
        }
    }
}

impl MetricStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstStable => "first_stable",
            Self::HotOpen => "hot_open",
            Self::PageTurn => "page_turn",
            Self::FontReflow => "font_reflow",
        }
    }

    pub const fn mode(self) -> &'static str {
        match self {
            Self::FirstStable => "cold",
            Self::HotOpen | Self::PageTurn | Self::FontReflow => "hot",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "first_stable" => Some(Self::FirstStable),
            "hot_open" => Some(Self::HotOpen),
            "page_turn" => Some(Self::PageTurn),
            "font_reflow" => Some(Self::FontReflow),
            _ => None,
        }
    }
}

impl TelemetryError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidOrigin => "invalid-origin",
            Self::InvalidMessage => "invalid-message",
            Self::OutOfRange => "out-of-range",
        }
    }
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for TelemetryError {}

fn is_reader_document(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix(READER_PAGE) else {
        return false;
    };
    suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
}

fn parse_range<T>(value: &str, minimum: T, maximum: T) -> Result<T, TelemetryError>
where
    T: std::str::FromStr + PartialOrd,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| TelemetryError::InvalidMessage)?;
    if parsed < minimum || parsed > maximum {
        return Err(TelemetryError::OutOfRange);
    }
    Ok(parsed)
}

fn allowed_error(value: &str) -> Option<&'static str> {
    match value {
        "active-content" => Some("active-content"),
        "active-link" => Some("active-link"),
        "active-style" => Some("active-style"),
        "book-load" => Some("book-load"),
        "css-subresource" => Some("css-subresource"),
        "event-handler" => Some("event-handler"),
        "external-resource" => Some("external-resource"),
        "formula-selectors" => Some("formula-selectors"),
        "image-load" => Some("image-load"),
        "invalid-formula-size" => Some("invalid-formula-size"),
        "invalid-manifest" => Some("invalid-manifest"),
        "invalid-svg" => Some("invalid-svg"),
        "invalid-xhtml" => Some("invalid-xhtml"),
        "layout-cut" => Some("layout-cut"),
        "missing-book-url" => Some("missing-book-url"),
        "missing-stylesheet" => Some("missing-stylesheet"),
        "manifest-load" => Some("manifest-load"),
        "network-block" => Some("network-block"),
        "reader-style-load" => Some("reader-style-load"),
        "sample-boundary" => Some("sample-boundary"),
        "section-index" => Some("section-index"),
        "section-load" => Some("section-load"),
        "state-persistence" => Some("state-persistence"),
        "stylesheet-load" => Some("stylesheet-load"),
        "svg-event-handler" => Some("svg-event-handler"),
        "svg-external-resource" => Some("svg-external-resource"),
        "svg-external-style" => Some("svg-external-style"),
        "svg-load" => Some("svg-load"),
        "unstable-layout" => Some("unstable-layout"),
        "unsupported-resource-attribute" => Some("unsupported-resource-attribute"),
        "undeclared-resource" => Some("undeclared-resource"),
        _ => None,
    }
}
