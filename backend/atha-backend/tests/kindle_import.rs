use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{kindle::import_kindle, library::LocalLibrary};

const SAFE_XHTML: &[u8] = br##"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Gate</title><meta charset="utf-8"/></head>
<body><h1 id="start">PalmDOC gate</h1><p><a href="#end">Continue</a></p><a href="images/missing.png">Broken image link</a><p id="end">Ready.</p></body></html>"##;

#[test]
fn huff_with_large_nontext_records_opens_through_the_library() {
    let root = TestRoot::new();
    let source = root.0.join("bounded.mobi");
    let alternate = root.0.join("same.azw3");
    let text = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Gate</title></head><body><h1>Bounded HUFF gate</h1><p><img recindex="00001" alt="Gate pixel"/></p><mbp:pagebreak/><h2>Next chapter</h2></body></html>"#;
    let png = small_png();
    let (huff, cdic, mut first, mut second) = huff_records(text);
    // Two independent trailer fields must be stripped once from each record.
    first.extend_from_slice(&[0, 0x81]);
    second.extend_from_slice(&[0, 0x81]);

    let mut record_zero = full_record_zero(0x4448, text);
    record_zero.resize(0xf4, 0);
    record_zero[8..10].copy_from_slice(&2_u16.to_be_bytes());
    record_zero[20..24].copy_from_slice(&0xe4_u32.to_be_bytes());
    record_zero[0x68..0x6c].copy_from_slice(&6_u32.to_be_bytes());
    record_zero[0x6c..0x70].copy_from_slice(&5_u32.to_be_bytes());
    record_zero[0x70..0x74].copy_from_slice(&3_u32.to_be_bytes());
    record_zero[0x74..0x78].copy_from_slice(&2_u32.to_be_bytes());
    record_zero[0xf2..0xf4].copy_from_slice(&3_u16.to_be_bytes());
    // A photo-heavy book can have more than 2,000 individually small assets.
    let mut records: Vec<&[u8]> = vec![&record_zero, &first, &second, &huff, &cdic];
    records.extend(std::iter::repeat_n(png.as_slice(), 2_001));
    records.push(b"EOF");
    let bytes = palm_records(&records);
    // A large trailing non-text record crosses the old whole-file gate,
    // while neither the HUFF dictionary nor the compressed body is large.
    let mut file = fs::File::create(&source).expect("create HUFF fixture");
    file.write_all(&bytes).expect("write HUFF fixture");
    file.set_len(5 * 1024 * 1024).expect("pad non-text record");
    drop(file);
    fs::copy(&source, &alternate).expect("copy same Kindle bytes");

    let library_path = root.0.join("library");
    let library = LocalLibrary::open(&library_path).expect("open library");
    let staged = library
        .stage_with_title_hint(&source, None)
        .expect("stage HUFF");
    assert!(!staged.prepared);
    let opened = library.open_book(&staged.id).expect("open HUFF");
    assert!(opened.book.prepared);
    let section = opened
        .root
        .read("/.atha-kindle/section-0001.xhtml")
        .unwrap();
    let html = String::from_utf8(section.bytes).unwrap();
    assert!(html.contains("Bounded HUFF gate"));
    assert!(html.contains(r#"src="images/image-0001.png""#));
    assert_eq!(
        opened
            .root
            .read("/.atha-kindle/images/image-0001.png")
            .unwrap()
            .bytes,
        png
    );
    let manifest = opened.root.read("/.atha-reader.json").unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest.bytes).unwrap();
    assert_eq!(manifest["sections"].as_array().unwrap().len(), 2);
    let next = opened
        .root
        .read("/.atha-kindle/section-0002.xhtml")
        .unwrap();
    let next = String::from_utf8(next.bytes).unwrap();
    assert!(next.contains("<title>Gate</title>"));
    assert!(next.contains("<h2>Next chapter</h2>"));
    assert_eq!(manifest["resources"].as_array().unwrap().len(), 2_001);
    assert_eq!(
        opened
            .root
            .read("/.atha-kindle/images/image-2001.png")
            .unwrap()
            .bytes,
        png
    );
    assert_eq!(library.import(&alternate).unwrap().id, staged.id);
    assert_eq!(library.list().unwrap().len(), 1);

    let imported = import_kindle(&source, library_path.join("ImportedBooks")).unwrap();
    let cache_marker = fs::read_to_string(imported.root.join(".atha-kindle-import")).unwrap();
    assert_eq!(
        cache_marker,
        format!("atha-kindle-import-v1\n{}\n", staged.id)
    );
    assert!(!imported.root.join(".source.kindle").exists());
    assert!(!imported.root.join(".source.unpacked.kindle").exists());
}

#[test]
fn huff_ncx_fallback_keeps_chapters_and_fragment_on_both_admission_paths() {
    let root = TestRoot::new();
    let text = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Guide</title></head><body><h1>First chapter</h1><h2>Indexed chapter</h2></body></html>"#;
    let position = text.windows(4).position(|bytes| bytes == b"<h2>").unwrap() as u32;
    let (huff, cdic, first, second) = huff_records(text);
    let (indx, entries) = single_ncx_position(position);
    let mut record_zero = full_record_zero(0x4448, text);
    record_zero.resize(0xf8, 0);
    record_zero[8..10].copy_from_slice(&2_u16.to_be_bytes());
    record_zero[20..24].copy_from_slice(&0xe8_u32.to_be_bytes());
    record_zero[0x68..0x6c].copy_from_slice(&6_u32.to_be_bytes());
    record_zero[0x6c..0x70].copy_from_slice(&u32::MAX.to_be_bytes());
    record_zero[0x70..0x74].copy_from_slice(&3_u32.to_be_bytes());
    record_zero[0x74..0x78].copy_from_slice(&2_u32.to_be_bytes());
    record_zero[0xf4..0xf8].copy_from_slice(&5_u32.to_be_bytes());
    let bytes = palm_records(&[
        &record_zero,
        &first,
        &second,
        &huff,
        &cdic,
        &indx,
        &entries,
        b"EOF",
    ]);

    for (name, extended) in [("legacy.mobi", false), ("extended.mobi", true)] {
        let source = root.0.join(name);
        let mut file = fs::File::create(&source).unwrap();
        file.write_all(&bytes).unwrap();
        if extended {
            file.set_len(5 * 1024 * 1024).unwrap();
        }
        drop(file);
        let book = import_kindle(source, root.0.join("cache")).unwrap();
        let doc = fs::read_to_string(book.root.join(".atha-kindle/section-0002.xhtml")).unwrap();
        assert!(doc.contains("<title>Guide</title>"));
        assert!(doc.contains("<h2>Indexed chapter</h2>"));
        assert!(doc.contains(&format!(r#"id="filepos{position}""#)));
        let manifest = fs::read(book.root.join(".atha-reader.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(manifest["sections"].as_array().unwrap().len(), 2);
        assert_eq!(
            manifest["toc"][0]["href"],
            format!(".atha-kindle/section-0002.xhtml#filepos{position}")
        );
    }
}

fn single_ncx_position(position: u32) -> (Vec<u8>, Vec<u8>) {
    let tags = b"TAGX\0\0\0\x14\0\0\0\x01\x01\x01\x01\0\0\0\0\x01";
    let mut entry = b"\x06Second\x01".to_vec(); // label and one TAGX control byte
    put_index_varint(position, &mut entry);
    single_index_entry(tags, &entry)
}

#[test]
fn extended_kf8_huff_rewrites_a_normal_position_fragment() {
    let root = TestRoot::new();
    let text = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Gate</title></head><body><h1 id="start">Chapter</h1><p><a href="kindle:pos:fid:0000:off:0000">Back to chapter</a></p></body></html>"#;
    let (huff, cdic, first, second) = huff_records(text);
    let tags = b"TAGX\0\0\0\x14\0\0\0\x01\x06\x02\x01\0\0\0\0\x01";
    let mut entry = b"\x03doc\x01".to_vec();
    put_index_varint(0, &mut entry); // SKEL's start and length
    put_index_varint(text.len() as u32, &mut entry);
    let (skel, entries) = single_index_entry(tags, &entry);
    let mut record_zero = full_record_zero(0x4448, text);
    record_zero.resize(0x108, 0);
    record_zero[8..10].copy_from_slice(&2_u16.to_be_bytes());
    record_zero[20..24].copy_from_slice(&0xf8_u32.to_be_bytes());
    record_zero[36..40].copy_from_slice(&8_u32.to_be_bytes());
    record_zero[0x68..0x6c].copy_from_slice(&8_u32.to_be_bytes());
    record_zero[0x6c..0x70].copy_from_slice(&u32::MAX.to_be_bytes());
    record_zero[0x70..0x74].copy_from_slice(&3_u32.to_be_bytes());
    record_zero[0x74..0x78].copy_from_slice(&2_u32.to_be_bytes());
    record_zero[0xc0..0xc4].copy_from_slice(&u32::MAX.to_be_bytes()); // FDST
    record_zero[0xf4..0xf8].copy_from_slice(&u32::MAX.to_be_bytes()); // NCX
    record_zero[0xf8..0xfc].copy_from_slice(&u32::MAX.to_be_bytes()); // DIV
    record_zero[0xfc..0x100].copy_from_slice(&5_u32.to_be_bytes()); // SKEL
    let bytes = palm_records(&[
        &record_zero,
        &first,
        &second,
        &huff,
        &cdic,
        &skel,
        &entries,
        b"EOF",
    ]);
    let source = root.0.join("position.azw3");
    let mut file = fs::File::create(&source).unwrap();
    file.write_all(&bytes).unwrap();
    file.set_len(5 * 1024 * 1024).unwrap();
    drop(file);

    let book = import_kindle(source, root.0.join("cache")).unwrap();
    let doc = fs::read_to_string(book.root.join(".atha-kindle/section-0001.xhtml")).unwrap();
    assert!(doc.contains(r#"id="start""#));
    assert!(doc.contains(r##"href=".atha-kindle/section-0001.xhtml#start""##));
}

fn single_index_entry(tags: &[u8], entry: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut index = vec![0; 192];
    index[..4].copy_from_slice(b"INDX");
    index[4..8].copy_from_slice(&192_u32.to_be_bytes());
    index[24..28].copy_from_slice(&1_u32.to_be_bytes()); // one data record
    index[180..184].copy_from_slice(&192_u32.to_be_bytes());
    let mut entries = index.clone();
    index.extend_from_slice(tags);
    entries.extend_from_slice(entry);
    let idxt = entries.len() as u32;
    entries[20..24].copy_from_slice(&idxt.to_be_bytes());
    entries.extend_from_slice(b"IDXT\0\xc0"); // the row starts after the 192-byte header
    (index, entries)
}

fn put_index_varint(value: u32, entry: &mut Vec<u8>) {
    assert!(value < 16_384);
    if value >= 128 {
        entry.push((value >> 7) as u8);
    }
    entry.push(0x80 | (value as u8 & 0x7f));
}

fn small_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[128])
        .unwrap();
    bytes
}

/// A normal fixed-width HUFF codebook. Each code represents four source bytes.
fn huff_records(text: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut huff = vec![0; 24];
    huff[..8].copy_from_slice(b"HUFF\0\0\0\x18");
    huff[8..12].copy_from_slice(&24_u32.to_be_bytes());
    huff[12..16].copy_from_slice(&1048_u32.to_be_bytes());
    for _ in 0..256 {
        huff.extend_from_slice(&0xff88_u32.to_be_bytes());
    }
    huff.extend_from_slice(&[0; 32 * 8]);

    let count = text.len().div_ceil(4);
    assert!(count <= 256);
    let mut cdic = vec![0; 16 + count * 2];
    cdic[..8].copy_from_slice(b"CDIC\0\0\0\x10");
    cdic[8..12].copy_from_slice(&(count as u32).to_be_bytes());
    cdic[12..16].copy_from_slice(&8_u32.to_be_bytes());
    let mut codes = Vec::with_capacity(count);
    for (index, phrase) in text.chunks(4).enumerate() {
        let offset = (cdic.len() - 16) as u16;
        cdic[16 + index * 2..18 + index * 2].copy_from_slice(&offset.to_be_bytes());
        cdic.extend_from_slice(&(0x8000 | phrase.len() as u16).to_be_bytes());
        cdic.extend_from_slice(phrase);
        codes.push(255 - index as u8);
    }
    let second = codes.split_off(count / 2);
    (huff, cdic, codes, second)
}

#[test]
fn kindle_can_be_staged_before_its_first_open() {
    let root = TestRoot::new();
    let source = root.0.join("staged.azw3");
    fs::write(
        &source,
        palm_database(minimal_record_zero(2, SAFE_XHTML), SAFE_XHTML),
    )
    .expect("write Kindle fixture");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let staged = library
        .stage_with_title_hint(&source, None)
        .expect("stage Kindle");
    assert!(!staged.prepared);
    assert!(
        library
            .open_book(&staged.id)
            .expect("prepare Kindle")
            .book
            .prepared
    );
}

#[test]
fn imports_palmdoc_through_the_shared_library() {
    let root = TestRoot::new();
    let source = root.0.join("gate.mobi");
    fs::write(
        &source,
        palm_database(minimal_record_zero(2, SAFE_XHTML), SAFE_XHTML),
    )
    .expect("write PalmDOC fixture");
    let library = LocalLibrary::open(root.0.join("library")).expect("open library");

    let imported = library.import(&source).expect("import PalmDOC");
    assert_eq!(imported.title, "Gate");
    let opened = library.open_book(&imported.id).expect("open PalmDOC");
    let manifest: serde_json::Value = serde_json::from_slice(
        &opened
            .root
            .read("/.atha-reader.json")
            .expect("read manifest")
            .bytes,
    )
    .expect("parse manifest");
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(1));
    assert_eq!(manifest["toc"].as_array().map(Vec::len), Some(1));
    let section = opened
        .root
        .read("/.atha-kindle/section-0001.xhtml")
        .expect("read Kindle section");
    let section = String::from_utf8(section.bytes).expect("UTF-8 XHTML");
    assert!(section.contains("PalmDOC gate"));
    assert!(section.contains("href=\"#end\""));
    assert!(section.contains("<a>Broken image link</a>"));
}

#[test]
fn content_identity_does_not_depend_on_kindle_suffix() {
    let root = TestRoot::new();
    let bytes = palm_database(minimal_record_zero(2, SAFE_XHTML), SAFE_XHTML);
    let mobi = root.0.join("same.mobi");
    let azw = root.0.join("same.azw");
    let azw3 = root.0.join("same.azw3");
    fs::write(&mobi, &bytes).expect("write MOBI");
    fs::write(&azw, &bytes).expect("write AZW");
    fs::write(&azw3, &bytes).expect("write AZW3");

    let cache = root.0.join("cache");
    let first = import_kindle(&mobi, &cache).expect("import MOBI");
    for path in [&azw, &azw3] {
        let imported = import_kindle(path, &cache).expect("import alternate suffix");
        assert_eq!(imported.content_version, first.content_version);
        assert_eq!(imported.root, first.root);
    }
    assert!(
        fs::read_dir(&cache)
            .expect("read Kindle cache")
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".kindle.staging-")),
        "cache hits must remove their source snapshots"
    );
}

#[test]
fn rejects_dictionary_drm_unknown_values_and_active_markup() {
    let root = TestRoot::new();
    let active = SAFE_XHTML
        .windows(b"</body>".len())
        .position(|window| window == b"</body>")
        .map(|position| {
            let mut bytes = SAFE_XHTML.to_vec();
            bytes.splice(
                position..position,
                b"<script>bad()</script>".iter().copied(),
            );
            bytes
        })
        .expect("body end");
    let invalid_fragment = String::from_utf8(SAFE_XHTML.to_vec())
        .expect("fixture UTF-8")
        .replace("#end", "#bad?")
        .into_bytes();
    let active_style = String::from_utf8(SAFE_XHTML.to_vec())
        .expect("fixture UTF-8")
        .replace(
            "</head>",
            "<style>@import url(https://example.invalid/a.css)</style></head>",
        )
        .into_bytes();
    let mut dictionary = full_record_zero(1, SAFE_XHTML);
    dictionary[40..44].copy_from_slice(&1_u32.to_be_bytes());
    let mut drm = minimal_record_zero(1, SAFE_XHTML);
    drm[12..14].copy_from_slice(&1_u16.to_be_bytes());
    let unknown_compression = minimal_record_zero(7, SAFE_XHTML);
    let missing_huff_records = full_record_zero(0x4448, SAFE_XHTML);
    let mut unknown_encoding = full_record_zero(1, SAFE_XHTML);
    unknown_encoding[28..32].copy_from_slice(&932_u32.to_be_bytes());
    let mut oversized = minimal_record_zero(1, SAFE_XHTML);
    oversized[4..8].copy_from_slice(&(128_u32 * 1024 * 1024 + 1).to_be_bytes());

    for (name, record_zero, text, code) in [
        (
            "dictionary.mobi",
            dictionary,
            SAFE_XHTML.to_vec(),
            "kindle-dictionary-unsupported",
        ),
        ("drm.mobi", drm, SAFE_XHTML.to_vec(), "encrypted-kindle"),
        (
            "compression.mobi",
            unknown_compression,
            SAFE_XHTML.to_vec(),
            "unsupported-kindle",
        ),
        (
            "missing-huff.mobi",
            missing_huff_records,
            SAFE_XHTML.to_vec(),
            "invalid-kindle-structure",
        ),
        (
            "encoding.mobi",
            unknown_encoding,
            SAFE_XHTML.to_vec(),
            "invalid-kindle-encoding",
        ),
        (
            "oversized.mobi",
            oversized,
            SAFE_XHTML.to_vec(),
            "kindle-text-too-large",
        ),
        (
            "active.mobi",
            minimal_record_zero(1, &active),
            active,
            "invalid-kindle-markup",
        ),
        (
            "fragment.mobi",
            minimal_record_zero(1, &invalid_fragment),
            invalid_fragment,
            "invalid-kindle-reference",
        ),
        (
            "style.mobi",
            minimal_record_zero(1, &active_style),
            active_style,
            "invalid-kindle-markup",
        ),
    ] {
        let path = root.0.join(name);
        fs::write(&path, palm_database(record_zero, &text)).expect("write rejection fixture");
        let cache = root.0.join(format!("cache-{name}"));
        assert_eq!(
            import_kindle(&path, &cache)
                .expect_err("reject unsafe Kindle source")
                .code(),
            code,
            "{name}"
        );
        assert!(
            !cache.exists()
                || fs::read_dir(cache)
                    .expect("read cleaned rejection cache")
                    .next()
                    .is_none(),
            "{name}"
        );
    }
}

#[test]
fn rejects_invalid_pdb_record_ranges() {
    let root = TestRoot::new();
    let path = root.0.join("ranges.mobi");
    let mut bytes = palm_database(minimal_record_zero(1, SAFE_XHTML), SAFE_XHTML);
    let first = u32::from_be_bytes(bytes[78..82].try_into().expect("first offset"));
    bytes[86..90].copy_from_slice(&first.to_be_bytes());
    fs::write(&path, bytes).expect("write invalid PDB");

    assert_eq!(
        import_kindle(&path, root.0.join("cache"))
            .expect_err("reject descending record range")
            .code(),
        "invalid-kindle-structure"
    );

    let oversized = root.0.join("oversized.azw3");
    fs::File::create(&oversized)
        .and_then(|file| file.set_len(256 * 1024 * 1024 + 1))
        .expect("write sparse oversized Kindle source");
    assert_eq!(
        import_kindle(oversized, root.0.join("oversized-cache"))
            .expect_err("reject oversized Kindle source before reading")
            .code(),
        "kindle-source-too-large"
    );
}

#[test]
#[ignore = "uses private local Kindle samples without copying or logging their content"]
fn imports_private_kf8_samples() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ordinary = private_kindle_samples(&repository);
    assert_eq!(
        ordinary.len(),
        2,
        "expected two private ordinary Kindle samples"
    );
    let work = TestRoot::new();
    let cache = work.0.join("imported");
    let mut shapes = Vec::new();
    for path in &ordinary {
        let imported = import_kindle(path, &cache).expect("import private ordinary Kindle sample");
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(imported.root.join(".atha-reader.json")).expect("read private manifest"),
        )
        .expect("parse private manifest");
        assert!(
            manifest["sections"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        let toc = manifest["toc"].as_array().expect("private TOC");
        let unique_toc = toc
            .iter()
            .map(|item| item["href"].as_str().expect("private TOC href"))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            unique_toc.len(),
            toc.len(),
            "private TOC targets must be unique"
        );
        shapes.push((
            path,
            manifest["sections"].as_array().map_or(0, Vec::len),
            toc.len(),
            manifest["resources"].as_array().map_or(0, Vec::len),
        ));
    }
    let mut anonymous_shapes = shapes
        .iter()
        .map(|(_, sections, toc, resources)| (*sections, *toc, *resources))
        .collect::<Vec<_>>();
    anonymous_shapes.sort_unstable();
    assert_eq!(anonymous_shapes, [(1, 1, 405), (25, 204, 96)]);
    if let Some(root) = std::env::var_os("ATHA_KINDLE_GATE_LIBRARY_ROOT") {
        let root = PathBuf::from(root);
        assert_gate_root(&repository, &root);
        fs::create_dir(&root).expect("create a new Kindle gate library");
        let selected = shapes
            .into_iter()
            .max_by_key(|(_, sections, toc, _)| (*toc, *sections))
            .expect("select private Kindle GUI sample")
            .0;
        let library = LocalLibrary::open(root).expect("open Kindle gate library");
        let book = library.import(selected).expect("seed Kindle gate library");
        library
            .open_book(&book.id)
            .expect("open seeded Kindle gate book through BookRoot");
    }
}

#[test]
#[ignore = "uses a private local Kindle dictionary without copying or logging its content"]
fn rejects_private_dictionary_before_expansion() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = TestRoot::new();
    let cache = work.0.join("imported");
    let dictionary = repository
        .join("fixtures/local")
        .read_dir()
        .expect("read private fixture directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("mobi"))
        .expect("find private Kindle dictionary");
    assert_eq!(
        import_kindle(dictionary, &cache)
            .expect_err("reject dictionary before expansion")
            .code(),
        "kindle-dictionary-unsupported"
    );
    assert!(
        !cache.exists(),
        "dictionary rejection must precede cache creation"
    );
}

fn private_kindle_samples(repository: &std::path::Path) -> Vec<PathBuf> {
    let mut samples = repository
        .join("fixtures/local")
        .read_dir()
        .expect("read private fixture directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    matches!(extension.to_ascii_lowercase().as_str(), "azw" | "azw3")
                })
        })
        .collect::<Vec<_>>();
    samples.sort();
    samples
}

fn assert_gate_root(repository: &std::path::Path, root: &std::path::Path) {
    let temporary = fs::canonicalize(repository.join(".tmp")).expect("resolve .tmp root");
    let resolved = fs::canonicalize(root).unwrap_or_else(|_| {
        fs::canonicalize(root.parent().expect("Kindle gate library parent"))
            .expect("resolve Kindle gate library parent")
            .join(root.file_name().expect("Kindle gate library name"))
    });
    assert!(
        root.is_absolute() && resolved.starts_with(temporary),
        "Kindle gate library must stay inside the repository .tmp directory"
    );
}

fn minimal_record_zero(compression: u16, text: &[u8]) -> Vec<u8> {
    let mut record = vec![0_u8; 16];
    record[0..2].copy_from_slice(&compression.to_be_bytes());
    record[4..8].copy_from_slice(&(text.len() as u32).to_be_bytes());
    record[8..10].copy_from_slice(&1_u16.to_be_bytes());
    record[10..12].copy_from_slice(&4096_u16.to_be_bytes());
    record
}

fn full_record_zero(compression: u16, text: &[u8]) -> Vec<u8> {
    let mut record = vec![0_u8; 44];
    record[..16].copy_from_slice(&minimal_record_zero(compression, text));
    record[16..20].copy_from_slice(b"MOBI");
    record[20..24].copy_from_slice(&28_u32.to_be_bytes());
    record[28..32].copy_from_slice(&65001_u32.to_be_bytes());
    record[36..40].copy_from_slice(&6_u32.to_be_bytes());
    record[40..44].copy_from_slice(&u32::MAX.to_be_bytes());
    record
}

fn palm_database(record_zero: Vec<u8>, text: &[u8]) -> Vec<u8> {
    palm_records(&[&record_zero, text])
}

fn palm_records(records: &[&[u8]]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 78 + records.len() * 8];
    bytes[..4].copy_from_slice(b"Gate");
    bytes[60..68].copy_from_slice(b"TEXtREAd");
    bytes[76..78].copy_from_slice(&(records.len() as u16).to_be_bytes());
    for (index, record) in records.iter().enumerate() {
        let offset = bytes.len() as u32;
        let entry = 78 + index * 8;
        bytes[entry..entry + 4].copy_from_slice(&offset.to_be_bytes());
        bytes.extend_from_slice(record);
    }
    bytes
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let temporary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
        fs::create_dir_all(&temporary).expect("create repository temporary root");
        let path = temporary.join(format!(
            "atha-kindle-test-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test root");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_dir_all(&self.0).expect("remove test root");
        }
    }
}
