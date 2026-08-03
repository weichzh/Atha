use std::{
    collections::HashSet,
    env,
    error::Error,
    fs::{self, File, OpenOptions},
    io::{BufWriter, ErrorKind, Write},
    net::TcpListener,
    path::PathBuf,
    process,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::telemetry::{Metric, MetricStage, Ready};

use super::launch::{Benchmark, BenchmarkMode};

const SAMPLE_ID: &str = "logic-1-2-v1";

pub enum DiagnosticError {
    Reader(&'static str),
    Recorder(&'static str, std::io::Error),
}

pub enum ReadyDisposition {
    Interactive,
    VerificationComplete,
}

pub struct Diagnostics {
    recorder: Recorder,
    progress: BenchmarkProgress,
    network_probe: Option<TcpListener>,
    verify_sample: bool,
    benchmark_mode: Option<BenchmarkMode>,
    startup: Instant,
}

impl Diagnostics {
    pub fn new(
        startup: Instant,
        verify_sample: bool,
        benchmark: Option<&Benchmark>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut recorder = Recorder::new(benchmark)?;
        recorder.log("info", "start")?;
        let network_probe = if verify_sample {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            listener.set_nonblocking(true)?;
            Some(listener)
        } else {
            None
        };
        Ok(Self {
            recorder,
            progress: BenchmarkProgress::default(),
            network_probe,
            verify_sample,
            benchmark_mode: benchmark.map(|value| value.mode),
            startup,
        })
    }

    pub fn network_probe(&self) -> Option<&TcpListener> {
        self.network_probe.as_ref()
    }

    pub fn record_metric(&mut self, metric: Metric) -> Result<(), DiagnosticError> {
        if !self.progress.insert(metric) {
            return Err(DiagnosticError::Reader("duplicate-metric"));
        }
        if metric.stage == MetricStage::FirstStable
            && self.benchmark_mode == Some(BenchmarkMode::Cold)
        {
            self.recorder
                .cold_start(self.startup.elapsed().as_secs_f64() * 1000.0, metric.pages)
                .map_err(|error| {
                    DiagnosticError::Recorder("reader cold-start write failed", error)
                })?;
        }
        self.recorder
            .metric(metric)
            .map_err(|error| DiagnosticError::Recorder("reader telemetry write failed", error))
    }

    pub fn record_ready(&mut self, ready: Ready) -> Result<ReadyDisposition, DiagnosticError> {
        if (self.verify_sample && (ready.cuts != 0 || ready.pages == 0))
            || self.network_connected()
            || self
                .benchmark_mode
                .is_some_and(|mode| !self.progress.complete(mode))
        {
            return Err(DiagnosticError::Reader("self-check"));
        }
        self.recorder
            .log("info", "ready")
            .and_then(|_| self.recorder.flush())
            .map_err(|error| DiagnosticError::Recorder("reader recorder flush failed", error))?;
        Ok(if self.verify_sample {
            ReadyDisposition::VerificationComplete
        } else {
            ReadyDisposition::Interactive
        })
    }

    pub fn record_failure(&mut self, code: &str) {
        let _ = self.recorder.log("error", safe_event(code));
        let _ = self.recorder.flush();
    }

    pub fn flush(&mut self) {
        let _ = self.recorder.flush();
    }

    fn network_connected(&self) -> bool {
        let Some(listener) = &self.network_probe else {
            return false;
        };
        match listener.accept() {
            Ok(_) => true,
            Err(error) if error.kind() == ErrorKind::WouldBlock => false,
            Err(_) => true,
        }
    }
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
                780,
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

fn complete_samples(values: &HashSet<u8>) -> bool {
    values.len() == 10 && (1..=10).all(|sample| values.contains(&sample))
}

fn create_new(path: PathBuf) -> std::io::Result<BufWriter<File>> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(BufWriter::new)
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
