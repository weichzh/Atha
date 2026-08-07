#[cfg(windows)]
pub(crate) use atha_reader_host::diagnostics::{DiagnosticError, Diagnostics, ReadyDisposition};

#[cfg(not(windows))]
use atha_backend::reader::telemetry::{Metric, Ready};

#[cfg(not(windows))]
pub(crate) type DiagnosticError = std::convert::Infallible;

#[cfg(not(windows))]
pub(crate) enum ReadyDisposition {
    Interactive,
}

#[cfg(not(windows))]
#[derive(Default)]
pub(crate) struct Diagnostics;

#[cfg(not(windows))]
impl Diagnostics {
    pub(crate) fn record_metric(&mut self, _metric: Metric) -> Result<(), DiagnosticError> {
        Ok(())
    }

    pub(crate) fn record_ready(
        &mut self,
        _ready: Ready,
    ) -> Result<ReadyDisposition, DiagnosticError> {
        Ok(ReadyDisposition::Interactive)
    }

    pub(crate) fn record_failure(&mut self, _code: &str) {}

    pub(crate) fn flush(&mut self) {}
}
