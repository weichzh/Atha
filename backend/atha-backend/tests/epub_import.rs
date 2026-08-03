use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{
    epub::{ImportError, READER_MANIFEST, import_epub},
    library::{LibraryError, LocalLibrary},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[derive(Clone, Copy)]
enum EpubVariant {
    Valid,
    Changed,
    InvalidMimetype,
    UnsafePath,
    Doctype,
    ExtraContainerRoot,
    ExtraPackageRoot,
    MultipleRootfiles,
    ExternalReference,
    Encryption,
    TruncatedNavigation,
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("atha-epub-import-{}-{nonce}", std::process::id()));
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
fn imports_epub_and_rejects_unsafe_or_unsupported_sources() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let cache = root.0.join("cache");
    let imported = import_epub(&source, &cache).expect("import epub");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");

    assert_eq!(manifest["contentVersion"], imported.content_version);
    assert_eq!(manifest["sections"].as_array().expect("sections").len(), 2);
    assert_eq!(manifest["sections"][0]["href"], "OEBPS/text/one.xhtml");
    assert_eq!(manifest["toc"].as_array().expect("toc").len(), 2);
    assert_eq!(manifest["toc"][1]["href"], "OEBPS/text/two.xhtml#start");
    assert_eq!(
        manifest["resources"],
        serde_json::json!(["OEBPS/images/cover.png", "OEBPS/styles/book.css"])
    );
    assert_eq!(imported.title.as_deref(), Some("Example Book"));
    assert_eq!(imported.authors, ["Example Author"]);
    assert_eq!(
        imported.cover_path.as_deref(),
        Some("OEBPS/images/cover.png")
    );
    assert!(imported.root.join("OEBPS/text/two.xhtml").is_file());

    let moved = root.0.join("moved.epub");
    fs::copy(&source, &moved).expect("copy epub");
    let repeated = import_epub(&moved, &cache).expect("reuse import");
    assert_eq!(repeated, imported);

    for (name, variant, expected) in [
        (
            "invalid-mimetype.epub",
            EpubVariant::InvalidMimetype,
            ImportError::UnsupportedEpub,
        ),
        (
            "unsafe.epub",
            EpubVariant::UnsafePath,
            ImportError::UnsafePath,
        ),
        (
            "doctype.epub",
            EpubVariant::Doctype,
            ImportError::InvalidXml,
        ),
        (
            "extra-container-root.epub",
            EpubVariant::ExtraContainerRoot,
            ImportError::InvalidXml,
        ),
        (
            "extra-package-root.epub",
            EpubVariant::ExtraPackageRoot,
            ImportError::InvalidXml,
        ),
        (
            "external.epub",
            EpubVariant::ExternalReference,
            ImportError::UnsafePath,
        ),
        (
            "multiple-rootfiles.epub",
            EpubVariant::MultipleRootfiles,
            ImportError::UnsupportedEpub,
        ),
        (
            "encrypted.epub",
            EpubVariant::Encryption,
            ImportError::Encrypted,
        ),
        (
            "truncated-nav.epub",
            EpubVariant::TruncatedNavigation,
            ImportError::InvalidXml,
        ),
    ] {
        let rejected = root.0.join(name);
        write_epub(&rejected, variant);
        assert_eq!(import_epub(&rejected, &cache), Err(expected));
    }

    let invalid_archive = root.0.join("invalid-archive.epub");
    fs::write(&invalid_archive, b"not a ZIP archive").expect("write invalid archive");
    assert_eq!(
        import_epub(&invalid_archive, &cache),
        Err(ImportError::InvalidArchive)
    );

    let oversized = root.0.join("oversized.epub");
    File::create(&oversized)
        .expect("create oversized epub")
        .set_len(512 * 1024 * 1024 + 1)
        .expect("size oversized epub");
    assert_eq!(
        import_epub(&oversized, &cache),
        Err(ImportError::SourceTooLarge)
    );
    assert_eq!(fs::read_dir(cache).expect("read cache").count(), 1);

    let changed_source = root.0.join("changed.epub");
    write_epub(&changed_source, EpubVariant::Changed);
    let changed = import_epub(&changed_source, root.0.join("cache")).expect("import changed epub");
    assert_ne!(changed.content_version, imported.content_version);
    assert_ne!(changed.root, imported.root);
}

#[test]
fn local_library_deduplicates_opens_and_removes_books_without_deleting_content() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let data = root.0.join("data");
    let library = LocalLibrary::open(&data).expect("open library");

    let imported = library.import(&source).expect("import into library");
    assert_eq!(imported.title, "Example Book");
    assert_eq!(imported.authors, ["Example Author"]);
    assert!(imported.has_cover);

    let moved = root.0.join("moved.epub");
    fs::copy(&source, &moved).expect("copy epub");
    assert_eq!(library.import(&moved).expect("repeat import"), imported);
    let listed = library.list().expect("list library");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], imported);

    let opened = library.open_book(&imported.id).expect("open book");
    assert_eq!(opened.book, imported);
    assert!(opened.root.read(&format!("/{READER_MANIFEST}")).is_ok());
    assert_eq!(
        library
            .cover(&opened.book.id)
            .expect("read cover")
            .content_type,
        "image/png"
    );

    let reopened = LocalLibrary::open(&data).expect("reopen library");
    let listed = reopened.list().expect("list reopened");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], opened.book);
    reopened.remove(&opened.book.id).expect("remove book");
    assert!(reopened.list().expect("list empty").is_empty());
    assert!(data.join("ImportedBooks").join(&opened.book.id).is_dir());
    assert_eq!(
        reopened.import(&moved).expect("restore book").id,
        opened.book.id
    );
    assert_eq!(
        reopened.open_book("not-a-content-hash").unwrap_err(),
        LibraryError::InvalidBookId
    );
    fs::write(
        data.join("Library")
            .join(format!("{}.json", opened.book.id)),
        b"{}",
    )
    .expect("corrupt library record");
    assert_eq!(reopened.list().unwrap_err(), LibraryError::CorruptRecord);
}

fn write_epub(path: &Path, variant: EpubVariant) {
    let file = File::create(path).expect("create epub");
    let mut archive = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    archive.start_file("mimetype", stored).expect("mimetype");
    let mimetype = if matches!(variant, EpubVariant::InvalidMimetype) {
        b"application/zip".as_slice()
    } else {
        b"application/epub+zip".as_slice()
    };
    archive.write_all(mimetype).expect("write mimetype");
    if matches!(variant, EpubVariant::UnsafePath) {
        archive
            .start_file("../escape", stored)
            .expect("unsafe path");
        archive.write_all(b"escape").expect("write unsafe path");
        archive.finish().expect("finish unsafe epub");
        return;
    }
    if matches!(variant, EpubVariant::Encryption) {
        archive
            .start_file("META-INF/encryption.xml", stored)
            .expect("encryption marker");
        archive
            .write_all(b"<encryption/>")
            .expect("write encryption marker");
    }
    let container = match variant {
        EpubVariant::Doctype => br#"<?xml version="1.0"?><!DOCTYPE container><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.as_slice(),
        EpubVariant::ExtraContainerRoot => br#"<?xml version="1.0"?><extra/><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.as_slice(),
        EpubVariant::MultipleRootfiles => br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/><rootfile full-path="OEBPS/other.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.as_slice(),
        _ => br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.as_slice(),
    };
    let package = if matches!(variant, EpubVariant::ExternalReference) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="https://example.com/one.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/></spine></package>"#.as_slice()
    } else if matches!(variant, EpubVariant::ExtraPackageRoot) {
        br#"<?xml version="1.0"?><extra/><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/></spine></package>"#.as_slice()
    } else {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/" version="3.0"><metadata><dc:title> Example   Book </dc:title><dc:creator>Example Author</dc:creator></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"></item><item id="css" href="styles/book.css" media-type="text/css"/><item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/></manifest><spine><itemref idref="one"/><itemref idref="two"></itemref></spine></package>"#.as_slice()
    };
    let navigation = if matches!(variant, EpubVariant::TruncatedNavigation) {
        br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One"#.as_slice()
    } else {
        br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One</a></li><li><a href="text/two.xhtml#start"><span>Two</span></a></li></ol></nav></body></html>"#.as_slice()
    };
    for (name, bytes) in [
        ("META-INF/container.xml", container),
        ("OEBPS/book.opf", package),
        ("OEBPS/nav.xhtml", navigation),
        (
            "OEBPS/text/one.xhtml",
            br#"<html xmlns="http://www.w3.org/1999/xhtml"><body>One</body></html>"#.as_slice(),
        ),
        (
            "OEBPS/text/two.xhtml",
            br#"<html xmlns="http://www.w3.org/1999/xhtml"><body id="start">Two</body></html>"#
                .as_slice(),
        ),
        (
            "OEBPS/styles/book.css",
            b"body { color: black; }".as_slice(),
        ),
        ("OEBPS/images/cover.png", b"test-png".as_slice()),
    ] {
        archive.start_file(name, stored).expect("start epub member");
        archive.write_all(bytes).expect("write epub member");
    }
    if matches!(variant, EpubVariant::Changed) {
        archive
            .start_file("OEBPS/unused.bin", stored)
            .expect("changed member");
        archive.write_all(b"changed").expect("write changed member");
    }
    archive.finish().expect("finish epub");
}
