use std::{
    fs::{self, File},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{
    cbz::{ImportError, import_cbz},
    library::LocalLibrary,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

#[test]
fn imports_cbz_as_naturally_sorted_image_sections() {
    let root = TestRoot::new();
    let source = root.0.join("natural-pages.cbz");
    write_cbz(&source);
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import CBZ");
    assert_eq!(imported.title, "natural-pages");
    assert!(imported.authors.is_empty());
    assert!(imported.has_cover);

    let opened = library.open_book(&imported.id).expect("open imported CBZ");
    let manifest = opened
        .root
        .read("/.atha-reader.json")
        .expect("read reader manifest");
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest.bytes).expect("parse reader manifest");
    assert_eq!(
        manifest["sections"],
        serde_json::json!([
            {"id": "page-0001", "href": ".atha-cbz/page-0001.xhtml"},
            {"id": "page-0002", "href": ".atha-cbz/page-0002.xhtml"},
            {"id": "page-0003", "href": ".atha-cbz/page-0003.xhtml"}
        ])
    );
    assert_eq!(
        manifest["toc"],
        serde_json::json!([
            {"label": "pages/1.png", "href": ".atha-cbz/page-0001.xhtml"},
            {"label": "pages/2.png", "href": ".atha-cbz/page-0002.xhtml"},
            {"label": "pages/10.png", "href": ".atha-cbz/page-0003.xhtml"}
        ])
    );
    assert_eq!(
        manifest["resources"],
        serde_json::json!([
            ".atha-cbz/images/page-0001.png",
            ".atha-cbz/images/page-0002.png",
            ".atha-cbz/images/page-0003.png"
        ])
    );
    let first_page = opened
        .root
        .read("/.atha-cbz/page-0001.xhtml")
        .expect("read generated page");
    assert_eq!(
        first_page.content_type,
        "application/xhtml+xml; charset=utf-8"
    );
    let first_page = std::str::from_utf8(&first_page.bytes).expect("generated page is UTF-8");
    assert!(first_page.contains("class=\"atha-cbz-page\""));
    assert!(first_page.contains("src=\"images/page-0001.png\""));

    let cover = library.cover(&imported.id).expect("read CBZ cover");
    assert_eq!(cover.content_type, "image/png");
    assert_eq!(cover.bytes, PNG_1X1);

    let moved = root.0.join("moved.cbz");
    fs::copy(&source, &moved).expect("copy identical CBZ");
    let duplicate = library.import(&moved).expect("reuse content-addressed CBZ");
    assert_eq!(duplicate.id, imported.id);
    assert_eq!(library.list().expect("list deduplicated library").len(), 1);
}

#[test]
fn natural_page_order_is_total_across_path_segments_and_leading_zeroes() {
    let root = TestRoot::new();
    let source = root.0.join("total-order.cbz");
    write_members(
        &source,
        &[
            ("0a/1.png", PNG_1X1),
            ("0b/0.png", PNG_1X1),
            ("00/1.png", PNG_1X1),
        ],
    );

    let imported = import_cbz(&source, root.0.join("cache")).expect("import total-order CBZ");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(".atha-reader.json")).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(
        manifest["toc"],
        serde_json::json!([
            {"label": "00/1.png", "href": ".atha-cbz/page-0001.xhtml"},
            {"label": "0a/1.png", "href": ".atha-cbz/page-0002.xhtml"},
            {"label": "0b/0.png", "href": ".atha-cbz/page-0003.xhtml"}
        ])
    );
}

#[test]
fn imports_bounded_comicinfo_fields_and_declared_front_cover() {
    let root = TestRoot::new();
    let source = root.0.join("comicinfo.cbz");
    let second_page = [PNG_1X1, b"second-page"].concat();
    write_members(
        &source,
        &[
            ("pages/1.png", PNG_1X1),
            ("pages/2.png", &second_page),
            (
                "ComicInfo.xml",
                br#"<?xml version="1.0"?><ComicInfo><Title>Self-authored CBZ</Title><Writer>Ada, Lin</Writer><Pages><Page Image="1" Type="FrontCover" /></Pages></ComicInfo>"#,
            ),
        ],
    );

    let imported = import_cbz(&source, root.0.join("cache")).expect("import CBZ metadata");

    assert_eq!(imported.title.as_deref(), Some("Self-authored CBZ"));
    assert_eq!(imported.authors, ["Ada, Lin"]);
    assert_eq!(
        imported.cover_path.as_deref(),
        Some(".atha-cbz/images/page-0002.png")
    );
    assert_eq!(
        fs::read(imported.root.join(imported.cover_path.expect("cover path")))
            .expect("read declared cover"),
        second_page
    );
}

#[test]
fn ignores_invalid_or_ambiguous_comicinfo() {
    let root = TestRoot::new();
    let cases = [
        (
            "malformed",
            &[("ComicInfo.xml", b"<ComicInfo><Title>broken" as &[u8])]
                as &[(&str, &[u8])],
        ),
        (
            "duplicate-title",
            &[(
                "ComicInfo.xml",
                b"<ComicInfo><Title>one</Title><Title>two</Title></ComicInfo>",
            )],
        ),
        (
            "ambiguous-nested",
            &[
                (
                    "a/ComicInfo.xml",
                    b"<ComicInfo><Title>one</Title></ComicInfo>",
                ),
                (
                    "b/ComicInfo.xml",
                    b"<ComicInfo><Title>two</Title></ComicInfo>",
                ),
            ],
        ),
        (
            "ambiguous-cover",
            &[(
                "ComicInfo.xml",
                br#"<ComicInfo><Title>ignored</Title><Pages><Page Image="0" Type="FrontCover"/><Page Image="0" Type="FrontCover"/></Pages></ComicInfo>"#,
            )],
        ),
    ];

    for (name, metadata) in &cases {
        let source = root.0.join(format!("{name}.cbz"));
        let mut members = vec![("page.png", PNG_1X1)];
        members.extend_from_slice(metadata);
        write_members(&source, &members);

        let imported =
            import_cbz(&source, root.0.join(format!("{name}-cache"))).unwrap_or_else(|error| {
                panic!("{name} metadata must not reject valid images: {error}")
            });
        assert_eq!(imported.title, None, "{name}");
        assert!(imported.authors.is_empty(), "{name}");
        assert_eq!(
            imported.cover_path.as_deref(),
            Some(".atha-cbz/images/page-0001.png"),
            "{name}"
        );
    }

    let source = root.0.join("depth-overflow.cbz");
    let deep_metadata = format!(
        "<ComicInfo>{}<Title>ignored</Title>{}</ComicInfo>",
        "<Node>".repeat(64),
        "</Node>".repeat(64)
    );
    write_members(
        &source,
        &[
            ("page.png", PNG_1X1),
            ("ComicInfo.xml", deep_metadata.as_bytes()),
        ],
    );
    let imported = import_cbz(&source, root.0.join("depth-overflow-cache"))
        .expect("depth overflow metadata must not reject valid images");
    assert_eq!(imported.title, None);
    assert!(imported.authors.is_empty());
}

#[test]
fn validates_jpeg_and_png_content_and_dimensions() {
    let root = TestRoot::new();
    let source = root.0.join("supported-images.cbz");
    let jpeg = jpeg_header(2, 3);
    write_members(
        &source,
        &[("1.jpeg", &jpeg), ("2.PNG", &png_with_size(4, 5))],
    );
    import_cbz(&source, root.0.join("supported-cache")).expect("accept bounded JPEG and PNG");

    for (name, filename, bytes) in [
        ("extension-mismatch", "page.png", jpeg_header(1, 1)),
        ("zero-width", "page.png", png_with_size(0, 1)),
        ("side-limit", "page.png", png_with_size(8_193, 1)),
        ("pixel-limit", "page.png", png_with_size(5_000, 5_000)),
    ] {
        let source = root.0.join(format!("{name}.cbz"));
        write_members(&source, &[(filename, &bytes)]);
        assert_eq!(
            import_cbz(&source, root.0.join(format!("{name}-cache"))),
            Err(ImportError::InvalidImage),
            "{name}"
        );
    }
}

#[test]
fn rejects_representative_unsafe_duplicate_and_epub_archives() {
    let root = TestRoot::new();
    for (name, members, expected) in [
        (
            "unsafe-path",
            vec![("../page.png", PNG_1X1)],
            ImportError::UnsafePath,
        ),
        (
            "case-duplicate",
            vec![("Page.png", PNG_1X1), ("page.PNG", PNG_1X1)],
            ImportError::UnsafePath,
        ),
        (
            "epub-mimetype",
            vec![("mimetype", b"application/epub+zip"), ("page.png", PNG_1X1)],
            ImportError::UnsupportedCbz,
        ),
        (
            "epub-container",
            vec![
                ("META-INF/container.xml", b"<container/>"),
                ("page.png", PNG_1X1),
            ],
            ImportError::UnsupportedCbz,
        ),
    ] {
        let source = root.0.join(format!("{name}.cbz"));
        write_members(&source, &members);
        assert_eq!(
            import_cbz(&source, root.0.join(format!("{name}-cache"))),
            Err(expected),
            "{name}"
        );
    }

    let opaque_epub = root.0.join("damaged-epub.book");
    write_members(
        &opaque_epub,
        &[("mimetype", b"application/epub+zip"), ("page.png", PNG_1X1)],
    );
    let library = LocalLibrary::open(root.0.join("opaque-epub-library")).expect("open library");
    assert_eq!(
        library
            .import(&opaque_epub)
            .expect_err("damaged opaque EPUB must not fall back to CBZ")
            .code(),
        "invalid-epub-archive"
    );
}

#[test]
fn imports_content_recognized_cbz_with_opaque_extension() {
    let root = TestRoot::new();
    let source = root.0.join("opaque.book");
    write_cbz(&source);
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("recognize CBZ content");

    assert_eq!(imported.title, "opaque");
    assert!(imported.has_cover);
}

#[test]
fn enforces_empty_archive_and_page_count_boundaries() {
    let root = TestRoot::new();
    let empty = root.0.join("empty.cbz");
    write_members(&empty, &[]);
    assert_eq!(
        import_cbz(&empty, root.0.join("empty-cache")),
        Err(ImportError::InvalidArchive)
    );

    let no_pages = root.0.join("no-pages.cbz");
    write_members(&no_pages, &[("notes.txt", b"not a page")]);
    assert_eq!(
        import_cbz(&no_pages, root.0.join("no-pages-cache")),
        Err(ImportError::UnsupportedCbz)
    );

    let at_limit = root.0.join("one-thousand.cbz");
    write_page_count(&at_limit, 1_000);
    import_cbz(&at_limit, root.0.join("at-limit-cache")).expect("accept 1,000 pages");

    let over_limit = root.0.join("one-thousand-one.cbz");
    write_page_count(&over_limit, 1_001);
    assert_eq!(
        import_cbz(&over_limit, root.0.join("over-limit-cache")),
        Err(ImportError::TooManyPages)
    );
}

#[test]
fn actual_member_output_cannot_hide_behind_a_forged_central_size() {
    const OVER_MEMBER_LIMIT: usize = 16 * 1024 * 1024 + 1;

    let root = TestRoot::new();
    let source = root.0.join("forged-size.cbz");
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("page.png", options)
        .expect("start oversized member");
    writer.write_all(PNG_1X1).expect("write PNG header");
    writer
        .write_all(&vec![0; OVER_MEMBER_LIMIT - PNG_1X1.len()])
        .expect("write oversized output");
    let mut bytes = writer.finish().expect("finish forged archive").into_inner();

    let eocd = bytes
        .windows(4)
        .rposition(|value| value == b"PK\x05\x06")
        .expect("find EOCD");
    let central = usize::try_from(u32::from_le_bytes(
        bytes[eocd + 16..eocd + 20]
            .try_into()
            .expect("central directory offset"),
    ))
    .expect("central directory offset fits usize");
    assert_eq!(&bytes[central..central + 4], b"PK\x01\x02");
    bytes[central + 24..central + 28].copy_from_slice(
        &u32::try_from(PNG_1X1.len())
            .expect("forged member size")
            .to_le_bytes(),
    );
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).expect("open forged archive");
    assert_eq!(
        archive
            .by_name("page.png")
            .expect("find forged member")
            .size(),
        PNG_1X1.len() as u64
    );

    fs::write(&source, bytes).expect("write forged archive");
    assert_eq!(
        import_cbz(&source, root.0.join("cache")),
        Err(ImportError::ArchiveTooLarge)
    );
}

fn write_cbz(path: &Path) {
    write_members(
        path,
        &[
            ("pages/10.png", PNG_1X1),
            ("notes.txt", b"ignored metadata"),
            (".hidden.png", PNG_1X1),
            ("__MACOSX/pages/0.png", PNG_1X1),
            ("pages/2.png", PNG_1X1),
            ("pages/1.png", PNG_1X1),
        ],
    );
}

fn write_members(path: &Path, members: &[(&str, &[u8])]) {
    let file = File::create(path).expect("create CBZ");
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for &(name, bytes) in members {
        archive.start_file(name, options).expect("start CBZ member");
        archive.write_all(bytes).expect("write CBZ member");
    }
    archive.finish().expect("finish CBZ");
}

fn write_page_count(path: &Path, count: usize) {
    let file = File::create(path).expect("create CBZ");
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for page in 0..count {
        archive
            .start_file(format!("page-{page:04}.png"), options)
            .expect("start CBZ page");
        archive.write_all(PNG_1X1).expect("write CBZ page");
    }
    archive.finish().expect("finish CBZ");
}

fn png_with_size(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = PNG_1X1.to_vec();
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    bytes
}

fn jpeg_header(width: u16, height: u16) -> Vec<u8> {
    vec![
        0xff,
        0xd8,
        0xff,
        0xc0,
        0x00,
        0x11,
        0x08,
        (height >> 8) as u8,
        height as u8,
        (width >> 8) as u8,
        width as u8,
        0x03,
        0x01,
        0x11,
        0x00,
        0x02,
        0x11,
        0x00,
        0x03,
        0x11,
        0x00,
        0xff,
        0xd9,
    ]
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
                "atha-cbz-import-{}-{nonce}-{}",
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

#[test]
#[ignore = "writes the deterministic CBZ artifact used by target-platform gates"]
fn writes_cbz_gate_fixture() {
    let temporary_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
    let fixture = temporary_root.join("cbz-gate.cbz");
    let imports = temporary_root.join("cbz-gate-imports");
    fs::create_dir_all(&temporary_root).expect("create gate artifact root");
    if fixture.exists() {
        fs::remove_file(&fixture).expect("remove previous gate fixture");
    }
    if imports.exists() {
        fs::remove_dir_all(&imports).expect("remove previous gate imports");
    }

    write_cbz_gate_fixture(&fixture);
    let imported = import_cbz(&fixture, &imports).expect("prepare CBZ gate book root");
    assert_eq!(imported.root, imports.join(&imported.content_version));
    assert_eq!(imported.title.as_deref(), Some("Atha CBZ Gate 71c9"));
    assert_eq!(imported.authors, ["Gate Writer 71c9"]);
    assert_eq!(
        imported.cover_path.as_deref(),
        Some(".atha-cbz/images/page-0001.png")
    );
    assert!(imported.root.join(".atha-reader.json").is_file());
    println!("fixture_sha256={}", imported.content_version);
}

fn write_cbz_gate_fixture(path: &Path) {
    let page_1 = solid_rgb_png(2_048, 3_072, [0x22, 0x66, 0xaa]);
    let page_2 = solid_rgb_png(3_072, 2_048, [0xcc, 0x66, 0x22]);
    let mut page_3 = solid_rgb_png(2_048, 2_048, [0xaa, 0x22, 0x66]);
    page_3.truncate(33); // Keep a valid PNG signature/IHDR while removing all image data.
    let page_10 = solid_rgb_png(2_400, 3_200, [0x44, 0x99, 0x55]);
    write_members(
        path,
        &[
            ("pages/10.png", &page_10),
            (
                "ComicInfo.xml",
                br#"<?xml version="1.0"?><ComicInfo><Title>Atha CBZ Gate 71c9</Title><Writer>Gate Writer 71c9</Writer><Pages><Page Image="0" Type="FrontCover" /></Pages></ComicInfo>"#,
            ),
            ("notes.txt", b"ignored gate metadata"),
            ("pages/3.png", &page_3),
            ("pages/2.png", &page_2),
            ("pages/1.png", &page_1),
        ],
    );
}

fn solid_rgb_png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let pixels = color.repeat(usize::try_from(width * height).expect("gate PNG pixel count"));
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .add_text_chunk("Description".into(), "Atha CBZ Image 71c9".into())
            .expect("write gate PNG description");
        encoder
            .write_header()
            .expect("write gate PNG header")
            .write_image_data(&pixels)
            .expect("write gate PNG pixels");
    }
    bytes
}
