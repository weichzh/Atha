use std::{env, error::Error, ffi::OsString, net::TcpListener, path::PathBuf};

use tao::dpi::LogicalSize;

pub(super) const APP_PAGE: &str = "https://atha.localhost/atha-reader.html";
const PAGE_DEVICE_WIDTH: f64 = 1264.0;
const PAGE_DEVICE_HEIGHT: f64 = 1680.0;
const WINDOW_PADDING_LOGICAL: f64 = 48.0;
const WINDOW_FRAME_ALLOWANCE_LOGICAL: f64 = 48.0;
const MAX_SCREEN_FRACTION: f64 = 0.8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BenchmarkMode {
    Cold,
    Hot,
}

impl BenchmarkMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Hot => "hot",
        }
    }
}

pub(super) struct Benchmark {
    pub(super) run_id: String,
    pub(super) process_sample: u8,
    pub(super) mode: BenchmarkMode,
}

pub(super) struct Arguments {
    pub(super) book_root: PathBuf,
    pub(super) entry: String,
    pub(super) verify_sample: bool,
    pub(super) benchmark: Option<Benchmark>,
}

impl Arguments {
    pub(super) fn parse() -> Result<Self, Box<dyn Error>> {
        let mut values = env::args_os().skip(1);
        let mut book_root = None;
        let mut entry = None;
        let mut verify_sample = false;
        let mut run_id = None;
        let mut process_sample = None;
        let mut mode = None;
        while let Some(flag) = values.next() {
            match flag.to_str() {
                Some("--book-root") => book_root = Some(required(&mut values, "book root")?.into()),
                Some("--entry") => {
                    entry = Some(
                        required(&mut values, "entry")?
                            .into_string()
                            .map_err(|_| "entry must be Unicode")?,
                    )
                }
                Some("--verify-sample") => verify_sample = true,
                Some("--benchmark-run") => {
                    run_id = Some(
                        required(&mut values, "benchmark run")?
                            .into_string()
                            .map_err(|_| "benchmark run must be Unicode")?,
                    )
                }
                Some("--sample") => {
                    process_sample = Some(
                        required(&mut values, "sample")?
                            .into_string()
                            .map_err(|_| "sample must be Unicode")?
                            .parse::<u8>()?,
                    )
                }
                Some("--benchmark") => {
                    mode = match required(&mut values, "benchmark mode")?.to_str() {
                        Some("cold") => Some(BenchmarkMode::Cold),
                        Some("hot") => Some(BenchmarkMode::Hot),
                        _ => return Err("benchmark mode must be cold or hot".into()),
                    }
                }
                _ => return Err("unknown or non-Unicode argument".into()),
            }
        }
        let book_root = book_root.ok_or("missing --book-root")?;
        let entry = entry.ok_or("missing --entry")?;
        let benchmark = match (run_id, process_sample, mode) {
            (None, None, None) => None,
            (Some(run_id), Some(process_sample), Some(mode))
                if safe_identifier(&run_id) && (1..=10).contains(&process_sample) =>
            {
                Some(Benchmark {
                    run_id,
                    process_sample,
                    mode,
                })
            }
            _ => return Err("benchmark arguments must be complete and valid".into()),
        };
        if benchmark.is_some() && !verify_sample {
            return Err("benchmarks require --verify-sample".into());
        }
        Ok(Self {
            book_root,
            entry,
            verify_sample,
            benchmark,
        })
    }
}

pub(super) fn reader_url(arguments: &Arguments, probe: Option<&TcpListener>) -> String {
    let mut query = vec![format!("entry={}", percent_encode(&arguments.entry))];
    if arguments.verify_sample {
        query.push("verify=1".into());
        let port = probe
            .expect("verification probe")
            .local_addr()
            .expect("probe address")
            .port();
        query.push(format!(
            "probe={}",
            percent_encode(&format!("http://127.0.0.1:{port}/blocked.png"))
        ));
    }
    if let Some(benchmark) = &arguments.benchmark {
        query.push(format!("benchmark={}", benchmark.mode.as_str()));
    }
    format!("atha://localhost/atha-reader.html?{}", query.join("&"))
}

pub(super) fn initial_window_size(
    screen: LogicalSize<f64>,
    monitor_scale_factor: f64,
) -> LogicalSize<f64> {
    let scale_factor = if monitor_scale_factor.is_finite() && monitor_scale_factor > 0.0 {
        monitor_scale_factor
    } else {
        1.0
    };
    let max_width = (screen.width * MAX_SCREEN_FRACTION - WINDOW_FRAME_ALLOWANCE_LOGICAL).max(1.0);
    let max_height =
        (screen.height * MAX_SCREEN_FRACTION - WINDOW_FRAME_ALLOWANCE_LOGICAL).max(1.0);
    LogicalSize::new(
        (PAGE_DEVICE_WIDTH / scale_factor + WINDOW_PADDING_LOGICAL).min(max_width),
        (PAGE_DEVICE_HEIGHT / scale_factor + WINDOW_PADDING_LOGICAL).min(max_height),
    )
}

fn required(
    values: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<OsString, Box<dyn Error>> {
    values
        .next()
        .ok_or_else(|| format!("missing {name} value").into())
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_window_reserves_system_frame_within_screen_bounds() {
        let size = initial_window_size(LogicalSize::new(1920.0, 1080.0), 2.0);

        assert_eq!(size.width, 680.0);
        assert_eq!(size.height, 816.0);
    }
}
