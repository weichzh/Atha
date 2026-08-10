use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{fb2::import_fb2, library::LocalLibrary};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

#[test]
fn fb2_can_be_staged_before_its_first_open() {
    let root = TestRoot::new();
    let source = root.0.join("staged.fb2");
    fs::write(&source, sample_fb2()).expect("write FB2");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let staged = library
        .stage_with_title_hint(&source, None)
        .expect("stage FB2");
    assert!(!staged.prepared);
    assert!(
        library
            .open_book(&staged.id)
            .expect("prepare FB2")
            .book
            .prepared
    );
}

#[test]
fn imports_fb2_metadata_sections_links_and_cover() {
    let root = TestRoot::new();
    let source = root.0.join("reader-gate.fb2");
    fs::write(&source, sample_fb2()).expect("write FB2");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import FB2");
    assert_eq!(imported.title, "Atha FB2 Gate");
    assert_eq!(imported.authors, ["Ada Lin"]);
    assert!(imported.has_cover);

    let opened = library.open_book(&imported.id).expect("open FB2");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse manifest");
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        manifest["toc"],
        serde_json::json!([
            {"label": "第一章", "href": ".atha-fb2/section-0002.xhtml#chapter-1"},
            {"label": "第二章", "href": ".atha-fb2/section-0003.xhtml#chapter-2"},
            {"label": "注释", "href": ".atha-fb2/section-0004.xhtml#note-1"}
        ])
    );
    assert_eq!(
        manifest["resources"],
        serde_json::json!([".atha-fb2/images/image-0001.png"])
    );

    let first_chapter = opened
        .root
        .read("/.atha-fb2/section-0002.xhtml")
        .expect("read first chapter");
    assert_eq!(
        first_chapter.content_type,
        "application/xhtml+xml; charset=utf-8"
    );
    let first_chapter = String::from_utf8(first_chapter.bytes).expect("UTF-8 XHTML");
    assert!(first_chapter.contains("<strong class=\"strong\">重点</strong>"));
    assert!(first_chapter.contains("href=\"section-0004.xhtml#note-1\""));
    assert!(first_chapter.contains("src=\"images/image-0001.png\""));
    assert!(!first_chapter.contains("https://"));

    let cover = library.cover(&imported.id).expect("read FB2 cover");
    assert_eq!(cover.content_type, "image/png");
    assert_eq!(cover.bytes, PNG_1X1);

    let moved = root.0.join("moved.fb2");
    fs::copy(&source, &moved).expect("copy FB2");
    assert_eq!(library.import(&moved).expect("reuse FB2").id, imported.id);
}

#[test]
fn fbz_uses_the_same_content_identity_as_its_fb2() {
    let root = TestRoot::new();
    let fb2 = root.0.join("same.fb2");
    let fbz = root.0.join("same.fbz");
    let xml = sample_fb2();
    fs::write(&fb2, &xml).expect("write FB2");
    write_fbz(&fbz, &[("book.fb2", &xml)]);

    let direct = import_fb2(&fb2, root.0.join("cache")).expect("import FB2");
    let zipped = import_fb2(&fbz, root.0.join("cache")).expect("import FBZ");
    assert_eq!(zipped.content_version, direct.content_version);
    assert_eq!(zipped.root, direct.root);
}

#[test]
fn rejects_active_external_ambiguous_and_broken_inputs() {
    let root = TestRoot::new();
    for (name, source, code) in [
        (
            "doctype.fb2",
            sample_fb2().replace(
                "<FictionBook",
                "<!DOCTYPE FictionBook SYSTEM \"https://example.invalid/book.dtd\"><FictionBook",
            ),
            "invalid-fb2-xml",
        ),
        (
            "processing-instruction.fb2",
            sample_fb2().replace("<description>", "<?unsafe value?><description>"),
            "invalid-fb2-xml",
        ),
        (
            "unknown-root.fb2",
            sample_fb2()
                .replacen("<FictionBook", "<Book", 1)
                .replace("</FictionBook>", "</Book>"),
            "unsupported-fb2",
        ),
        (
            "script.fb2",
            sample_fb2().replace(
                "<p>第二节正文</p>",
                "<p>第二节正文<script>bad()</script></p>",
            ),
            "unsupported-fb2",
        ),
        (
            "external.fb2",
            sample_fb2().replace("l:href=\"#note-1\"", "l:href=\"https://example.invalid\""),
            "invalid-fb2-reference",
        ),
        (
            "missing-image.fb2",
            sample_fb2().replace("#cover", "#missing"),
            "invalid-fb2-reference",
        ),
        (
            "bad-image.fb2",
            sample_fb2().replace(&BASE64.encode(PNG_1X1), "bm90LWEtcG5n"),
            "invalid-fb2-image",
        ),
        (
            "unsupported-image.fb2",
            sample_fb2().replace("image/png", "image/gif"),
            "invalid-fb2-image",
        ),
        (
            "unknown-unreferenced-binary.fb2",
            sample_fb2().replace(
                "</FictionBook>",
                "<binary id=\"unused\" content-type=\"application/javascript\">YWxlcnQoMSk=</binary></FictionBook>",
            ),
            "invalid-fb2-image",
        ),
        (
            "empty.fb2",
            "<?xml version=\"1.0\"?><FictionBook><description/><body/></FictionBook>".to_owned(),
            "unsupported-fb2",
        ),
    ] {
        let path = root.0.join(name);
        fs::write(&path, source).expect("write rejection case");
        let cache = root.0.join(format!("cache-{name}"));
        let error = import_fb2(&path, &cache).expect_err("reject unsafe FB2");
        assert_eq!(error.code(), code, "{name}");
        assert_eq!(
            fs::read_dir(cache)
                .expect("read cleaned rejection cache")
                .count(),
            0,
            "{name}"
        );
    }

    let ambiguous = root.0.join("ambiguous.fbz");
    let xml = sample_fb2();
    write_fbz(&ambiguous, &[("one.fb2", &xml), ("two.fb2", &xml)]);
    assert_eq!(
        import_fb2(&ambiguous, root.0.join("ambiguous-cache"))
            .expect_err("reject ambiguous FBZ")
            .code(),
        "unsupported-fb2"
    );

    let nested = root.0.join("nested.fbz");
    write_fbz(&nested, &[("books/book.fb2", &xml)]);
    assert_eq!(
        import_fb2(&nested, root.0.join("nested-cache"))
            .expect_err("reject nested FBZ")
            .code(),
        "unsupported-fb2"
    );
}

#[test]
fn supports_declared_windows_1251_input() {
    let root = TestRoot::new();
    let source = root.0.join("cp1251.fb2");
    let xml = r##"<?xml version="1.0" encoding="windows-1251"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><book-title>Книга</book-title></title-info></description><body><section id="one"><title><p>Глава</p></title><p>Текст</p></section></body></FictionBook>"##;
    let (encoded, _, had_errors) = encoding_rs::WINDOWS_1251.encode(xml);
    assert!(!had_errors);
    fs::write(&source, encoded).expect("write Windows-1251 FB2");

    let imported = import_fb2(&source, root.0.join("cache")).expect("import Windows-1251 FB2");
    assert_eq!(imported.title.as_deref(), Some("Книга"));
}

#[test]
fn supports_xml_predefined_and_numeric_references() {
    let root = TestRoot::new();
    let source = root.0.join("references.fb2");
    let xml = sample_fb2()
        .replace("Atha FB2 Gate", "Atha &amp; FB2 &#71;ate")
        .replace("第二节正文", "A &lt; B &#160; C");
    fs::write(&source, xml).expect("write entity FB2");

    let imported = import_fb2(&source, root.0.join("cache")).expect("import entity FB2");
    assert_eq!(imported.title.as_deref(), Some("Atha & FB2 Gate"));
    let section = fs::read_to_string(imported.root.join(".atha-fb2/section-0003.xhtml"))
        .expect("read entity XHTML");
    assert!(section.contains("A &lt; B \u{a0} C"));
}

#[test]
fn nested_sections_without_ids_keep_unique_toc_targets() {
    let root = TestRoot::new();
    let source = root.0.join("nested.fb2");
    let xml = r#"<?xml version="1.0"?><FictionBook><body>
<section><title><p>One</p></title><section><title><p>Nested</p></title><p>Text</p></section></section>
<section><title><p>Two</p></title><p>Text</p></section>
</body></FictionBook>"#;
    fs::write(&source, xml).expect("write nested FB2");

    let imported = import_fb2(&source, root.0.join("cache")).expect("import nested FB2");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(".atha-reader.json")).expect("read nested manifest"),
    )
    .expect("parse nested manifest");
    let hrefs = manifest["toc"]
        .as_array()
        .expect("nested TOC")
        .iter()
        .map(|item| item["href"].as_str().expect("TOC href"))
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(manifest["toc"].as_array().map(Vec::len), Some(2));
    assert_eq!(hrefs.len(), 2);
}

#[test]
#[ignore = "writes the deterministic fixture consumed by the formal platform gates"]
fn writes_fb2_gate_fixture() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = repository.join(".tmp/fb2-gate.fb2");
    let imports = repository.join(".tmp/fb2-gate-imports");
    if source.exists() {
        fs::remove_file(&source).expect("remove old FB2 fixture");
    }
    if imports.exists() {
        fs::remove_dir_all(&imports).expect("remove old FB2 imports");
    }
    fs::write(&source, sample_fb2()).expect("write deterministic FB2 fixture");
    import_fb2(&source, &imports).expect("prepare deterministic FB2 fixture");
    if let Some(root) = std::env::var_os("ATHA_FB2_GATE_LIBRARY_ROOT") {
        let root = PathBuf::from(root);
        let temporary = fs::canonicalize(repository.join(".tmp")).expect("resolve .tmp root");
        let resolved = fs::canonicalize(&root).unwrap_or_else(|_| {
            fs::canonicalize(root.parent().expect("FB2 gate library parent"))
                .expect("resolve FB2 gate library parent")
                .join(root.file_name().expect("FB2 gate library name"))
        });
        assert!(
            root.is_absolute() && resolved.starts_with(temporary),
            "FB2 gate library must stay inside the repository .tmp directory"
        );
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove old FB2 gate library");
        }
        let library = LocalLibrary::open(root).expect("open FB2 gate library");
        let book = library.import(&source).expect("seed FB2 gate library");
        assert_eq!(book.title, "Atha FB2 Gate");
    }
}

fn sample_fb2() -> String {
    format!(
        r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
  <description><title-info><genre>prose</genre><author><first-name>Ada</first-name><last-name>Lin</last-name></author><book-title>Atha FB2 Gate</book-title><coverpage><image l:href="#cover"/></coverpage><lang>zh</lang></title-info></description>
  <body><title><p>卷首</p></title>
    <section id="chapter-1"><title><p>第一章</p></title><p>正文<strong>重点</strong><a l:href="#note-1" type="note">1</a></p><image l:href="#cover"/></section>
    <section id="chapter-2"><title><p>第二章</p></title><p>第二节正文</p></section>
  </body>
  <body name="notes"><section id="note-1"><title><p>注释</p></title><p>注释正文</p></section></body>
  <binary id="cover" content-type="image/png">{}</binary>
</FictionBook>"##,
        BASE64.encode(PNG_1X1)
    )
}

fn write_fbz(path: &Path, members: &[(&str, &str)]) {
    let file = File::create(path).expect("create FBZ");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in members {
        writer.start_file(*name, options).expect("start FBZ member");
        writer
            .write_all(content.as_bytes())
            .expect("write FBZ member");
    }
    writer.finish().expect("finish FBZ");
}

struct TestRoot(PathBuf);
static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!(
                "atha-fb2-import-{}-{nonce}-{}",
                std::process::id(),
                TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).expect("create test root");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test root");
    }
}
