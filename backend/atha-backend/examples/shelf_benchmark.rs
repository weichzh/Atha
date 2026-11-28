//! Opt-in synthetic shelf fixture and release timing; never opens a live profile.
use std::{
    env,
    fs::{self, File},
    hint::black_box,
    io::Write,
    path::Path,
    time::Instant,
};

use atha_backend::reader::library::{BookOrganization, LibraryBook, LocalLibrary};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use zip::{ZipWriter, write::SimpleFileOptions};

const COUNT: usize = 256;
const PREPARED: usize = 208;
const PROFILE: &str = "com.atha.reader";
const FIXTURE: &str = ".atha-shelf-benchmark-v1";
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn main() {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    assert!(
        args.len() == 2,
        "usage: shelf_benchmark seed|startup|return NEW_OR_SEEDED_RUN_ROOT"
    );
    let root = Path::new(&args[1]);
    match args[0].to_str() {
        Some("seed") => seed(root),
        Some(phase @ ("startup" | "return")) => measure(root, phase),
        _ => panic!("unknown shelf benchmark command"),
    }
}

fn seed(root: &Path) {
    // create_dir, not create_dir_all: refuse to overlay any existing fixture or profile.
    fs::create_dir(root).expect("use a new fixture directory under an existing parent");
    let inputs = root.join("inputs");
    fs::create_dir(&inputs).unwrap();
    let png = STANDARD.decode(PNG).unwrap();
    let cover = inputs.join("custom.png");
    fs::write(&cover, &png).unwrap();
    let library = LocalLibrary::open(root.join(PROFILE)).unwrap();
    let mut open_id = String::new();
    for i in 0..COUNT {
        let prepared = i < PREPARED;
        let extension = if i < 96 {
            "fb2"
        } else if i < 160 {
            "epub"
        } else if i < 192 {
            "md"
        } else if i < PREPARED {
            "txt"
        } else {
            ["fb2", "epub", "md", "txt"][i % 4]
        };
        let title = format!("Benchmark {i:03}");
        let source = inputs.join(format!("{title}.{extension}"));
        match extension {
            "fb2" => write_fb2(&source, &title, i % 2 == 0),
            "epub" => write_epub(&source, &title, (i % 2 == 0).then_some(png.as_slice())),
            "md" => fs::write(
                &source,
                format!("# {title}\n\nA synthetic reading section.\n"),
            )
            .unwrap(),
            "txt" => fs::write(
                &source,
                format!("{title}\n\nA synthetic reading section.\n"),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let book = library
            .stage_with_title_hint(&source, Some(&title))
            .unwrap();
        assert!(!book.prepared);
        if prepared {
            assert!(library.open_book(&book.id).unwrap().book.prepared);
        }
        if i % 4 == 0 {
            library
                .update_book_organization(
                    &book.id,
                    BookOrganization {
                        favorite: Some(true),
                        tags: Some(vec![format!("Benchmark tag {}", (i / 4) % 8)]),
                        ..BookOrganization::default()
                    },
                )
                .unwrap();
        }
        if i % 8 == 0 {
            library.set_custom_cover(&book.id, &cover).unwrap();
        }
        if i == 0 {
            open_id = book.id;
        }
    }
    assert_shape(&library.list().unwrap(), &open_id);
    fs::write(root.join(FIXTURE), open_id).unwrap();
}

fn write_fb2(path: &Path, title: &str, cover: bool) {
    let (image, binary) = if cover {
        (
            r##"<coverpage><image l:href="#cover.png"/></coverpage>"##,
            format!(r#"<binary id="cover.png" content-type="image/png">{PNG}</binary>"#),
        )
    } else {
        ("", String::new())
    };
    fs::write(
        path,
        format!(
            r#"<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
  <description><title-info><genre>adventure</genre><author><nickname>Shelf Fixture</nickname></author>
  <book-title>{title}</book-title><sequence name="Shelf Cycle" number="1"/>{image}</title-info></description>
  <body><section><title><p>{title}</p></title><p>A synthetic reading section.</p></section></body>{binary}
</FictionBook>"#
        ),
    )
    .unwrap();
}

fn write_epub(path: &Path, title: &str, cover: Option<&[u8]>) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    let mut write = |name, data: &[u8]| {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    };
    write("mimetype", b"application/epub+zip");
    write(
        "META-INF/container.xml",
        br#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
    );
    let image = if cover.is_some() {
        r#"<item id="cover" href="cover.png" media-type="image/png" properties="cover-image"/>"#
    } else {
        ""
    };
    write(
        "package.opf",
        format!(
            r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{title}</dc:title><dc:language>en</dc:language><dc:identifier id="id">urn:benchmark:{title}</dc:identifier><dc:creator>Shelf Fixture</dc:creator><dc:subject>Fiction</dc:subject></metadata>
<manifest><item id="chapter" href="chapter.xhtml" media-type="application/xhtml+xml"/>{image}</manifest>
<spine><itemref idref="chapter"/></spine></package>"#
        )
        .as_bytes(),
    );
    write(
        "chapter.xhtml",
        format!(
            r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>{title}</title></head><body><h1>{title}</h1><p>A synthetic reading section.</p></body></html>"#
        )
        .as_bytes(),
    );
    if let Some(bytes) = cover {
        write("cover.png", bytes);
    }
    zip.finish().unwrap();
}

fn assert_shape(books: &[LibraryBook], open_id: &str) {
    assert_eq!(books.len(), COUNT);
    assert_eq!(books.iter().filter(|book| book.prepared).count(), PREPARED);
    assert_eq!(books.iter().filter(|book| book.favorite).count(), 64);
    assert_eq!(
        books.iter().filter(|book| book.has_custom_cover).count(),
        32
    );
    assert_eq!(books.iter().filter(|book| book.has_cover).count(), 92);
    assert_eq!(
        books
            .iter()
            .filter(|book| !book.subjects.is_empty())
            .count(),
        160
    );
    assert!(books.iter().any(|book| book.id == open_id && book.prepared));
}

fn measure(root: &Path, phase: &str) {
    let marker = root.join(FIXTURE);
    assert!(
        fs::symlink_metadata(&marker).is_ok_and(|meta| meta.is_file()),
        "use a fixture created by shelf_benchmark seed"
    );
    let id = fs::read_to_string(marker).expect("unreadable synthetic shelf marker");
    assert!(
        id.len() == 64
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    );
    let profile = root.join(PROFILE);
    assert!(
        fs::symlink_metadata(&profile).is_ok_and(|meta| meta.is_dir()),
        "missing synthetic shelf profile"
    );
    let started = Instant::now();
    let library = LocalLibrary::open(profile).unwrap();
    let library_open_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let open_book_ms = if phase == "return" {
        // Mirror an already-used shelf, then open the same prepared FB2 as the native probe.
        black_box(library.list().unwrap());
        let opened = Instant::now();
        let book = library.open_book(&id).unwrap();
        let elapsed = opened.elapsed().as_secs_f64() * 1_000.0;
        assert!(book.book.prepared);
        black_box(book);
        Some(elapsed)
    } else {
        None
    };
    let started = Instant::now();
    let books = library.list().unwrap();
    let list_ms = started.elapsed().as_secs_f64() * 1_000.0;
    assert_shape(&books, &id);
    println!(
        "{}",
        json!({
            "phase": phase,
            "books": books.len(),
            "library_open_ms": library_open_ms,
            "open_book_ms": open_book_ms,
            "list_ms": list_ms,
            "startup_library_ms": (phase == "startup").then_some(library_open_ms + list_ms),
        })
    );
}
