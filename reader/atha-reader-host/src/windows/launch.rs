use std::{env, error::Error, ffi::OsString, net::TcpListener, path::PathBuf};

use atha_backend::reader::epub::{READER_MANIFEST, import_epub};

use tao::dpi::LogicalSize;

pub(super) const APP_PAGE: &str = "https://atha.localhost/atha-reader.html";
const PAGE_DEVICE_WIDTH: f64 = 780.0;
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
    input: BookInput,
    pub(super) verify_sample: bool,
    import_probe: bool,
    pub(super) hold_after_verify: bool,
    pub(super) state_probe: Option<StateProbe>,
    pub(super) benchmark: Option<Benchmark>,
}

enum BookInput {
    Prepared {
        book_root: PathBuf,
        source: BookSource,
    },
    Epub(PathBuf),
}

pub(super) struct ResolvedBook {
    pub(super) book_root: PathBuf,
    pub(super) source: BookSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StateProbe {
    Write,
    Read,
}

impl StateProbe {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Read => "read",
        }
    }
}

#[derive(Clone)]
pub(super) enum BookSource {
    Entry(String),
    Manifest(String),
}

impl BookSource {
    pub(super) fn path(&self) -> &str {
        match self {
            Self::Entry(path) | Self::Manifest(path) => path,
        }
    }
}

impl Arguments {
    pub(super) fn parse() -> Result<Self, Box<dyn Error>> {
        Self::parse_values(env::args_os().skip(1))
    }

    fn parse_values(mut values: impl Iterator<Item = OsString>) -> Result<Self, Box<dyn Error>> {
        let mut book_root = None;
        let mut epub = None;
        let mut entry = None;
        let mut manifest = None;
        let mut verify_sample = false;
        let mut import_probe = false;
        let mut hold_after_verify = false;
        let mut state_probe = None;
        let mut run_id = None;
        let mut process_sample = None;
        let mut mode = None;
        while let Some(flag) = values.next() {
            match flag.to_str() {
                Some("--book-root") => book_root = Some(required(&mut values, "book root")?.into()),
                Some("--epub") => epub = Some(required(&mut values, "EPUB path")?.into()),
                Some("--entry") => {
                    entry = Some(
                        required(&mut values, "entry")?
                            .into_string()
                            .map_err(|_| "entry must be Unicode")?,
                    )
                }
                Some("--manifest") => {
                    manifest = Some(
                        required(&mut values, "manifest")?
                            .into_string()
                            .map_err(|_| "manifest must be Unicode")?,
                    )
                }
                Some("--verify-sample") => verify_sample = true,
                Some("--verify-import") => import_probe = true,
                Some("--hold-after-verify") => hold_after_verify = true,
                Some("--state-probe") => {
                    state_probe = match required(&mut values, "state probe")?.to_str() {
                        Some("write") => Some(StateProbe::Write),
                        Some("read") => Some(StateProbe::Read),
                        _ => return Err("state probe must be write or read".into()),
                    }
                }
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
        let input = match (epub, book_root, entry, manifest) {
            (Some(path), None, None, None) => BookInput::Epub(path),
            (None, Some(book_root), Some(path), None) => BookInput::Prepared {
                book_root,
                source: BookSource::Entry(path),
            },
            (None, Some(book_root), None, Some(path)) => BookInput::Prepared {
                book_root,
                source: BookSource::Manifest(path),
            },
            _ => {
                return Err(
                    "use either --epub or --book-root with exactly one of --entry/--manifest"
                        .into(),
                );
            }
        };
        if import_probe && !matches!(&input, BookInput::Epub(_)) {
            return Err("import verification requires --epub".into());
        }
        if verify_sample && import_probe {
            return Err("verification modes are mutually exclusive".into());
        }
        verify_sample |= import_probe;
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
        if import_probe && (benchmark.is_some() || state_probe.is_some() || hold_after_verify) {
            return Err("import verification cannot benchmark, persist, or hold".into());
        }
        if state_probe.is_some() && (!verify_sample || benchmark.is_some()) {
            return Err("state probe requires non-benchmark verification".into());
        }
        if hold_after_verify
            && (!verify_sample || benchmark.is_some() || state_probe == Some(StateProbe::Read))
        {
            return Err("hold-after-verify requires plain or write verification".into());
        }
        Ok(Self {
            input,
            verify_sample,
            import_probe,
            hold_after_verify,
            state_probe,
            benchmark,
        })
    }

    pub(super) fn resolve_book(&self) -> Result<ResolvedBook, Box<dyn Error>> {
        match &self.input {
            BookInput::Prepared { book_root, source } => Ok(ResolvedBook {
                book_root: book_root.clone(),
                source: source.clone(),
            }),
            BookInput::Epub(path) => {
                let local_app_data = env::var_os("LOCALAPPDATA").ok_or("missing LOCALAPPDATA")?;
                let imported = import_epub(
                    path,
                    PathBuf::from(local_app_data)
                        .join("Atha")
                        .join("ImportedBooks"),
                )?;
                Ok(ResolvedBook {
                    book_root: imported.root,
                    source: BookSource::Manifest(READER_MANIFEST.into()),
                })
            }
        }
    }
}

pub(super) fn reader_url(
    arguments: &Arguments,
    source: &BookSource,
    probe: Option<&TcpListener>,
    state_key: &str,
    content_version: Option<&str>,
) -> String {
    let mut query = vec![match source {
        BookSource::Entry(path) => format!("entry={}", percent_encode(path)),
        BookSource::Manifest(path) => format!("manifest={}", percent_encode(path)),
    }];
    if arguments.verify_sample {
        query.push("verify=1".into());
        if arguments.import_probe {
            query.push("verify-import=1".into());
        }
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
    query.push(format!("state={state_key}"));
    if let Some(version) = content_version {
        query.push(format!("version={version}"));
    }
    if let Some(state_probe) = arguments.state_probe {
        query.push("persist=1".into());
        query.push(format!("state-probe={}", state_probe.as_str()));
    }
    if let Some(benchmark) = &arguments.benchmark {
        query.push(format!("benchmark={}", benchmark.mode.as_str()));
    }
    format!("atha://localhost/atha-reader.html?{}", query.join("&"))
}

pub(super) fn state_key(source: &std::path::Path) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in source
        .to_string_lossy()
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(super) fn content_fingerprint(bytes: &[u8]) -> String {
    [
        0xcbf29ce484222325,
        0x84222325cbf29ce4,
        0x9e3779b185ebca87,
        0xd6e8feb86659fd93,
    ]
    .map(|mut hash| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    })
    .join("")
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

        assert_eq!(size.width, 438.0);
        assert_eq!(size.height, 816.0);
    }

    #[test]
    fn arguments_require_exactly_one_book_source() {
        let values = |items: &[&str]| {
            items
                .iter()
                .map(|value| OsString::from(*value))
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert!(
            Arguments::parse_values(values(&["--book-root", "book", "--entry", "a.xhtml"])).is_ok()
        );
        assert!(
            Arguments::parse_values(values(&[
                "--book-root",
                "book",
                "--manifest",
                ".atha-reader.json"
            ]))
            .is_ok()
        );
        assert!(Arguments::parse_values(values(&["--epub", "book.epub"])).is_ok());
        assert!(
            Arguments::parse_values(values(&["--epub", "book.epub", "--verify-import"])).is_ok()
        );
        assert!(
            Arguments::parse_values(values(&[
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
                "--verify-sample",
                "--state-probe",
                "write",
                "--hold-after-verify",
            ]))
            .is_ok()
        );
        assert!(
            Arguments::parse_values(values(&[
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
                "--verify-sample",
                "--hold-after-verify",
            ]))
            .is_ok()
        );
        for invalid in [
            vec!["--book-root", "book"],
            vec![
                "--epub",
                "book.epub",
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
            ],
            vec!["--epub", "book.epub", "--manifest", ".atha-reader.json"],
            vec!["--epub", "book.epub", "--verify-sample", "--verify-import"],
            vec![
                "--book-root",
                "book",
                "--manifest",
                ".atha-reader.json",
                "--verify-import",
            ],
            vec![
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
                "--manifest",
                ".atha-reader.json",
            ],
            vec![
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
                "--hold-after-verify",
            ],
            vec![
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
                "--verify-sample",
                "--hold-after-verify",
                "--state-probe",
                "read",
            ],
            vec![
                "--book-root",
                "book",
                "--entry",
                "a.xhtml",
                "--verify-sample",
                "--hold-after-verify",
                "--benchmark-run",
                "test",
                "--sample",
                "1",
                "--benchmark",
                "cold",
            ],
        ] {
            assert!(Arguments::parse_values(values(&invalid)).is_err());
        }
    }

    #[test]
    fn state_key_is_stable_without_exposing_the_path() {
        let first = state_key(std::path::Path::new(r"C:\Books\one\book.json"));
        let repeated = state_key(std::path::Path::new(r"C:\Books\one\book.json"));
        let other = state_key(std::path::Path::new(r"C:\Books\two\book.json"));

        assert_eq!(first, repeated);
        assert_ne!(first, other);
        assert_eq!(first.len(), 16);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn content_fingerprint_tracks_legacy_entry_bytes() {
        let first = content_fingerprint(b"first");
        assert_eq!(first, content_fingerprint(b"first"));
        assert_ne!(first, content_fingerprint(b"second"));
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}
