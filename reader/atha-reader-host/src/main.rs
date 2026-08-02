#[cfg(windows)]
mod windows {
    use std::{
        borrow::Cow,
        collections::HashSet,
        env,
        error::Error,
        ffi::OsString,
        fs::{self, File, OpenOptions},
        io::{BufWriter, ErrorKind, Write},
        net::TcpListener,
        path::PathBuf,
        process,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    use atha_backend::reader::{
        READER_ORIGIN,
        resources::{BookRoot, ResourceError},
        telemetry::{Metric, MetricStage, ReaderEvent, Ready, TelemetryError, parse_reader_event},
    };
    use tao::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
        window::{Window, WindowBuilder},
    };
    use wry::{
        NewWindowResponse, PermissionResponse, WebViewBuilder, WebViewBuilderExtWindows,
        http::{Request, Response, StatusCode, header},
    };

    const APP_PAGE: &str = "https://atha.localhost/atha-reader.html";
    const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src https://atha-book.localhost; connect-src 'self' https://atha-book.localhost; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";
    const SAMPLE_ID: &str = "logic-1-2-v1";
    const PAGE_DEVICE_WIDTH: f64 = 1264.0;
    const PAGE_DEVICE_HEIGHT: f64 = 1680.0;
    const WINDOW_PADDING_LOGICAL: f64 = 48.0;
    const WINDOW_FRAME_ALLOWANCE_LOGICAL: f64 = 48.0;
    const MAX_SCREEN_FRACTION: f64 = 0.8;

    enum UserEvent {
        Reader(Result<ReaderEvent, TelemetryError>),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum BenchmarkMode {
        Cold,
        Hot,
    }

    impl BenchmarkMode {
        const fn as_str(self) -> &'static str {
            match self {
                Self::Cold => "cold",
                Self::Hot => "hot",
            }
        }
    }

    struct Benchmark {
        run_id: String,
        process_sample: u8,
        mode: BenchmarkMode,
    }

    struct Arguments {
        book_root: PathBuf,
        entry: String,
        verify_sample: bool,
        benchmark: Option<Benchmark>,
    }

    #[derive(Default)]
    struct BenchmarkProgress {
        first_stable: HashSet<u8>,
        hot_open: HashSet<u8>,
        page_turn: HashSet<u8>,
        font_reflow: HashSet<u8>,
    }

    impl BenchmarkProgress {
        fn insert(&mut self, metric: Metric) -> bool {
            match metric.stage {
                MetricStage::FirstStable => self.first_stable.insert(metric.sample),
                MetricStage::HotOpen => self.hot_open.insert(metric.sample),
                MetricStage::PageTurn => self.page_turn.insert(metric.sample),
                MetricStage::FontReflow => self.font_reflow.insert(metric.sample),
            }
        }

        fn complete(&self, mode: BenchmarkMode) -> bool {
            match mode {
                BenchmarkMode::Cold => {
                    self.first_stable == HashSet::from([1])
                        && self.hot_open.is_empty()
                        && self.page_turn.is_empty()
                        && self.font_reflow.is_empty()
                }
                BenchmarkMode::Hot => {
                    self.first_stable.is_empty()
                        && complete_samples(&self.hot_open)
                        && complete_samples(&self.page_turn)
                        && complete_samples(&self.font_reflow)
                }
            }
        }
    }

    struct Recorder {
        log: BufWriter<File>,
        benchmark: Option<BufWriter<File>>,
        renderer: String,
        run_id: String,
        process_sample: u8,
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let startup = Instant::now();
        let arguments = Arguments::parse()?;
        let book_root = BookRoot::new(&arguments.book_root)?;
        book_root.read(&format!("/{}", arguments.entry))?;
        let mut recorder = Recorder::new(arguments.benchmark.as_ref())?;
        recorder.log("info", "start")?;
        let network_probe = if arguments.verify_sample {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            listener.set_nonblocking(true)?;
            Some(listener)
        } else {
            None
        };

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let window_size = event_loop
            .primary_monitor()
            .map(|monitor| {
                let scale_factor = monitor.scale_factor();
                let screen = monitor.size().to_logical::<f64>(scale_factor);
                initial_window_size(screen, scale_factor)
            })
            .unwrap_or_else(|| LogicalSize::new(900.0, 900.0));
        let window = WindowBuilder::new()
            .with_title("Atha Reader")
            .with_inner_size(window_size)
            .build(&event_loop)?;
        let proxy = event_loop.create_proxy();
        let url = reader_url(&arguments, network_probe.as_ref());
        let book_resources = book_root.clone();

        let builder = WebViewBuilder::new()
            .with_custom_protocol("atha".into(), move |_id, request| app_response(request))
            .with_custom_protocol("atha-book".into(), move |_id, request| {
                book_response(&book_resources, request)
            })
            .with_url(url)
            .with_incognito(true)
            .with_devtools(false)
            .with_clipboard(false)
            .with_general_autofill_enabled(false)
            .with_hotkeys_zoom(false)
            .with_drag_drop_handler(|_| true)
            .with_navigation_handler(is_reader_url)
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
        let mut progress = BenchmarkProgress::default();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::UserEvent(UserEvent::Reader(Ok(ReaderEvent::Metric(metric)))) => {
                    if !progress.insert(metric) {
                        fail_run(
                            &mut recorder,
                            &window,
                            "duplicate-metric",
                            arguments.verify_sample,
                        );
                        return;
                    }
                    if metric.stage == MetricStage::FirstStable
                        && arguments
                            .benchmark
                            .as_ref()
                            .is_some_and(|benchmark| benchmark.mode == BenchmarkMode::Cold)
                        && recorder
                            .cold_start(startup.elapsed().as_secs_f64() * 1000.0, metric.pages)
                            .is_err()
                    {
                        eprintln!("reader cold-start write failed");
                        process::exit(1);
                    }
                    if let Err(error) = recorder.metric(metric) {
                        eprintln!("reader telemetry write failed: {error}");
                        process::exit(1);
                    }
                }
                Event::UserEvent(UserEvent::Reader(Ok(ReaderEvent::Ready(ready)))) => {
                    if !ready_is_valid(&arguments, ready)
                        || network_connected(network_probe.as_ref())
                        || arguments
                            .benchmark
                            .as_ref()
                            .is_some_and(|benchmark| !progress.complete(benchmark.mode))
                    {
                        fail_run(
                            &mut recorder,
                            &window,
                            "self-check",
                            arguments.verify_sample,
                        );
                        return;
                    }
                    if recorder.log("info", "ready").is_err() || recorder.flush().is_err() {
                        eprintln!("reader recorder flush failed");
                        process::exit(1);
                    }
                    if arguments.verify_sample {
                        process::exit(0);
                    }
                    window.set_title("Atha Reader");
                }
                Event::UserEvent(UserEvent::Reader(Ok(ReaderEvent::Error(code)))) => {
                    fail_run(&mut recorder, &window, code, arguments.verify_sample);
                }
                Event::UserEvent(UserEvent::Reader(Err(error))) => {
                    fail_run(
                        &mut recorder,
                        &window,
                        error.code(),
                        arguments.verify_sample,
                    );
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    let _ = recorder.flush();
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            }
        });
    }

    impl Arguments {
        fn parse() -> Result<Self, Box<dyn Error>> {
            let mut values = env::args_os().skip(1);
            let mut book_root = None;
            let mut entry = None;
            let mut verify_sample = false;
            let mut run_id = None;
            let mut process_sample = None;
            let mut mode = None;
            while let Some(flag) = values.next() {
                match flag.to_str() {
                    Some("--book-root") => {
                        book_root = Some(required(&mut values, "book root")?.into())
                    }
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

    impl Recorder {
        fn new(benchmark: Option<&Benchmark>) -> Result<Self, Box<dyn Error>> {
            let renderer = safe_token(&wry::webview_version().unwrap_or_else(|_| "unknown".into()));
            let generated_run_id = format!(
                "{}-{}",
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
                process::id()
            );
            let (run_id, process_sample, suffix) = benchmark
                .map(|value| {
                    (
                        value.run_id.clone(),
                        value.process_sample,
                        format!("{}-{}", value.mode.as_str(), value.process_sample),
                    )
                })
                .unwrap_or_else(|| (generated_run_id, 0, "interactive".into()));
            let artifacts = env::current_dir()?.join("artifacts/local");
            let logs = artifacts.join("logs");
            fs::create_dir_all(&logs)?;
            let log = create_new(logs.join(format!("reader-{run_id}-{suffix}.log")))?;
            let benchmark_file = if benchmark.is_some() {
                let benchmarks = artifacts.join("benchmarks");
                fs::create_dir_all(&benchmarks)?;
                let mut file = create_new(benchmarks.join(format!("{run_id}-{suffix}.csv")))?;
                writeln!(
                    file,
                    "run_id,process_sample,stage_sample,renderer,sample_id,page_width,page_height,font_size,pages,mode,stage,duration_ms"
                )?;
                Some(file)
            } else {
                None
            };
            Ok(Self {
                log,
                benchmark: benchmark_file,
                renderer,
                run_id,
                process_sample,
            })
        }

        fn metric(&mut self, metric: Metric) -> std::io::Result<()> {
            self.write_metric(
                metric.sample,
                metric.font_size,
                metric.pages,
                metric.stage.mode(),
                metric.stage.as_str(),
                metric.duration_ms,
            )
        }

        fn cold_start(&mut self, duration_ms: f64, pages: u16) -> std::io::Result<()> {
            self.write_metric(1, 32, pages, "cold", "cold_start", duration_ms)
        }

        fn write_metric(
            &mut self,
            stage_sample: u8,
            font_size: u16,
            pages: u16,
            mode: &str,
            stage: &str,
            duration_ms: f64,
        ) -> std::io::Result<()> {
            if let Some(file) = &mut self.benchmark {
                writeln!(
                    file,
                    "{},{},{},{},{},{},{},{},{},{},{},{:.3}",
                    self.run_id,
                    self.process_sample,
                    stage_sample,
                    self.renderer,
                    SAMPLE_ID,
                    1264,
                    1680,
                    font_size,
                    pages,
                    mode,
                    stage,
                    duration_ms
                )?;
            }
            Ok(())
        }

        fn log(&mut self, level: &str, event: &str) -> std::io::Result<()> {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |value| value.as_millis());
            writeln!(
                self.log,
                "timestamp_ms={timestamp} level={level} event={event}"
            )
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.log.flush()?;
            if let Some(file) = &mut self.benchmark {
                file.flush()?;
            }
            Ok(())
        }
    }

    fn required(
        values: &mut impl Iterator<Item = OsString>,
        name: &str,
    ) -> Result<OsString, Box<dyn Error>> {
        values
            .next()
            .ok_or_else(|| format!("missing {name} value").into())
    }

    fn reader_url(arguments: &Arguments, probe: Option<&TcpListener>) -> String {
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

    fn is_reader_url(url: String) -> bool {
        let Some(suffix) = url.strip_prefix(APP_PAGE) else {
            return false;
        };
        suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
    }

    fn app_response(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
        if request.method() != "GET" {
            return empty_response(StatusCode::METHOD_NOT_ALLOWED);
        }
        let (bytes, content_type): (&'static [u8], &'static str) = match request.uri().path() {
            "/atha-reader.html" => (
                include_bytes!("../../atha-reader.html"),
                "text/html; charset=utf-8",
            ),
            "/atha-reader.css" => (
                include_bytes!("../../atha-reader.css"),
                "text/css; charset=utf-8",
            ),
            "/atha-reader.js" => (
                include_bytes!("../../atha-reader.js"),
                "text/javascript; charset=utf-8",
            ),
            _ => return empty_response(StatusCode::NOT_FOUND),
        };
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::CONTENT_SECURITY_POLICY, CSP)
            .header("x-content-type-options", "nosniff")
            .header("referrer-policy", "no-referrer")
            .body(Cow::Borrowed(bytes))
            .expect("valid app response")
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
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, READER_ORIGIN)
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
            ResourceError::InvalidRoot | ResourceError::ReadFailed => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn ready_is_valid(arguments: &Arguments, ready: Ready) -> bool {
        !arguments.verify_sample || (ready.cuts == 0 && ready.pages > 0)
    }

    fn network_connected(listener: Option<&TcpListener>) -> bool {
        let Some(listener) = listener else {
            return false;
        };
        match listener.accept() {
            Ok(_) => true,
            Err(error) if error.kind() == ErrorKind::WouldBlock => false,
            Err(_) => true,
        }
    }

    fn complete_samples(values: &HashSet<u8>) -> bool {
        values.len() == 10 && (1..=10).all(|sample| values.contains(&sample))
    }

    fn fail_run(recorder: &mut Recorder, window: &Window, code: &str, exit: bool) {
        let _ = recorder.log("error", safe_event(code));
        let _ = recorder.flush();
        window.set_title("Atha Reader - Error");
        if exit {
            eprintln!("reader self-check failed: {}", safe_event(code));
            process::exit(1);
        }
    }

    fn create_new(path: PathBuf) -> std::io::Result<BufWriter<File>> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(BufWriter::new)
    }

    fn safe_identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    }

    fn safe_token(value: &str) -> String {
        value
            .chars()
            .take(64)
            .map(|value| {
                if value.is_ascii_alphanumeric() || matches!(value, '.' | '-' | '_') {
                    value
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn safe_event(value: &str) -> &str {
        if value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        {
            value
        } else {
            "invalid-event"
        }
    }

    fn initial_window_size(
        screen: LogicalSize<f64>,
        monitor_scale_factor: f64,
    ) -> LogicalSize<f64> {
        let scale_factor = if monitor_scale_factor.is_finite() && monitor_scale_factor > 0.0 {
            monitor_scale_factor
        } else {
            1.0
        };
        let max_width =
            (screen.width * MAX_SCREEN_FRACTION - WINDOW_FRAME_ALLOWANCE_LOGICAL).max(1.0);
        let max_height =
            (screen.height * MAX_SCREEN_FRACTION - WINDOW_FRAME_ALLOWANCE_LOGICAL).max(1.0);
        LogicalSize::new(
            (PAGE_DEVICE_WIDTH / scale_factor + WINDOW_PADDING_LOGICAL).min(max_width),
            (PAGE_DEVICE_HEIGHT / scale_factor + WINDOW_PADDING_LOGICAL).min(max_height),
        )
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
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    windows::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("atha-reader-host requires Windows");
    std::process::exit(1);
}
