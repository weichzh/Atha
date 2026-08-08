use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::library::LocalLibrary;
use encoding_rs::GBK;

#[test]
fn imports_repository_markdown_as_a_readable_book() {
    let root = TestRoot::new();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import repository Markdown");
    assert_eq!(imported.title, "Atha");
    assert!(imported.authors.is_empty());
    assert!(!imported.has_cover);

    let opened = library
        .open_book(&imported.id)
        .expect("open imported Markdown");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read reader manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse reader manifest");
    assert_eq!(manifest["schema"], 1);
    assert_eq!(manifest["contentVersion"], imported.id);
    assert_eq!(
        manifest["sections"].as_array().map(Vec::len),
        Some(1),
        "README has one top-level H1"
    );
    assert_eq!(manifest["toc"][0]["label"], "Atha");
    assert!(
        manifest["toc"]
            .as_array()
            .expect("TOC array")
            .iter()
            .any(|item| item["label"] == "工程入口")
    );

    let href = manifest["sections"][0]["href"]
        .as_str()
        .expect("section href");
    let section = opened
        .root
        .read(&format!("/{href}"))
        .expect("read generated section");
    assert_eq!(section.content_type, "application/xhtml+xml; charset=utf-8");
    let section = std::str::from_utf8(&section.bytes).expect("generated XHTML is UTF-8");
    assert!(section.contains("<h1"));
    assert!(section.contains("Atha 是一个本地优先"));
    assert!(section.contains("pre { white-space: pre-wrap; overflow-wrap: break-word; }"));
    assert!(section.contains("table { border-collapse: collapse; }"));
    assert!(section.contains("th, td { border: 1px solid currentColor; padding: 0.2em 0.5em; }"));
    assert!(section.contains("blockquote { margin-inline: 1em; }"));
}

#[test]
fn imports_existing_research_markdown_without_a_generated_book_fixture() {
    let root = TestRoot::new();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/epub2-ncx-library-assessment.md");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import research Markdown");
    assert_eq!(imported.title, "EPUB2 / NCX 最小兼容与成熟库评估");
    let manifest = read_manifest(&library, &imported.id);
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(1));
    assert!(
        manifest["toc"]
            .as_array()
            .expect("research TOC")
            .iter()
            .any(|item| item["label"] == "规范兼容边界")
    );
}

#[test]
fn markdown_projects_active_content_to_inert_readable_text() {
    let root = TestRoot::new();
    let source = root.0.join("unsafe.md");
    fs::write(
        &source,
        concat!(
            "---\nsecret: must-not-render\n---\n",
            "# 安全章节\n",
            "<script>globalThis.pwned = true</script>\n\n",
            "[可见链接](javascript:alert(1))\n\n",
            "![远程图片](https://example.invalid/cover.png)\n\n",
            "![本机图片](file:///private/cover.png)\n\n",
            "- [x] 已完成\n"
        ),
    )
    .expect("write hostile Markdown boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import hostile Markdown");
    let opened = library.open_book(&imported.id).expect("open imported book");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse manifest");
    let href = manifest["sections"][0]["href"]
        .as_str()
        .expect("section href");
    let section = opened.root.read(&format!("/{href}")).expect("read section");
    let section = std::str::from_utf8(&section.bytes).expect("generated XHTML is UTF-8");

    assert!(section.contains("&lt;script&gt;globalThis.pwned = true&lt;/script&gt;"));
    assert!(section.contains("可见链接"));
    assert!(section.contains("远程图片"));
    assert!(section.contains("本机图片"));
    assert!(section.contains("[x] 已完成"));
    for forbidden in [
        "<script",
        "<a ",
        "<img",
        "<input",
        "javascript:",
        "https://",
        "file://",
        "must-not-render",
    ] {
        assert!(
            !section.contains(forbidden),
            "forbidden capability: {forbidden}"
        );
    }
}

#[test]
fn markdown_accepts_1000_sections_and_rejects_the_next() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");
    let at_limit = root.0.join("at-limit.md");
    write_markdown_headings(&at_limit, 1_000);

    let imported = library.import(&at_limit).expect("accept 1,000 sections");
    let opened = library.open_book(&imported.id).expect("open at-limit book");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read at-limit manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse at-limit manifest");
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(1_000));
    assert_eq!(manifest["toc"].as_array().map(Vec::len), Some(1_000));

    let over_limit = root.0.join("over-limit.md");
    write_markdown_headings(&over_limit, 1_001);
    let error = library
        .import(&over_limit)
        .expect_err("reject more than 1,000 sections");
    assert_eq!(error.code(), "too-many-markdown-sections");
}

#[test]
fn markdown_toc_labels_fit_the_reader_utf16_contract() {
    let root = TestRoot::new();
    let source = root.0.join("long-label.md");
    fs::write(&source, format!("# {}\nbody", "😀".repeat(300)))
        .expect("write UTF-16 label boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import long heading");
    let manifest = read_manifest(&library, &imported.id);
    let label = manifest["toc"][0]["label"].as_str().expect("TOC label");
    assert_eq!(label.encode_utf16().count(), 256);
    assert_eq!(label.chars().count(), 128);
}

#[test]
fn markdown_enforces_source_and_generated_section_budgets() {
    const RESOURCE_BYTES: usize = 16 * 1024 * 1024;
    let root = TestRoot::new();
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");
    let oversized_source = root.0.join("oversized-source.md");
    fs::write(&oversized_source, vec![b'a'; RESOURCE_BYTES + 1])
        .expect("write Markdown source boundary");
    let source_error = library
        .import(&oversized_source)
        .expect_err("reject oversized Markdown source");
    assert_eq!(source_error.code(), "markdown-source-too-large");

    let expanding_section = root.0.join("expanding-section.md");
    let mut source = String::from("# title\n");
    source.push_str(&"&".repeat(3_400_000));
    fs::write(&expanding_section, source).expect("write expanding Markdown section boundary");
    let section_error = library
        .import(&expanding_section)
        .expect_err("reject oversized generated XHTML");
    assert_eq!(section_error.code(), "markdown-section-too-large");

    let invalid_xml = root.0.join("invalid-xml-character.md");
    fs::write(&invalid_xml, "# title\ninvalid\u{1}character")
        .expect("write invalid XML character boundary");
    let encoding_error = library
        .import(&invalid_xml)
        .expect_err("reject characters that cannot enter XHTML");
    assert_eq!(encoding_error.code(), "invalid-markdown-encoding");
}

#[test]
fn identical_markdown_and_txt_bytes_have_distinct_book_identities() {
    let root = TestRoot::new();
    let hostile_bytes = b"# same bytes\n<script>must stay inert</script>\n";
    let markdown = root.0.join("same.md");
    let txt = root.0.join("same.txt");
    fs::write(&markdown, hostile_bytes).expect("write Markdown identity boundary");
    fs::write(&txt, hostile_bytes).expect("write TXT identity boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let markdown = library.import(&markdown).expect("import Markdown bytes");
    let txt = library.import(&txt).expect("import TXT bytes");

    assert_ne!(markdown.id, txt.id);
    assert_eq!(library.list().expect("list distinct projections").len(), 2);
    assert!(library.open_book(&markdown.id).is_ok());
    assert!(library.open_book(&txt.id).is_ok());
}

#[test]
fn title_hint_is_sanitized_bounded_and_lower_priority_than_importer_metadata() {
    let root = TestRoot::new();
    let txt = root.0.join("hint.txt");
    fs::write(&txt, "正文").expect("write title-hint boundary");
    let markdown = root.0.join("metadata.md");
    fs::write(&markdown, "# Importer title\nbody").expect("write metadata priority boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");
    let hint = format!("\0\t  Hint\u{7f} {}", "x".repeat(600));

    let hinted = library
        .import_with_title_hint(&txt, Some(&hint))
        .expect("import TXT with title hint");
    assert!(hinted.title.starts_with("Hint "));
    assert_eq!(hinted.title.chars().count(), 512);
    assert!(!hinted.title.chars().any(char::is_control));

    let metadata = library
        .import_with_title_hint(&markdown, Some("must not win"))
        .expect("import Markdown with metadata title");
    assert_eq!(metadata.title, "Importer title");
}

#[test]
fn txt_requires_two_main_chapters_before_semantic_splitting() {
    let root = TestRoot::new();
    let specials_only = root.0.join("specials-only.txt");
    fs::write(&specials_only, "前言\r\n普通正文\r后记\n收尾正文")
        .expect("write special-heading boundary");
    let chapters = root.0.join("chapters.txt");
    fs::write(
        &chapters,
        "前置正文\n第1章 开始\n这里是开篇正文\n第2章 继续\n这里是后续正文\n后记\n收尾正文",
    )
    .expect("write main-heading boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let specials_only = library
        .import(&specials_only)
        .expect("import specials-only TXT");
    let specials_manifest = read_manifest(&library, &specials_only.id);
    assert_eq!(
        specials_manifest["sections"].as_array().map(Vec::len),
        Some(1)
    );

    let chapters = library.import(&chapters).expect("import chaptered TXT");
    let chapter_manifest = read_manifest(&library, &chapters.id);
    assert_eq!(
        chapter_manifest["sections"].as_array().map(Vec::len),
        Some(2),
        "prelude plus one grouped chapter section"
    );
    assert_eq!(chapter_manifest["toc"].as_array().map(Vec::len), Some(3));
    assert_eq!(chapter_manifest["toc"][0]["label"], "第1章 开始");
    assert_eq!(chapter_manifest["toc"][2]["label"], "后记");
    assert_eq!(
        chapter_manifest["toc"][0]["href"],
        ".atha-text/section-0002.xhtml#chapter-0001"
    );
    assert_eq!(
        chapter_manifest["toc"][2]["href"],
        ".atha-text/section-0002.xhtml#chapter-0003"
    );
}

#[test]
fn txt_groups_physical_sections_without_losing_chapter_toc() {
    let root = TestRoot::new();
    let source = root.0.join("grouped-chapters.txt");
    let mut text = String::from("前置正文\n");
    for chapter in 1..=4 {
        text.push_str(&format!("第{chapter}章 标题\n"));
        text.push_str(&"正文".repeat(100_000));
        text.push('\n');
    }
    fs::write(&source, text).expect("write grouped TXT boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import grouped TXT");
    let manifest = read_manifest(&library, &imported.id);
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(3));
    assert_eq!(manifest["toc"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        manifest["toc"][0]["href"],
        ".atha-text/section-0002.xhtml#chapter-0001"
    );
    assert_eq!(
        manifest["toc"][1]["href"],
        ".atha-text/section-0002.xhtml#chapter-0002"
    );
    assert_eq!(
        manifest["toc"][2]["href"],
        ".atha-text/section-0003.xhtml#chapter-0003"
    );
    assert_eq!(
        manifest["toc"][3]["href"],
        ".atha-text/section-0003.xhtml#chapter-0004"
    );
}

#[test]
fn txt_decodes_supported_boms_and_rejects_bomless_utf16() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");
    let text = "第一行\n第二行";
    let utf8 = root.0.join("utf8-bom.txt");
    let mut utf8_bytes = vec![0xEF, 0xBB, 0xBF];
    utf8_bytes.extend_from_slice(text.as_bytes());
    fs::write(&utf8, utf8_bytes).expect("write UTF-8 BOM boundary");
    let utf16le = root.0.join("utf16le-bom.txt");
    fs::write(&utf16le, utf16_bytes(text, true, true)).expect("write UTF-16LE boundary");
    let utf16be = root.0.join("utf16be-bom.txt");
    fs::write(&utf16be, utf16_bytes(text, false, true)).expect("write UTF-16BE boundary");

    for source in [&utf8, &utf16le, &utf16be] {
        let imported = library.import(source).expect("import BOM TXT");
        let section = first_section_text(&library, &imported.id);
        assert!(section.contains("第一行"));
        assert!(section.contains("第二行"));
    }

    let bomless = root.0.join("utf16-without-bom.txt");
    fs::write(&bomless, utf16_bytes(text, true, false)).expect("write BOM-less UTF-16 boundary");
    let error = library
        .import(&bomless)
        .expect_err("reject BOM-less UTF-16");
    assert_eq!(error.code(), "invalid-txt-encoding");
}

#[test]
fn txt_keeps_multibyte_characters_and_mixed_line_endings_across_read_chunks() {
    let root = TestRoot::new();
    let source = root.0.join("chunk-boundary.txt");
    let mut text = "a".repeat(65_535);
    text.push_str("界\r\nsecond\rthird\nfourth");
    fs::write(&source, text).expect("write decoding chunk boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import chunked UTF-8 TXT");
    let section = first_section_text(&library, &imported.id);
    assert!(section.contains('界'));
    assert!(section.contains("<p>second</p>"));
    assert!(section.contains("<p>third</p>"));
    assert!(section.contains("<p>fourth</p>"));
}

#[test]
fn txt_uses_bounded_legacy_encoding_detection_after_strict_utf8_fails() {
    let root = TestRoot::new();
    let source = root.0.join("legacy-encoding.txt");
    let text = "中文内容".repeat(128);
    let (bytes, _, had_errors) = GBK.encode(&text);
    assert!(!had_errors);
    fs::write(&source, bytes.as_ref()).expect("write legacy encoding boundary");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import detected legacy TXT");
    let section = first_section_text(&library, &imported.id);
    assert!(section.contains("中文内容"));
    assert!(!section.contains('\u{FFFD}'));
}

#[test]
fn txt_accepts_2000_chapter_toc_items_and_rejects_the_next() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");
    let at_limit = root.0.join("at-limit.txt");
    write_txt_chapters(&at_limit, 2_000);

    let imported = library
        .import(&at_limit)
        .expect("accept 2,000 TXT chapter TOC items");
    let manifest = read_manifest(&library, &imported.id);
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(1));
    assert_eq!(manifest["toc"].as_array().map(Vec::len), Some(2_000));
    assert_eq!(
        manifest["toc"][0]["href"],
        ".atha-text/section-0001.xhtml#chapter-0001"
    );
    assert_eq!(
        manifest["toc"][1_999]["href"],
        ".atha-text/section-0001.xhtml#chapter-2000"
    );

    let over_limit = root.0.join("over-limit.txt");
    write_txt_chapters(&over_limit, 2_001);
    let error = library
        .import(&over_limit)
        .expect_err("reject more than 2,000 TXT chapter TOC items");
    assert_eq!(error.code(), "too-many-txt-sections");
}

#[test]
fn txt_rejects_truncated_bom_text_and_oversized_lines_without_over_splitting() {
    const RESOURCE_BYTES: usize = 16 * 1024 * 1024;
    let root = TestRoot::new();
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let truncated = root.0.join("truncated-utf8.txt");
    fs::write(&truncated, [0xEF, 0xBB, 0xBF, 0xE4, 0xB8]).expect("write truncated UTF-8 boundary");
    let encoding_error = library
        .import(&truncated)
        .expect_err("reject truncated BOM text");
    assert_eq!(encoding_error.code(), "invalid-txt-encoding");

    let single_candidate = root.0.join("single-candidate.txt");
    fs::write(
        &single_candidate,
        "普通正文\n第1章 只有一个疑似标题\n后续正文",
    )
    .expect("write single chapter-candidate boundary");
    let single = library
        .import(&single_candidate)
        .expect("import single chapter candidate");
    let manifest = read_manifest(&library, &single.id);
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(1));

    let oversized_line = root.0.join("oversized-line.txt");
    fs::write(&oversized_line, vec![b'a'; RESOURCE_BYTES + 1])
        .expect("write oversized line boundary");
    let section_error = library
        .import(&oversized_line)
        .expect_err("reject oversized logical line");
    assert_eq!(section_error.code(), "txt-section-too-large");
}

fn write_txt_chapters(path: &std::path::Path, count: usize) {
    let mut source = String::new();
    for index in 1..=count {
        source.push('第');
        source.push_str(&index.to_string());
        source.push_str("章 title\n正文\n");
    }
    fs::write(path, source).expect("write TXT section boundary");
}

#[test]
#[ignore = "requires the private local TXT sample selected by the target-platform gate"]
fn imports_private_local_txt_sample() {
    let source = std::env::var_os("ATHA_LOCAL_TXT_SAMPLE")
        .expect("ATHA_LOCAL_TXT_SAMPLE is required for the private local gate");
    let root = TestRoot::new();
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library
        .import(PathBuf::from(source))
        .expect("import private local TXT sample");
    let opened = library
        .open_book(&imported.id)
        .expect("open private local TXT sample");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read private local manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse private local manifest");
    assert_eq!(manifest["toc"].as_array().map(Vec::len), Some(1_134));
    let sections = manifest["sections"].as_array().expect("private sections");
    assert!((2..=16).contains(&sections.len()));
    for index in [0, sections.len() - 1] {
        let href = sections[index]["href"]
            .as_str()
            .expect("private section href");
        let section = opened
            .root
            .read(&format!("/{href}"))
            .expect("read private section boundary");
        let section = std::str::from_utf8(&section.bytes).expect("private section is UTF-8");
        assert!(!section.contains('\u{FFFD}'));
        assert!(!section.is_ascii());
    }
}

fn utf16_bytes(value: &str, little_endian: bool, with_bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if with_bom {
        bytes.extend_from_slice(if little_endian {
            &[0xFF, 0xFE]
        } else {
            &[0xFE, 0xFF]
        });
    }
    for unit in value.encode_utf16() {
        let encoded = if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        bytes.extend_from_slice(&encoded);
    }
    bytes
}

fn first_section_text(library: &LocalLibrary, id: &str) -> String {
    let opened = library.open_book(id).expect("open imported text");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read text manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse text manifest");
    let href = manifest["sections"][0]["href"]
        .as_str()
        .expect("first section href");
    let section = opened.root.read(&format!("/{href}")).expect("read section");
    String::from_utf8(section.bytes).expect("generated section is UTF-8")
}

fn read_manifest(library: &LocalLibrary, id: &str) -> serde_json::Value {
    let opened = library.open_book(id).expect("open imported text");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read text manifest");
    serde_json::from_slice(&manifest.bytes).expect("parse text manifest")
}

fn write_markdown_headings(path: &std::path::Path, count: usize) {
    let mut source = String::new();
    for index in 0..count {
        source.push_str("# 章节 ");
        source.push_str(&index.to_string());
        source.push_str("\n正文\n");
    }
    fs::write(path, source).expect("write Markdown section boundary");
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
                "atha-text-import-{}-{nonce}-{}",
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
