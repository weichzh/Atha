use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{
    READER_PAGE,
    resources::{BookRoot, ResourceError},
    telemetry::{
        FailureStage, MetricStage, ReaderEvent, ReaderFailure, TelemetryError, parse_reader_event,
        safe_event,
    },
};

struct TestTree(PathBuf);

impl TestTree {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("reader-test-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("create test tree");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test tree");
    }
}

#[test]
fn book_root_rejects_path_and_media_escapes() {
    let tree = TestTree::new();
    let root = tree.path().join("book");
    fs::create_dir(&root).expect("create book root");
    fs::write(root.join("chapter.xhtml"), b"<html/>").expect("write chapter");
    fs::write(root.join(".atha-reader.json"), b"{\"schema\":1}").expect("write manifest");
    fs::write(root.join("secret.exe"), b"secret").expect("write unknown media");
    fs::write(tree.path().join("outside.svg"), b"<svg/>").expect("write outside file");
    let book = BookRoot::new(&root).expect("open book root");

    let resource = book.read("/chapter.xhtml").expect("read chapter");
    assert_eq!(
        resource.content_type,
        "application/xhtml+xml; charset=utf-8"
    );
    assert_eq!(resource.bytes, b"<html/>");

    let manifest = book.read("/.atha-reader.json").expect("read manifest");
    assert_eq!(manifest.content_type, "application/json; charset=utf-8");

    for path in [
        "chapter.xhtml",
        "/",
        "//server/share.svg",
        "/C:/secret.svg",
        "/a\\b.svg",
        "/../outside.svg",
        "/%2e%2e/outside.svg",
        "/%5c%5cserver%5cshare.svg",
        "/bad%00.svg",
        "/a//b.svg",
    ] {
        assert!(book.read(path).is_err(), "accepted unsafe path: {path}");
    }
    assert_eq!(
        book.read("/bad%GG.svg"),
        Err(ResourceError::InvalidEncoding)
    );
    assert_eq!(
        book.read("/secret.exe"),
        Err(ResourceError::UnsupportedMediaType)
    );
}

#[cfg(windows)]
#[test]
fn book_root_rejects_symlinks_outside_the_root() {
    use std::os::windows::fs::symlink_file;

    let tree = TestTree::new();
    let root = tree.path().join("book");
    fs::create_dir(&root).expect("create book root");
    let outside = tree.path().join("outside.svg");
    fs::write(&outside, b"<svg/>").expect("write outside file");
    symlink_file(&outside, root.join("linked.svg")).expect("create test symlink");
    let outside_manifest = tree.path().join("outside.json");
    fs::write(
        &outside_manifest,
        br#"{"schema":1,"sections":[{"href":"undeclared"}]}"#,
    )
    .expect("write outside manifest");
    symlink_file(&outside_manifest, root.join(".atha-reader.json"))
        .expect("create manifest symlink");
    fs::write(root.join("undeclared"), b"not xhtml").expect("write undeclared file");

    let book = BookRoot::new(root).expect("open book root");
    assert_eq!(book.read("/linked.svg"), Err(ResourceError::OutsideRoot));
    assert_eq!(
        book.read("/undeclared"),
        Err(ResourceError::UnsupportedMediaType)
    );
}

#[test]
fn telemetry_accepts_only_fixed_non_content_fields_from_the_reader() {
    let origin = format!("{READER_PAGE}?entry=EPUB%2Ftext%2Fch012.xhtml");
    let event = parse_reader_event(&origin, "metric|page_turn|4|1.25|32|4|860|1640")
        .expect("parse valid metric");
    let ReaderEvent::Metric(metric) = event else {
        panic!("expected metric");
    };
    assert_eq!(metric.stage, MetricStage::PageTurn);
    assert_eq!(metric.sample, 4);
    assert_eq!(metric.duration_ms, 1.25);
    assert_eq!((metric.page_width, metric.page_height), (860, 1640));
    assert_eq!(
        parse_reader_event(READER_PAGE, "error|state-persistence|layout-stable"),
        Ok(ReaderEvent::Error(ReaderFailure {
            code: "state-persistence",
            stage: FailureStage::LayoutStable,
        }))
    );
    assert_eq!(safe_event("state-persistence"), "state-persistence");
    assert_eq!(safe_event("E:/private/book.xhtml"), "invalid-event");

    for (origin, message, expected) in [
        (
            "https://evil.example/atha-reader.html",
            "ready|4|148|6|0",
            TelemetryError::InvalidOrigin,
        ),
        (
            READER_PAGE,
            "metric|unknown|1|1|32|4|860|1640",
            TelemetryError::InvalidMessage,
        ),
        (
            READER_PAGE,
            "metric|page_turn|11|1|32|4|860|1640",
            TelemetryError::OutOfRange,
        ),
        (
            READER_PAGE,
            "metric|page_turn|1|1|32|4|0|1640",
            TelemetryError::OutOfRange,
        ),
        (
            READER_PAGE,
            "error|E:/private/book.xhtml",
            TelemetryError::InvalidMessage,
        ),
        (
            READER_PAGE,
            "error|state-persistence",
            TelemetryError::InvalidMessage,
        ),
        (
            READER_PAGE,
            "error|book-load|E:/private/book.xhtml",
            TelemetryError::InvalidMessage,
        ),
    ] {
        assert_eq!(parse_reader_event(origin, message), Err(expected));
    }
}
