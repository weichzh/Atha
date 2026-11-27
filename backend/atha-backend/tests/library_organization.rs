use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::{
    local_data::{BrowserState, LocalData},
    messages::MessageStore,
    reader::{
        dictionary::LocalDictionaries,
        library::{BookOrganization, LibraryBook, LibraryError, LocalLibrary},
    },
};
use serde::Deserialize;
use serde_json::json;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];
const FB2: &str = r#"<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
  <description><title-info>
    <genre match="80">sf</genre><genre>sf</genre><genre>adventure</genre>
    <book-title>FictionBook gate</book-title><sequence name="Серия &amp; Том" number="3"/>
  </title-info><src-title-info><genre>do not include</genre></src-title-info></description>
  <body><section><p>Read this section.</p></section></body>
</FictionBook>"#;

#[test]
fn kindle_uses_existing_exth_subjects_and_plain_text_stays_unclassified() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let mobi = root.0.join("topics.mobi");
    fs::write(&mobi, mobi_with_subject("Poetry; Philosophy; Poetry")).unwrap();
    let metadata = prepare(&library, &mobi);
    assert_eq!(metadata.subjects, ["Poetry", "Philosophy"]);
    assert!(metadata.tags.is_empty() && metadata.series.is_empty() && !metadata.favorite);

    for (filename, input) in [
        ("Poetry.txt", "Poetry\n\nA title does not assert a genre."),
        (
            "Poetry.md",
            "# Poetry\n\nSubject: not embedded catalog metadata.",
        ),
    ] {
        let path = root.0.join(filename);
        fs::write(&path, input).unwrap();
        let metadata = prepare(&library, &path);
        assert!(
            metadata.subjects.is_empty() && metadata.tags.is_empty() && metadata.series.is_empty()
        );
        assert!(!metadata.favorite);
    }
}

#[test]
fn epub_projects_classification_without_guessing_or_breaking_optional_metadata() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let file = root.0.join("categorized.epub");
    epub(
        &file,
        "3.0",
        &format!(
            r##"
        <dc:subject>War &amp; Peace</dc:subject><dc:subject> War   &amp; Peace </dc:subject>
        <dc:subject xmlns:dc="urn:foreign">forged class</dc:subject>
        <dc:subject><x>structured markup</x></dc:subject>
        <dc:subject>{}</dc:subject><dc:subject>历史</dc:subject>
        <meta refines="#s1" property="collection-type">series</meta>
        <meta id="s1" property="belongs-to-collection">Chronicles</meta>
        <meta id="s2" property="belongs-to-collection">Box Set</meta>
        <meta refines="#s2" property="collection-type">set</meta>
        <meta id="s3" property="belongs-to-collection">Untyped</meta>
        <meta id="s4" property="belongs-to-collection">Conflicting</meta>
        <meta refines="#s4" property="collection-type">series</meta>
        <meta refines="#s4" property="collection-type">set</meta>
        <meta name="calibre:series" content="Fallback"/>
        "##,
            "过".repeat(200)
        ),
    );
    let book = prepare(&library, &file);
    assert_eq!(book.subjects, ["War & Peace", "历史"]);
    assert_eq!(book.series, ["Chronicles"]);
    assert!(book.tags.is_empty());
    assert!(!book.favorite);

    let file = root.0.join("classic.epub");
    let items = (0..35)
        .map(|index| format!("<dc:subject>Subject {index}</dc:subject>"))
        .collect::<String>();
    epub(
        &file,
        "2.0",
        &format!(r#"{items}<meta name="calibre:series" content="Older Series"/>"#),
    );
    let classic = prepare(&library, &file);
    assert_eq!(classic.series, ["Older Series"]);
    assert_eq!(classic.subjects.len(), 32);
    assert_eq!(classic.subjects[31], "Subject 31");
}

#[test]
fn existing_epub_cbz_and_fb2_caches_gain_classification_on_open_without_render_rebuild() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let epub_path = root.0.join("earlier.epub");
    epub(&epub_path, "3.0", "<dc:subject>Poetry</dc:subject>");
    let cbz_path = root.0.join("comics.cbz");
    zip(
        &cbz_path,
        &[
            (
                "ComicInfo.xml",
                br#"<ComicInfo><Title>Comics gate</Title><Series>Cloud, Rain</Series>
                  <Genre>Science &amp; Fiction, Historical</Genre><Tags>Ink, Ink, Color</Tags>
                  <Pages><Page Image="0" Type="FrontCover"/></Pages></ComicInfo>"#,
            ),
            ("001.png", PNG_1X1),
        ],
    );
    let fb2_path = root.0.join("story.fb2");
    fs::write(&fb2_path, FB2).unwrap();
    let fbz_path = root.0.join("story.fbz");
    zip(&fbz_path, &[("story.fb2", FB2.as_bytes())]);

    for (path, subjects, tags, series) in [
        (&epub_path, &["Poetry"][..], &[][..], &[][..]),
        (
            &cbz_path,
            &["Science & Fiction", "Historical"],
            &["Ink", "Color"],
            &["Cloud, Rain"],
        ),
        (&fb2_path, &["sf", "adventure"], &[], &["Серия & Том"]),
        (&fbz_path, &["sf", "adventure"], &[], &["Серия & Том"]),
    ] {
        let initial = prepare(&library, path);
        assert_eq!(initial.subjects, subjects);
        assert_eq!(initial.tags, tags);
        assert_eq!(initial.series, series);
        let cache = root.0.join("ImportedBooks").join(&initial.id);
        fs::remove_file(source_classification_path(&root.0, &initial.id)).unwrap();
        fs::write(cache.join("untouched-cache-file"), b"body cache retained").unwrap();

        let old = listed(&library, &initial.id);
        assert!(old.prepared);
        assert!(old.subjects.is_empty());
        let enriched = library.open_book(&initial.id).unwrap().book;
        assert_eq!(enriched.subjects, subjects);
        assert_eq!(enriched.tags, tags);
        assert_eq!(enriched.series, series);
        assert_eq!(enriched, listed(&library, &initial.id));
        assert_eq!(
            fs::read(cache.join("untouched-cache-file")).unwrap(),
            b"body cache retained"
        );
    }
}

#[test]
fn old_cache_still_opens_when_durable_source_identity_no_longer_matches() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let path = root.0.join("original.epub");
    epub(&path, "3.0", "<dc:subject>Original</dc:subject>");
    let book = prepare(&library, &path);
    fs::remove_file(source_classification_path(&root.0, &book.id)).unwrap();

    let durable = root.0.join("SourceBooks").join(format!("{}.epub", book.id));
    epub(&durable, "3.0", "<dc:subject>Unrelated</dc:subject>");
    let safe = library.open_book(&book.id).unwrap().book;
    assert!(safe.prepared);
    assert!(safe.subjects.is_empty());
    fs::copy(path, durable).unwrap();
    assert_eq!(
        library.open_book(&book.id).unwrap().book.subjects,
        ["Original"]
    );
}

#[test]
fn bad_optional_classification_cannot_disable_legacy_cache_only_reader_or_cover() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let source = root.0.join("legacy.cbz");
    zip(
        &source,
        &[
            ("001.png", PNG_1X1),
            (
                "ComicInfo.xml",
                b"<ComicInfo><Title>Legacy Comics</Title><Genre>Poetry</Genre></ComicInfo>",
            ),
        ],
    );
    let book = library.import(&source).unwrap();
    assert_eq!(book.subjects, ["Poetry"]);
    assert!(
        fs::read_dir(root.0.join("SourceBooks"))
            .unwrap()
            .next()
            .is_none()
    );
    fs::remove_file(source).unwrap();
    let cache = root.0.join("ImportedBooks").join(&book.id);
    let old_metadata_bytes = fs::read(cache.join(".atha-book.json")).unwrap();
    let old: LegacyBookMetadata = serde_json::from_slice(&old_metadata_bytes).unwrap();
    assert_eq!(old.schema, 1);
    assert_eq!(old.content_version, book.id);
    assert_eq!(old.title.as_deref(), Some("Legacy Comics"));
    assert!(old.authors.is_empty());
    assert!(old.cover_path.is_some());

    fs::write(
        source_classification_path(&root.0, &book.id),
        serde_json::to_vec(&json!({
            "schema": 1, "contentVersion": book.id, "classification": {"subjects": [" Poetry"]}
        }))
        .unwrap(),
    )
    .unwrap();
    let library = LocalLibrary::open(&root.0).unwrap();
    let unclassified = listed(&library, &book.id);
    assert!(unclassified.prepared && unclassified.has_cover);
    assert!(unclassified.subjects.is_empty());
    assert_eq!(library.cover(&book.id).unwrap().bytes, PNG_1X1);
    let opened = library.open_book(&book.id).unwrap();
    assert_eq!(opened.book, unclassified);
    assert!(opened.root.read("/.atha-reader.json").is_ok());
    assert_eq!(
        fs::read(cache.join(".atha-book.json")).unwrap(),
        old_metadata_bytes
    );
}

#[test]
fn legacy_records_are_editable_and_removal_does_not_inherit_user_classification() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let path = root.0.join("story.epub");
    epub(&path, "3.0", "<dc:subject>Source subject</dc:subject>");
    let staged = library
        .stage_with_title_hint(&path, Some("Held title"))
        .unwrap();
    assert!(!staged.prepared);
    let record = root.0.join("Library").join(format!("{}.json", staged.id));
    let old_bytes = fs::read(&record).unwrap();
    let old: LegacyStoredBook = serde_json::from_slice(&old_bytes).unwrap();
    assert_eq!(old.schema, 1);
    assert_eq!(old.id, staged.id);
    assert_eq!(old.title, staged.title);
    assert_eq!(old.imported_at, staged.imported_at);
    assert!(old.authors.is_empty() && old.cover_path.is_none() && old.source_path.is_some());
    let organization_path = root
        .0
        .join("BookOrganization")
        .join(format!("{}.json", staged.id));
    let saved = library
        .update_book_organization(
            &staged.id,
            BookOrganization {
                favorite: Some(true),
                tags: Some(vec![
                    "  Local\t favorites ".into(),
                    "Local favorites".into(),
                    " ".into(),
                ]),
                subjects: Some(vec![]),
                series: Some(vec!["我选的系列".into()]),
            },
        )
        .unwrap();
    assert_eq!(saved.tags, ["Local favorites"]);
    assert_eq!(saved.series, ["我选的系列"]);
    assert!(saved.favorite);
    assert!(saved.subjects.is_empty());
    assert_eq!(fs::read(&record).unwrap(), old_bytes);
    assert!(organization_path.is_file());

    for organization in [
        BookOrganization {
            tags: Some(vec!["x".into(); 33]),
            ..Default::default()
        },
        BookOrganization {
            subjects: Some(vec!["重".repeat(129)]),
            ..Default::default()
        },
        BookOrganization {
            series: Some(vec!["a\0b".into()]),
            ..Default::default()
        },
    ] {
        assert_eq!(
            library.update_book_organization(&staged.id, organization),
            Err(LibraryError::InvalidOrganization)
        );
        assert_eq!(listed(&library, &staged.id), saved);
    }
    assert_eq!(
        library.update_book_organization("../invalid", BookOrganization::default()),
        Err(LibraryError::InvalidBookId)
    );
    assert_eq!(
        library.update_book_organization(&"b".repeat(64), BookOrganization::default()),
        Err(LibraryError::UnknownBook)
    );

    let library = LocalLibrary::open(&root.0).unwrap();
    let opened = library.open_book(&staged.id).unwrap().book;
    assert!(opened.prepared && opened.favorite);
    assert_eq!(opened.title, "Classification gate");
    assert_eq!(opened.imported_at, staged.imported_at);
    assert!(
        opened.subjects.is_empty(),
        "user cleared the source subject"
    );
    assert_eq!(prepare(&library, &path), opened);
    library.remove(&staged.id).unwrap();
    assert!(!organization_path.exists());
    assert!(source_classification_path(&root.0, &staged.id).is_file());
    let readded = prepare(&library, &path);
    assert_eq!(readded.id, staged.id);
    assert_eq!(readded.subjects, ["Source subject"]);
    assert!(!readded.favorite && readded.tags.is_empty() && readded.series.is_empty());
    library
        .update_book_organization(
            &staged.id,
            BookOrganization {
                favorite: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    library.prepare_local_data_deletion(&staged.id).unwrap();
    library.finish_local_data_deletion(&staged.id).unwrap();
    assert!(!record.exists());
    assert!(!organization_path.exists());
    assert!(!source_classification_path(&root.0, &staged.id).exists());
    assert!(library.list().unwrap().is_empty());
    assert_eq!(prepare(&library, &path).subjects, ["Source subject"]);
}

#[test]
fn shelf_projection_observes_changes_made_via_another_handle() {
    let root = TestRoot::new();
    let reader = LocalLibrary::open(&root.0).unwrap();
    let writer = LocalLibrary::open(&root.0).unwrap();
    assert!(reader.list().unwrap().is_empty());

    let source = root.0.join("fresh.fb2");
    fs::write(&source, FB2).unwrap();
    let staged = writer
        .stage_with_title_hint(&source, Some("Staged title"))
        .unwrap();
    assert_eq!(reader.list().unwrap(), std::slice::from_ref(&staged));
    let prepared = writer.open_book(&staged.id).unwrap().book;
    assert!(prepared.prepared);
    assert_eq!(prepared.subjects, ["sf", "adventure"]);
    assert_eq!(reader.list().unwrap(), std::slice::from_ref(&prepared));

    let cover = root.0.join("cover.png");
    fs::write(&cover, PNG_1X1).unwrap();
    let organized = writer
        .update_book_organization(
            &staged.id,
            BookOrganization {
                tags: Some(vec!["Local tag".into()]),
                favorite: Some(true),
                ..BookOrganization::default()
            },
        )
        .unwrap();
    assert_eq!(reader.list().unwrap(), std::slice::from_ref(&organized));
    let with_cover = writer.set_custom_cover(&staged.id, &cover).unwrap();
    assert!(with_cover.has_cover && with_cover.has_custom_cover);
    assert_eq!(reader.list().unwrap(), [with_cover]);
    assert_eq!(writer.reset_custom_cover(&staged.id).unwrap(), organized);
    assert_eq!(reader.list().unwrap(), [organized]);

    writer.remove(&staged.id).unwrap();
    assert!(reader.list().unwrap().is_empty());
    assert_eq!(
        writer.stage_with_title_hint(&source, None).unwrap().id,
        staged.id
    );
    assert_eq!(reader.list().unwrap().len(), 1);
    let current = listed(&reader, &staged.id);
    assert!(current.prepared && !current.favorite && !current.has_custom_cover);
    assert!(current.tags.is_empty());
}

#[test]
fn prepared_cache_marker_order_does_not_make_the_source_path_authoritative() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let source = root.0.join("older.epub");
    epub(&source, "3.0", "<dc:subject>Source subject</dc:subject>");
    let book = prepare(&library, &source);
    let cache = root.0.join("ImportedBooks").join(&book.id);
    let marker = cache.join(".atha-epub-import");
    let old_marker = format!("atha-epub-import-v2\n{}\n", book.id);
    fs::write(&marker, &old_marker).unwrap();
    let record = root.0.join("Library").join(format!("{}.json", book.id));
    let mut stored: serde_json::Value =
        serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();

    // The real source disappears but the old prepared cache is still usable.
    fs::remove_file(root.0.join("SourceBooks").join(format!("{}.epub", book.id))).unwrap();
    assert_eq!(listed(&library, &book.id), book);
    assert_eq!(library.open_book(&book.id).unwrap().book, book);

    // A wrong or absent hint is possible in older records; only marker contents count.
    stored["sourcePath"] = json!(format!("{}.txt", book.id));
    fs::write(&record, serde_json::to_vec(&stored).unwrap()).unwrap();
    fs::write(cache.join(".atha-text-import"), b"wrong marker").unwrap();
    assert_eq!(listed(&library, &book.id), book);
    stored.as_object_mut().unwrap().remove("sourcePath");
    fs::write(&record, serde_json::to_vec(&stored).unwrap()).unwrap();
    assert_eq!(listed(&library, &book.id), book);

    fs::write(
        &marker,
        format!("atha-epub-import-v2\n{}\n", "f".repeat(64)),
    )
    .unwrap();
    let invalid_marker = listed(&library, &book.id);
    assert!(!invalid_marker.prepared && invalid_marker.subjects.is_empty());
    assert_eq!(invalid_marker.title, book.title);
    fs::write(&marker, old_marker).unwrap();
    fs::remove_file(cache.join(".atha-reader.json")).unwrap();
    let invalid_manifest = listed(&library, &book.id);
    assert!(!invalid_manifest.prepared);
    assert_eq!(
        library.open_book(&book.id).unwrap_err(),
        LibraryError::CorruptRecord
    );

    fs::write(&record, b"not a valid stored book").unwrap();
    assert_eq!(library.list(), Err(LibraryError::CorruptRecord));
    fs::rename(root.0.join("Library"), root.0.join("Moved Library")).unwrap();
    assert_eq!(library.list(), Err(LibraryError::ReadFailed));
}

#[test]
fn complete_backup_preserves_source_defaults_and_manual_empty_overrides_before_reopen() {
    let source = TestRoot::new();
    let library = LocalLibrary::open(&source.0).unwrap();
    let file = source.0.join("default.epub");
    epub(&file, "2.0", "<dc:subject>History</dc:subject>");
    let initial = prepare(&library, &file);
    fs::remove_file(source_classification_path(&source.0, &initial.id)).unwrap();
    let favorite = library
        .update_book_organization(
            &initial.id,
            BookOrganization {
                favorite: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(favorite.favorite && favorite.subjects.is_empty());
    let enriched = library.open_book(&initial.id).unwrap().book;
    assert!(enriched.favorite);
    assert_eq!(enriched.subjects, ["History"]);
    let file = source.0.join("custom.epub");
    epub(&file, "2.0", "<dc:subject>Unset this</dc:subject>");
    let custom = prepare(&library, &file);
    library
        .update_book_organization(
            &custom.id,
            BookOrganization {
                favorite: Some(true),
                tags: Some(vec!["Reference".into()]),
                subjects: Some(vec![]),
                ..Default::default()
            },
        )
        .unwrap();

    let messages = MessageStore::open(&source.0).unwrap();
    let dictionaries = LocalDictionaries::open(&source.0).unwrap();
    let empty = BrowserState {
        schema: 1,
        records: vec![],
    };
    let archive = source.0.join("copy.atha-data");
    LocalData::open(&source.0)
        .unwrap()
        .create_backup(&archive, &empty, &library, &dictionaries, &messages)
        .unwrap();

    let destination = TestRoot::new();
    let destination_data = LocalData::open(&destination.0).unwrap();
    let destination_messages = MessageStore::open(&destination.0).unwrap();
    LocalLibrary::open(&destination.0).unwrap();
    LocalDictionaries::open(&destination.0).unwrap();
    let restore = destination_data
        .prepare_restore(&archive, &empty, &destination_messages)
        .unwrap();
    destination_data
        .commit_restore(&restore.token, &destination_messages)
        .unwrap();
    destination_data.finish_restore(&restore.token).unwrap();
    let restored = LocalLibrary::open(&destination.0).unwrap();

    let book = listed(&restored, &initial.id);
    assert!(!book.prepared && book.favorite);
    assert_eq!(book.subjects, ["History"]);
    assert_eq!(
        restored.open_book(&initial.id).unwrap().book.subjects,
        book.subjects
    );
    let book = listed(&restored, &custom.id);
    assert!(!book.prepared && book.favorite);
    assert_eq!(book.tags, ["Reference"]);
    assert!(book.subjects.is_empty());
    let reopened = restored.open_book(&custom.id).unwrap().book;
    assert!(reopened.prepared && reopened.favorite && reopened.subjects.is_empty());
    assert_eq!(reopened.tags, ["Reference"]);
}

#[test]
fn restoring_an_old_backup_clears_organization_and_rollback_recovers_it() {
    let root = TestRoot::new();
    let library = LocalLibrary::open(&root.0).unwrap();
    let data = LocalData::open(&root.0).unwrap();
    let dictionaries = LocalDictionaries::open(&root.0).unwrap();
    let messages = MessageStore::open(&root.0).unwrap();
    let source = root.0.join("older.txt");
    fs::write(&source, "Old book without classification.").unwrap();
    let book = prepare(&library, &source);
    let empty = BrowserState {
        schema: 1,
        records: vec![],
    };
    let archive = root.0.join("legacy-layout.atha-data");
    data.create_backup(&archive, &empty, &library, &dictionaries, &messages)
        .unwrap();
    let zip = ZipArchive::new(File::open(&archive).unwrap()).unwrap();
    assert!(
        !zip.file_names()
            .any(|name| name.starts_with("BookOrganization/"))
    );
    drop(zip);
    library
        .update_book_organization(
            &book.id,
            BookOrganization {
                favorite: Some(true),
                subjects: Some(vec!["Local history".into()]),
                ..Default::default()
            },
        )
        .unwrap();
    let saved = listed(&library, &book.id);
    assert_eq!(saved.subjects, ["Local history"]);

    let pending = data.prepare_restore(&archive, &empty, &messages).unwrap();
    data.commit_restore(&pending.token, &messages).unwrap();
    let old = listed(&library, &book.id);
    assert!(!old.favorite && old.subjects.is_empty() && !old.prepared);
    data.rollback_restore(&pending.token, &messages).unwrap();
    data.finish_restore(&pending.token).unwrap();
    assert_eq!(listed(&library, &book.id), saved);
}

// These schema-1 shapes are the strict readers from before classification was introduced.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyStoredBook {
    schema: u8,
    id: String,
    title: String,
    authors: Vec<String>,
    cover_path: Option<String>,
    imported_at: u64,
    source_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyBookMetadata {
    schema: u8,
    content_version: String,
    title: Option<String>,
    authors: Vec<String>,
    cover_path: Option<String>,
}

fn source_classification_path(root: &Path, id: &str) -> PathBuf {
    root.join("ImportedBooks")
        .join(format!(".atha-classification-{id}.json"))
}

fn listed(library: &LocalLibrary, id: &str) -> LibraryBook {
    library
        .list()
        .unwrap()
        .into_iter()
        .find(|book| book.id == id)
        .unwrap()
}

fn prepare(library: &LocalLibrary, path: &Path) -> LibraryBook {
    let staged = library.stage_with_title_hint(path, None).unwrap();
    library.open_book(&staged.id).unwrap().book
}

fn epub(path: &Path, version: &str, metadata: &str) {
    let package = format!(
        r#"<package xmlns="http://www.idpf.org/2007/opf" version="{version}">
        <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
          <dc:title>Classification gate</dc:title>{metadata}
        </metadata>
        <manifest><item id="c1" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest>
        <spine><itemref idref="c1"/></spine>
      </package>"#
    );
    zip(path, &[
        ("META-INF/container.xml", br#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#),
        ("package.opf", package.as_bytes()),
        ("chapter.xhtml", br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Gate</title></head><body><p>Read this section.</p></body></html>"#),
    ]);
}

fn mobi_with_subject(subject: &str) -> Vec<u8> {
    let body =
        br#"<html><head><title>Topics</title></head><body><p>Read this section.</p></body></html>"#;
    let mut record = vec![0; 0x90];
    record[..2].copy_from_slice(&1_u16.to_be_bytes());
    record[4..8].copy_from_slice(&(body.len() as u32).to_be_bytes());
    record[8..10].copy_from_slice(&1_u16.to_be_bytes());
    record[10..12].copy_from_slice(&4096_u16.to_be_bytes());
    record[16..20].copy_from_slice(b"MOBI");
    record[20..24].copy_from_slice(&0x80_u32.to_be_bytes());
    record[28..32].copy_from_slice(&65001_u32.to_be_bytes());
    record[36..40].copy_from_slice(&6_u32.to_be_bytes());
    record[40..44].copy_from_slice(&u32::MAX.to_be_bytes());
    record[0x68..0x6c].copy_from_slice(&6_u32.to_be_bytes());
    record[0x6c..0x70].copy_from_slice(&u32::MAX.to_be_bytes());
    record[0x80..0x84].copy_from_slice(&0x40_u32.to_be_bytes());
    record.extend_from_slice(b"EXTH");
    record.extend_from_slice(&(20 + subject.len() as u32).to_be_bytes());
    record.extend_from_slice(&1_u32.to_be_bytes());
    record.extend_from_slice(&105_u32.to_be_bytes());
    record.extend_from_slice(&(8 + subject.len() as u32).to_be_bytes());
    record.extend_from_slice(subject.as_bytes());

    let mut database = vec![0; 94];
    database[..6].copy_from_slice(b"Topics");
    database[60..68].copy_from_slice(b"BOOKMOBI");
    database[76..78].copy_from_slice(&2_u16.to_be_bytes());
    database[78..82].copy_from_slice(&94_u32.to_be_bytes());
    database[86..90].copy_from_slice(&(94 + record.len() as u32).to_be_bytes());
    database.extend(record);
    database.extend_from_slice(body);
    database
}

fn zip(path: &Path, files: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in files {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "atha-classification-{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
