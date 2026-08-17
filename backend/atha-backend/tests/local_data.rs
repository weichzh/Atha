use std::{
    env, fs,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::{
    local_data::{
        BrowserState, BrowserStateRecord, LocalData, LocalDataError, MAX_BROWSER_STATE_BYTES,
    },
    messages::{
        EditionInput, MessageStore, RootMessageDraft, SourceAnchorInput, SourceSnapshotInput,
    },
    reader::{
        dictionary::LocalDictionaries,
        library::{LibraryError, LocalLibrary},
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

#[test]
fn complete_local_data_round_trip_rebuilds_cache_and_confirms_browser_state() {
    let source = TestRoot::new("round-trip-source");
    let archive = source.path().join("complete.atha-data");
    let source_book = source.path().join("source.txt");
    fs::write(&source_book, "第一章\n\n本地资料库往返。\n").expect("write source book");
    let library = LocalLibrary::open(source.path()).expect("open source library");
    let book = library
        .stage_with_title_hint(&source_book, Some("往返书籍"))
        .expect("stage source book");
    library.open_book(&book.id).expect("prepare source cache");
    let custom_cover = source.path().join("custom-cover.png");
    fs::write(&custom_cover, PNG_1X1).expect("write custom cover");
    let customized = library
        .set_custom_cover(&book.id, &custom_cover)
        .expect("set custom cover");
    assert!(customized.has_cover);
    assert!(customized.has_custom_cover);
    assert_eq!(
        library.cover(&book.id).expect("read custom cover").bytes,
        PNG_1X1
    );
    let invalid_cover = source.path().join("invalid-cover.png");
    fs::write(&invalid_cover, b"not an image").expect("write invalid cover");
    assert_eq!(
        library.set_custom_cover(&book.id, &invalid_cover),
        Err(LibraryError::InvalidCover)
    );
    let stored_cover = source.path().join(format!("Library/{}.cover", book.id));
    let previous_cover = source
        .path()
        .join(format!("Library/{}.cover.previous", book.id));
    let abandoned_cover = source.path().join("Library/.cover.staging-test");
    fs::rename(&stored_cover, &previous_cover).expect("simulate interrupted cover replacement");
    fs::write(&abandoned_cover, b"staging").expect("write abandoned cover");
    let library = LocalLibrary::open(source.path()).expect("recover source library");
    assert!(stored_cover.is_file());
    assert!(!previous_cover.exists());
    assert!(!abandoned_cover.exists());
    let dictionaries = LocalDictionaries::open(source.path()).expect("open source dictionaries");
    let messages = MessageStore::open(source.path()).expect("open source messages");
    messages
        .create_root(message_draft(&book.id))
        .expect("create source message");
    let data = LocalData::open(source.path()).expect("open source local data");
    let browser = browser_state(&book.id);

    data.create_backup(&archive, &browser, &library, &dictionaries, &messages)
        .expect("create complete backup");

    let destination = TestRoot::new("round-trip-destination");
    let destination_messages = MessageStore::open(destination.path()).expect("open messages");
    let destination_data = LocalData::open(destination.path()).expect("open local data");
    let orphan_asset = destination
        .path()
        .join(format!("Messages/Assets/{}", "f".repeat(64)));
    fs::write(&orphan_asset, "orphan").expect("write orphan asset");
    destination_data
        .recover(&destination_messages)
        .expect("recover empty destination");
    LocalLibrary::open(destination.path()).expect("open destination library");
    LocalDictionaries::open(destination.path()).expect("open destination dictionaries");
    let pending = destination_data
        .prepare_restore(&archive, &empty_browser_state(), &destination_messages)
        .expect("prepare restore");
    assert_eq!(pending.browser_state, browser);
    destination_data
        .commit_restore(&pending.token, &destination_messages)
        .expect("commit restore");
    assert!(!orphan_asset.exists());

    let restored = LocalLibrary::open(destination.path()).expect("reopen restored library");
    let listed = restored.list().expect("list restored library");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, book.id);
    assert_eq!(listed[0].title, "往返书籍");
    assert!(!listed[0].prepared);
    assert!(listed[0].has_cover);
    assert!(listed[0].has_custom_cover);
    assert_eq!(
        restored.cover(&book.id).expect("read restored cover").bytes,
        PNG_1X1
    );
    assert!(
        !destination
            .path()
            .join("ImportedBooks")
            .read_dir()
            .expect("read cache")
            .next()
            .is_some()
    );
    let restored_messages = destination_messages
        .roots(&book.id, None)
        .expect("list restored messages");
    assert_eq!(restored_messages.len(), 1);
    assert_eq!(restored_messages[0].text, "完整资料库消息");
    let reset = restored
        .reset_custom_cover(&book.id)
        .expect("reset restored cover");
    assert!(!reset.has_custom_cover);
    assert!(!reset.has_cover);
    restored
        .open_book(&book.id)
        .expect("rebuild restored cache");
    assert!(
        destination_messages
            .health()
            .expect("message health")
            .integrity
    );
    assert_eq!(
        destination_data
            .pending_restore()
            .expect("read pending restore")
            .expect("committed restore")
            .browser_state,
        browser
    );
    assert!(
        !destination_data
            .pending_restore()
            .expect("read pending restore")
            .expect("committed restore")
            .rollback
    );
    destination_data
        .finish_restore(&pending.token)
        .expect("finish restore");
    assert_eq!(
        destination_data.pending_restore().expect("no pending"),
        None
    );
}

#[test]
fn invalid_archive_entries_and_semantics_never_change_current_data() {
    let source = TestRoot::new("invalid-source");
    let (archive, book_id) = create_sample_backup(&source);
    let destination = TestRoot::new("invalid-destination");
    let current_source = destination.path().join("current.txt");
    fs::write(&current_source, "当前资料库\n").expect("write current source");
    let current_library = LocalLibrary::open(destination.path()).expect("open current library");
    let current = current_library
        .stage_with_title_hint(&current_source, Some("当前书籍"))
        .expect("stage current book");
    let messages = MessageStore::open(destination.path()).expect("open current messages");
    LocalDictionaries::open(destination.path()).expect("open current dictionaries");
    let data = LocalData::open(destination.path()).expect("open current local data");

    let unknown = source.path().join("unknown.atha-data");
    rewrite_archive(&archive, &unknown, |entries| {
        entries.push(("../outside".into(), b"outside".to_vec()));
    });
    assert_eq!(
        data.prepare_restore(&unknown, &empty_browser_state(), &messages),
        Err(LocalDataError::InvalidBackup)
    );

    let corrupt_source = source.path().join("corrupt-source.atha-data");
    rewrite_archive(&archive, &corrupt_source, |entries| {
        let changed = {
            let source_entry = entries
                .iter_mut()
                .find(|(name, _)| name.starts_with("SourceBooks/"))
                .expect("source entry");
            source_entry.1[0] ^= 0xff;
            source_entry.0.clone()
        };
        refresh_manifest(entries, &changed);
    });
    assert_eq!(
        data.prepare_restore(&corrupt_source, &empty_browser_state(), &messages),
        Err(LocalDataError::InvalidBackup)
    );

    let invalid_browser = source.path().join("invalid-browser.atha-data");
    rewrite_archive(&archive, &invalid_browser, |entries| {
        let browser = entries
            .iter_mut()
            .find(|(name, _)| name == "BrowserState.json")
            .expect("browser entry");
        browser.1 =
            br#"{"schema":1,"records":[{"key":"atha.reader.probe.application.v1","value":"{}"}]}"#
                .to_vec();
        refresh_manifest(entries, "BrowserState.json");
    });
    assert_eq!(
        data.prepare_restore(&invalid_browser, &empty_browser_state(), &messages),
        Err(LocalDataError::InvalidBackup)
    );

    let invalid_annotation = source.path().join("invalid-annotation.atha-data");
    rewrite_archive(&archive, &invalid_annotation, |entries| {
        let browser = entries
            .iter_mut()
            .find(|(name, _)| name == "BrowserState.json")
            .expect("browser entry");
        let mut state: Value = serde_json::from_slice(&browser.1).expect("parse browser state");
        let id = &book_id[..16];
        state["records"] = serde_json::json!([{
            "key": format!("atha.reader.annotations.{id}.v1"),
            "value": serde_json::json!({
                "schema": 1,
                "items": [{
                    "id": "tampered",
                    "type": "highlight",
                    "note": "",
                    "createdAt": 1,
                    "updatedAt": 1,
                    "deletedAt": null,
                    "sourceAnchor": {
                        "schema": 1,
                        "canonicalLocator": "locator",
                        "selectedText": "tampered",
                        "prefixText": "",
                        "suffixText": "",
                        "contentHash": "0".repeat(64)
                    }
                }]
            }).to_string()
        }]);
        browser.1 = serde_json::to_vec(&state).expect("serialize browser state");
        refresh_manifest(entries, "BrowserState.json");
    });
    assert_eq!(
        data.prepare_restore(&invalid_annotation, &empty_browser_state(), &messages),
        Err(LocalDataError::InvalidBackup)
    );

    assert_eq!(
        current_library.list().expect("current data remains")[0].id,
        current.id
    );
    assert_ne!(current.id, book_id);
    assert!(!destination.path().join("outside").exists());
}

#[test]
fn prepared_recovery_and_committed_rollback_restore_the_previous_dataset() {
    let source = TestRoot::new("recovery-source");
    let (archive, restored_id) = create_sample_backup(&source);
    let destination = TestRoot::new("recovery-destination");
    let current_source = destination.path().join("current.md");
    fs::write(&current_source, "# 当前书籍\n\n恢复前内容。\n").expect("write current source");
    let current_library = LocalLibrary::open(destination.path()).expect("open current library");
    let current = current_library
        .stage_with_title_hint(&current_source, None)
        .expect("stage current book");
    LocalDictionaries::open(destination.path()).expect("open dictionaries");
    let messages = MessageStore::open(destination.path()).expect("open messages");
    messages
        .create_root(message_draft(&current.id))
        .expect("create current message");
    let data = LocalData::open(destination.path()).expect("open local data");
    let previous = browser_state(&current.id);

    data.prepare_restore(&archive, &previous, &messages)
        .expect("prepare interrupted restore");
    data.recover(&messages).expect("recover prepared restore");
    assert_eq!(
        LocalLibrary::open(destination.path())
            .expect("reopen current library")
            .list()
            .expect("list current library")[0]
            .id,
        current.id
    );

    let pending = data
        .prepare_restore(&archive, &previous, &messages)
        .expect("prepare committed restore");
    data.commit_restore(&pending.token, &messages)
        .expect("commit restore");
    assert_eq!(
        LocalLibrary::open(destination.path())
            .expect("open restored library")
            .list()
            .expect("list restored library")[0]
            .id,
        restored_id
    );
    assert_eq!(
        data.rollback_restore(&pending.token, &messages)
            .expect("rollback restore"),
        previous
    );
    let rollback = data
        .pending_restore()
        .expect("read rollback pending")
        .expect("rollback remains pending until browser confirmation");
    assert!(rollback.rollback);
    assert_eq!(rollback.browser_state, previous);
    assert_eq!(
        LocalLibrary::open(destination.path())
            .expect("open rolled back library")
            .list()
            .expect("list rolled back library")[0]
            .id,
        current.id
    );
    messages
        .create_root(message_draft(&restored_id))
        .expect("simulate interrupted message rollback");
    data.recover(&messages)
        .expect("resume idempotent backend rollback");
    assert_eq!(
        messages
            .roots(&current.id, None)
            .expect("list previous messages")
            .len(),
        1
    );
    assert!(
        messages
            .roots(&restored_id, None)
            .expect("list rolled-back messages")
            .is_empty()
    );
    data.finish_restore(&pending.token)
        .expect("confirm browser rollback");
    assert_eq!(data.pending_restore().expect("rollback finished"), None);
}

#[test]
fn aborting_a_prepared_restore_does_not_rewrite_current_messages() {
    let source = TestRoot::new("abort-source");
    let (archive, _) = create_sample_backup(&source);
    let destination = TestRoot::new("abort-destination");
    let messages = MessageStore::open(destination.path()).expect("open messages");
    messages
        .create_root(message_draft(&"c".repeat(64)))
        .expect("create current message");
    LocalLibrary::open(destination.path()).expect("open library");
    LocalDictionaries::open(destination.path()).expect("open dictionaries");
    let data = LocalData::open(destination.path()).expect("open data");
    let database = destination.path().join("Messages/Messages.sqlite3");
    let before = fs::read(&database).expect("read database before prepare");
    let pending = data
        .prepare_restore(&archive, &empty_browser_state(), &messages)
        .expect("prepare restore");
    data.abort_restore(&pending.token).expect("abort restore");
    assert_eq!(
        fs::read(database).expect("read database after abort"),
        before
    );
    assert_eq!(data.pending_restore().expect("no pending restore"), None);
}

#[test]
fn storage_totals_and_physical_book_deletion_are_explicit() {
    let root = TestRoot::new("storage-delete");
    let source = root.path().join("delete.txt");
    fs::write(&source, "删除测试\n").expect("write source");
    let library = LocalLibrary::open(root.path()).expect("open library");
    let book = library
        .stage_with_title_hint(&source, None)
        .expect("stage book");
    library.open_book(&book.id).expect("prepare cache");
    LocalDictionaries::open(root.path()).expect("open dictionaries");
    MessageStore::open(root.path()).expect("open messages");
    let data = LocalData::open(root.path()).expect("open local data");
    let browser = browser_state(&book.id);
    let usage = data.storage_usage(&browser).expect("measure storage");
    assert_eq!(
        usage.total_bytes,
        usage.books_bytes
            + usage.cache_bytes
            + usage.messages_bytes
            + usage.dictionaries_bytes
            + usage.preferences_bytes
    );
    assert!(usage.books_bytes > 0);
    assert!(usage.cache_bytes > 0);

    library
        .prepare_local_data_deletion(&book.id)
        .expect("prepare physical book deletion");
    library
        .finish_local_data_deletion(&book.id)
        .expect("confirm physical book deletion");
    assert!(library.list().expect("list after delete").is_empty());
    assert!(!root.path().join("ImportedBooks").join(&book.id).exists());
    assert!(
        fs::read_dir(root.path().join("SourceBooks"))
            .expect("read sources")
            .next()
            .is_none()
    );
    assert_eq!(
        library
            .stage_with_title_hint(&source, None)
            .expect("reimport identical source")
            .id,
        book.id
    );

    let intent = root.path().join(format!(".atha-delete-{}", book.id));
    fs::write(&intent, []).expect("write interrupted deletion intent");
    let _ = fs::remove_dir_all(root.path().join("ImportedBooks").join(&book.id));
    let reopened = LocalLibrary::open(root.path()).expect("recover interrupted deletion");
    assert!(reopened.list().expect("list recovered library").is_empty());
    assert!(intent.exists());
    assert_eq!(
        reopened
            .pending_local_data_deletions()
            .expect("pending deletion")[0]
            .id,
        book.id
    );
    assert!(
        fs::read_dir(root.path().join("SourceBooks"))
            .expect("read recovered sources")
            .next()
            .is_none()
    );
    reopened
        .finish_local_data_deletion(&book.id)
        .expect("confirm browser-state deletion");
    assert!(!intent.exists());
}

#[cfg(unix)]
#[test]
fn owned_root_symlinks_are_rejected_without_deleting_external_data() {
    use std::os::unix::fs::symlink;

    let root = TestRoot::new("owned-root-symlink");
    let external = TestRoot::new("owned-root-external");
    let source = root.path().join("source.txt");
    fs::write(&source, "外部边界\n").expect("write source");
    let library = LocalLibrary::open(root.path()).expect("open library");
    let book = library
        .stage_with_title_hint(&source, None)
        .expect("stage book");
    let external_book = external.path().join(&book.id);
    fs::create_dir(&external_book).expect("create external book");
    let sentinel = external_book.join("keep.txt");
    fs::write(&sentinel, "keep").expect("write sentinel");

    let imports = root.path().join("ImportedBooks");
    fs::remove_dir(&imports).expect("remove empty imports");
    symlink(external.path(), &imports).expect("link imports");
    assert_eq!(
        library.prepare_local_data_deletion(&book.id),
        Err(LibraryError::InvalidRoot)
    );
    assert!(sentinel.is_file());
    fs::remove_file(&imports).expect("remove imports link");
    fs::create_dir(&imports).expect("restore imports");
    let _ = fs::remove_file(root.path().join(format!(".atha-delete-{}", book.id)));

    let dictionaries = root.path().join("Dictionaries");
    fs::create_dir(&dictionaries).expect("create dictionaries");
    fs::remove_dir(&dictionaries).expect("remove dictionaries");
    symlink(external.path(), &dictionaries).expect("link dictionaries");
    assert!(LocalDictionaries::open(root.path()).is_err());
    fs::remove_file(&dictionaries).expect("remove dictionaries link");

    let messages = root.path().join("Messages");
    MessageStore::open(root.path()).expect("create messages");
    fs::remove_dir_all(&messages).expect("remove messages");
    symlink(external.path(), &messages).expect("link messages");
    assert!(MessageStore::open(root.path()).is_err());
    assert!(!external.path().join("Assets").exists());
    fs::remove_file(messages).expect("remove messages link");
}

#[test]
fn complete_backup_reports_a_book_without_its_durable_source() {
    let root = TestRoot::new("missing-book-source");
    let source = root.path().join("legacy.txt");
    fs::write(&source, "旧缓存\n").expect("write source");
    let library = LocalLibrary::open(root.path()).expect("open library");
    let book = library
        .stage_with_title_hint(&source, None)
        .expect("stage book");
    fs::remove_file(root.path().join(format!("SourceBooks/{}.txt", book.id)))
        .expect("remove durable source");
    let dictionaries = LocalDictionaries::open(root.path()).expect("open dictionaries");
    let messages = MessageStore::open(root.path()).expect("open messages");
    let data = LocalData::open(root.path()).expect("open local data");
    let archive = root.path().join("missing.atha-data");
    assert_eq!(
        data.create_backup(
            &archive,
            &empty_browser_state(),
            &library,
            &dictionaries,
            &messages,
        ),
        Err(LocalDataError::MissingBookSource)
    );
    assert!(!archive.exists());
}

#[test]
fn interrupted_record_write_is_not_part_of_a_complete_backup() {
    let root = TestRoot::new("record-temp");
    let source = root.path().join("book.txt");
    fs::write(&source, "临时记录\n").expect("write source");
    let library = LocalLibrary::open(root.path()).expect("open library");
    let book = library
        .stage_with_title_hint(&source, None)
        .expect("stage book");
    let temporary = root.path().join(format!("Library/{}.12345.tmp", book.id));
    fs::write(&temporary, "partial").expect("write interrupted record");
    let dictionaries = LocalDictionaries::open(root.path()).expect("open dictionaries");
    let messages = MessageStore::open(root.path()).expect("open messages");
    let archive = root.path().join("complete.atha-data");
    LocalData::open(root.path())
        .expect("open data")
        .create_backup(
            &archive,
            &empty_browser_state(),
            &library,
            &dictionaries,
            &messages,
        )
        .expect("backup with interrupted record");
    let mut zip = ZipArchive::new(File::open(archive).expect("open backup")).expect("read backup");
    assert!(zip.by_name(&format!("Library/{}.json", book.id)).is_ok());
    assert!(
        zip.by_name(&format!("Library/{}.12345.tmp", book.id))
            .is_err()
    );
}

#[test]
fn removed_book_source_survives_backup_restore_and_recovers_identity() {
    let source = TestRoot::new("removed-source");
    let book_path = source.path().join("removed.txt");
    fs::write(&book_path, "移出书架后仍可恢复\n").expect("write source");
    let library = LocalLibrary::open(source.path()).expect("open source library");
    let book = library
        .stage_with_title_hint(&book_path, None)
        .expect("stage source book");
    library.remove(&book.id).expect("remove from shelf");
    let dictionaries = LocalDictionaries::open(source.path()).expect("open dictionaries");
    let messages = MessageStore::open(source.path()).expect("open messages");
    let archive = source.path().join("removed.atha-data");
    LocalData::open(source.path())
        .expect("open source data")
        .create_backup(
            &archive,
            &empty_browser_state(),
            &library,
            &dictionaries,
            &messages,
        )
        .expect("backup orphan source");

    let destination = TestRoot::new("removed-destination");
    LocalLibrary::open(destination.path()).expect("open destination library");
    LocalDictionaries::open(destination.path()).expect("open destination dictionaries");
    let destination_messages = MessageStore::open(destination.path()).expect("open messages");
    let data = LocalData::open(destination.path()).expect("open local data");
    let pending = data
        .prepare_restore(&archive, &empty_browser_state(), &destination_messages)
        .expect("prepare restore");
    data.commit_restore(&pending.token, &destination_messages)
        .expect("commit restore");
    data.finish_restore(&pending.token).expect("finish restore");
    let restored = LocalLibrary::open(destination.path()).expect("open restored library");
    assert!(restored.list().expect("list restored library").is_empty());
    assert_eq!(
        restored
            .stage_with_title_hint(&book_path, None)
            .expect("reimport same source")
            .id,
        book.id
    );
}

#[test]
fn browser_state_is_bounded_sorted_and_production_only() {
    let id = "a".repeat(64);
    assert!(browser_state(&id).validate().is_ok());
    let mut reversed = browser_state(&id);
    reversed.records.reverse();
    assert_eq!(
        reversed.validate(),
        Err(LocalDataError::InvalidBrowserState)
    );
    let probe = BrowserState {
        schema: 1,
        records: vec![BrowserStateRecord {
            key: "atha.reader.probe.application.v1".into(),
            value: r#"{"schema":1,"preferences":{}}"#.into(),
        }],
    };
    assert_eq!(probe.validate(), Err(LocalDataError::InvalidBrowserState));
    let mixed_versions = BrowserState {
        schema: 1,
        records: vec![
            BrowserStateRecord {
                key: "atha.reader.annotations.0123456789abcdef.v1".into(),
                value: format!(
                    r#"{{"schema":1,"items":[{{"sourceAnchor":{{"canonicalLocator":"{{\"schema\":1,\"contentVersion\":\"{}\",\"start\":{{\"section\":\"s\",\"offset\":0}},\"end\":{{\"section\":\"s\",\"offset\":1}}}}","selectedText":"x","contentHash":"{}"}}}}]}}"#,
                    "a".repeat(64),
                    hex(&Sha256::digest(b"x"))
                ),
            },
            BrowserStateRecord {
                key: "atha.reader.progress.0123456789abcdef.v1".into(),
                value: format!(
                    r#"{{"schema":1,"contentVersion":"{}","locator":"locator"}}"#,
                    "b".repeat(64)
                ),
            },
        ],
    };
    assert!(mixed_versions.validate().is_ok());
    let oversized = BrowserState {
        schema: 1,
        records: vec![BrowserStateRecord {
            key: "atha.reader.application.v1".into(),
            value: "x".repeat(MAX_BROWSER_STATE_BYTES as usize),
        }],
    };
    assert_eq!(
        oversized.validate(),
        Err(LocalDataError::InvalidBrowserState)
    );
}

#[test]
fn near_limit_browser_state_round_trips_compact_rollback_file() {
    let root = TestRoot::new("browser-boundary-source");
    let library = LocalLibrary::open(root.path()).expect("open library");
    let dictionaries = LocalDictionaries::open(root.path()).expect("open dictionaries");
    let messages = MessageStore::open(root.path()).expect("open messages");
    let archive = root.path().join("boundary.atha-data");
    LocalData::open(root.path())
        .expect("open local data")
        .create_backup(
            &archive,
            &empty_browser_state(),
            &library,
            &dictionaries,
            &messages,
        )
        .expect("create backup");

    let destination = TestRoot::new("browser-boundary-destination");
    LocalLibrary::open(destination.path()).expect("open destination library");
    LocalDictionaries::open(destination.path()).expect("open destination dictionaries");
    let destination_messages = MessageStore::open(destination.path()).expect("open messages");
    let data = LocalData::open(destination.path()).expect("open data");
    let records = (0..10_000)
        .map(|index| BrowserStateRecord {
            key: format!("atha.reader.book.{index:016x}.v1"),
            value: format!(
                r#"{{"schema":1,"bookmarks":[],"preferences":{{"padding":"{}"}}}}"#,
                "x".repeat(1_500)
            ),
        })
        .collect();
    let previous = BrowserState { schema: 1, records };
    assert!(
        serde_json::to_vec(&previous)
            .expect("serialize compact")
            .len()
            < MAX_BROWSER_STATE_BYTES as usize
    );
    let pending = data
        .prepare_restore(&archive, &previous, &destination_messages)
        .expect("prepare restore");
    assert_eq!(
        data.rollback_restore(&pending.token, &destination_messages)
            .expect("read compact rollback state"),
        previous
    );
}

#[test]
fn private_dictionary_round_trip_stays_content_free() {
    let Some(fixtures) = env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT").map(PathBuf::from) else {
        return;
    };
    let (mdx, resources) = private_mdict_files(&fixtures);
    let source = TestRoot::new("dictionary-source");
    let library = LocalLibrary::open(source.path()).expect("open source library");
    let dictionaries = LocalDictionaries::open(source.path()).expect("open source dictionaries");
    let imported = dictionaries
        .import_mdict(mdx, &resources)
        .expect("import private dictionary");
    let messages = MessageStore::open(source.path()).expect("open source messages");
    let data = LocalData::open(source.path()).expect("open source data");
    let archive = source.path().join("dictionary.atha-data");
    data.create_backup(
        &archive,
        &empty_browser_state(),
        &library,
        &dictionaries,
        &messages,
    )
    .expect("back up dictionary");

    let destination = TestRoot::new("dictionary-destination");
    LocalLibrary::open(destination.path()).expect("open destination library");
    LocalDictionaries::open(destination.path()).expect("open destination dictionaries");
    let destination_messages = MessageStore::open(destination.path()).expect("open messages");
    let destination_data = LocalData::open(destination.path()).expect("open destination data");
    let pending = destination_data
        .prepare_restore(&archive, &empty_browser_state(), &destination_messages)
        .expect("prepare dictionary restore");
    destination_data
        .commit_restore(&pending.token, &destination_messages)
        .expect("commit dictionary restore");
    assert_eq!(
        LocalDictionaries::open(destination.path())
            .expect("reopen dictionaries")
            .list()
            .expect("list dictionaries")
            .into_iter()
            .map(|dictionary| dictionary.id)
            .collect::<Vec<_>>(),
        vec![imported.id]
    );
    destination_data
        .finish_restore(&pending.token)
        .expect("finish dictionary restore");
}

fn create_sample_backup(root: &TestRoot) -> (PathBuf, String) {
    let source = root.path().join("backup.txt");
    fs::write(&source, "备份书籍\n").expect("write backup source");
    let library = LocalLibrary::open(root.path()).expect("open backup library");
    let book = library
        .stage_with_title_hint(&source, Some("备份书籍"))
        .expect("stage backup book");
    let dictionaries = LocalDictionaries::open(root.path()).expect("open backup dictionaries");
    let messages = MessageStore::open(root.path()).expect("open backup messages");
    let data = LocalData::open(root.path()).expect("open backup local data");
    let archive = root.path().join("sample.atha-data");
    data.create_backup(
        &archive,
        &browser_state(&book.id),
        &library,
        &dictionaries,
        &messages,
    )
    .expect("create sample backup");
    (archive, book.id)
}

fn message_draft(content_version: &str) -> RootMessageDraft {
    let selected_text = "资料";
    RootMessageDraft {
        edition: EditionInput {
            content_version: content_version.into(),
            title: "往返书籍".into(),
            authors: vec!["本地作者".into()],
        },
        anchor: SourceAnchorInput {
            canonical_locator: serde_json::json!({
                "schema": 1,
                "contentVersion": content_version,
                "start": { "section": "section-1", "offset": 0 },
                "end": { "section": "section-1", "offset": selected_text.encode_utf16().count() }
            })
            .to_string(),
            section: "section-1".into(),
            selected_text: selected_text.into(),
            prefix_text: String::new(),
            suffix_text: String::new(),
            content_hash: hex(&Sha256::digest(selected_text.as_bytes())),
        },
        snapshot: SourceSnapshotInput {
            fragment_html: "<p>资料</p>".into(),
            reader_css: ".book { color: #222; }".into(),
            book_css: "p { margin: 0; }".into(),
            user_css: String::new(),
            presentation_json: r#"{"schema":1,"theme":"paper","brightness":100,"fontSize":19,"fontFamily":"book","density":"standard"}"#.into(),
            resources: Vec::new(),
        },
        text: Some("完整资料库消息".into()),
    }
}

fn private_mdict_files(root: &Path) -> (PathBuf, Vec<PathBuf>) {
    let mut pending = vec![root.to_path_buf()];
    let mut mdx = Vec::new();
    let mut resources = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("read fixture directory") {
            let path = entry.expect("read fixture entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mdx"))
            {
                mdx.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mdd"))
            {
                resources.push(path);
            }
        }
    }
    assert_eq!(mdx.len(), 1, "fixture root must contain one MDX");
    resources.sort();
    (mdx.pop().expect("one MDX"), resources)
}

fn browser_state(id: &str) -> BrowserState {
    let key = &id[..16];
    BrowserState {
        schema: 1,
        records: vec![
            BrowserStateRecord {
                key: "atha.reader.application.v1".into(),
                value: r#"{"schema":1,"preferences":{"theme":"system","brightness":100,"fontSize":19,"fontFamily":"book","density":"standard"}}"#.into(),
            },
            BrowserStateRecord {
                key: format!("atha.reader.progress.{key}.v1"),
                value: format!(r#"{{"schema":1,"contentVersion":"{id}","locator":"locator"}}"#),
            },
        ],
    }
}

fn empty_browser_state() -> BrowserState {
    BrowserState {
        schema: 1,
        records: Vec::new(),
    }
}

fn rewrite_archive(source: &Path, target: &Path, mutate: impl FnOnce(&mut Vec<(String, Vec<u8>)>)) {
    let mut archive = ZipArchive::new(File::open(source).expect("open source archive"))
        .expect("read source archive");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read archive entry");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read entry bytes");
        entries.push((entry.name().to_owned(), bytes));
    }
    mutate(&mut entries);
    let file = File::create(target).expect("create rewritten archive");
    let mut output = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in entries {
        output
            .start_file(name, options)
            .expect("start rewritten entry");
        output.write_all(&bytes).expect("write rewritten entry");
    }
    output.finish().expect("finish rewritten archive");
}

fn refresh_manifest(entries: &mut [(String, Vec<u8>)], changed: &str) {
    let bytes = entries
        .iter()
        .find(|(name, _)| name == changed)
        .expect("changed entry")
        .1
        .clone();
    let manifest = entries
        .iter_mut()
        .find(|(name, _)| name == "manifest.json")
        .expect("manifest entry");
    let mut value: Value = serde_json::from_slice(&manifest.1).expect("parse manifest");
    let file = value
        .get_mut("files")
        .and_then(Value::as_array_mut)
        .expect("manifest files")
        .iter_mut()
        .find(|file| file.get("path").and_then(Value::as_str) == Some(changed))
        .expect("manifest file");
    file["byteLength"] = Value::from(bytes.len() as u64);
    file["contentHash"] = Value::from(hex(&Sha256::digest(bytes)));
    manifest.1 = serde_json::to_vec_pretty(&value).expect("serialize manifest");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(".tmp")
            .join(format!(
                "local-data-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
        fs::create_dir_all(&path).expect("create test root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
