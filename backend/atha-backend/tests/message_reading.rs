use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use atha_backend::messages::{
    EditionInput, LegacyAnnotationInput, LegacyImport, MessageSearch, MessageStore, ReplyDraft,
    ReselectDraft, RichTextInput, RootMessageDraft, SnapshotResourceInput, SourceAnchorInput,
    SourceSnapshotInput,
};
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-data")
            .join(format!("{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("create test root");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn edition() -> EditionInput {
    EditionInput {
        content_version: "11".repeat(32),
        title: "数学及其历史".into(),
        authors: vec!["测试作者".into()],
    }
}

fn text_hash(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(windows)]
fn link_directory(target: &Path, link: &Path) {
    let target = fs::canonicalize(target).expect("canonicalize junction target");
    let link = fs::canonicalize(link.parent().expect("junction parent"))
        .expect("canonicalize junction parent")
        .join(link.file_name().expect("junction name"));
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status()
        .expect("run mklink");
    assert!(status.success(), "create directory junction");
}

#[cfg(unix)]
fn link_directory(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create directory symlink");
}

#[cfg(windows)]
fn link_file(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link).expect("create file symlink");
}

#[cfg(unix)]
fn link_file(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create file symlink");
}

fn anchor() -> SourceAnchorInput {
    let selected_text = "算术与几何";
    SourceAnchorInput {
        canonical_locator: r#"{"schema":1,"contentVersion":"1111111111111111111111111111111111111111111111111111111111111111","start":{"section":"section-1","offset":10},"end":{"section":"section-1","offset":15}}"#.into(),
        section: "section-1".into(),
        selected_text: selected_text.into(),
        prefix_text: "第一章".into(),
        suffix_text: "之间".into(),
        content_hash: text_hash(selected_text),
    }
}

fn snapshot() -> SourceSnapshotInput {
    SourceSnapshotInput {
        fragment_html: "<p>算术与几何</p>".into(),
        reader_css: ".book { color: #222; }".into(),
        book_css: "p { text-indent: 2em; }".into(),
        user_css: String::new(),
        presentation_json: r#"{"schema":1,"theme":"paper","brightness":100,"fontSize":32,"fontFamily":"book","density":"standard"}"#.into(),
        resources: Vec::new(),
    }
}

fn snapshot_for(selected_text: &str) -> SourceSnapshotInput {
    let mut value = snapshot();
    value.fragment_html = format!("<p>{selected_text}</p>");
    value
}

fn set_anchor_text(anchor: &mut SourceAnchorInput, selected_text: &str) {
    let end = 10 + selected_text.encode_utf16().count();
    anchor.selected_text = selected_text.into();
    anchor.content_hash = text_hash(selected_text);
    anchor.canonical_locator = serde_json::json!({
        "schema": 1,
        "contentVersion": edition().content_version,
        "start": { "section": anchor.section, "offset": 10 },
        "end": { "section": anchor.section, "offset": end }
    })
    .to_string();
}

fn tamper_export_manifest(
    source: &Path,
    target: &Path,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let mut archive = ZipArchive::new(File::open(source).expect("open source export"))
        .expect("read source export");
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read export entry");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read export bytes");
        entries.push((entry.name().to_owned(), bytes));
    }
    let manifest = entries
        .iter_mut()
        .find(|(name, _)| name == "manifest.json")
        .expect("manifest entry");
    let mut value = serde_json::from_slice(&manifest.1).expect("parse manifest");
    mutate(&mut value);
    manifest.1 = serde_json::to_vec_pretty(&value).expect("serialize manifest");

    let mut writer = ZipWriter::new(File::create(target).expect("create tampered export"));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in entries {
        writer
            .start_file(name, options)
            .expect("start export entry");
        writer.write_all(&bytes).expect("write export entry");
    }
    writer.finish().expect("finish tampered export");
}

#[test]
fn source_only_highlight_is_a_retrievable_root_message() {
    let root = TestRoot::new("message-root");
    let store = MessageStore::open(&root.0).expect("open store");

    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: None,
        })
        .expect("create root message");
    let conversation = store
        .conversation(&created.conversation_id)
        .expect("load conversation");

    assert_eq!(conversation.messages.len(), 1);
    assert_eq!(conversation.messages[0].id, created.message_id);
    assert_eq!(conversation.messages[0].kind, "source-only");
    assert_eq!(conversation.messages[0].text, "");
    assert_eq!(
        conversation.messages[0]
            .source
            .as_ref()
            .expect("root source")
            .selected_text,
        "算术与几何"
    );
}

#[test]
fn adding_a_note_revises_the_same_message_and_rejects_stale_edits() {
    let root = TestRoot::new("message-revision");
    let store = MessageStore::open(&root.0).expect("open store");
    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: None,
        })
        .expect("create highlight");

    let revised = store
        .revise(
            &created.message_id,
            &created.revision_id,
            Some("这是我的笔记"),
        )
        .expect("add note");
    let conversation = store
        .conversation(&created.conversation_id)
        .expect("load conversation");
    let revisions = store
        .revisions(&created.message_id)
        .expect("load revisions");

    assert_eq!(conversation.messages.len(), 1);
    assert_eq!(conversation.messages[0].id, created.message_id);
    assert_eq!(conversation.messages[0].revision_id, revised.revision_id);
    assert_eq!(conversation.messages[0].kind, "text");
    assert_eq!(conversation.messages[0].text, "这是我的笔记");
    assert_eq!(
        revisions
            .iter()
            .map(|revision| (revision.kind.as_str(), revision.text.as_str()))
            .collect::<Vec<_>>(),
        vec![("source-only", ""), ("text", "这是我的笔记")]
    );
    assert_eq!(
        store.revise(&created.message_id, &created.revision_id, Some("过期写入")),
        Err(atha_backend::messages::MessageError::RevisionConflict)
    );
}

#[test]
fn replies_and_message_references_are_queryable_in_both_directions() {
    let root = TestRoot::new("message-relations");
    let store = MessageStore::open(&root.0).expect("open store");
    let first = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: Some("第一条笔记".into()),
        })
        .expect("create first root");
    let mut second_anchor = anchor();
    set_anchor_text(&mut second_anchor, "有理数");
    let second = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: second_anchor,
            snapshot: snapshot_for("有理数"),
            text: Some("第二条笔记".into()),
        })
        .expect("create second root");
    let mut third_anchor = anchor();
    set_anchor_text(&mut third_anchor, "无理数");
    let third = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: third_anchor,
            snapshot: snapshot_for("无理数"),
            text: Some("第三条笔记".into()),
        })
        .expect("create third root");
    let nested = store
        .reply(ReplyDraft {
            conversation_id: second.conversation_id.clone(),
            reply_to_message_id: second.message_id.clone(),
            text: "带引用的笔记".into(),
            rich_text: None,
            reference_ids: vec![third.message_id.clone()],
        })
        .expect("create nested reference");

    let reply = store
        .reply(ReplyDraft {
            conversation_id: first.conversation_id.clone(),
            reply_to_message_id: first.message_id.clone(),
            text: "把两处内容联系起来".into(),
            rich_text: None,
            reference_ids: vec![second.message_id.clone(), nested.message_id.clone()],
        })
        .expect("create reply");
    let conversation = store
        .conversation(&first.conversation_id)
        .expect("load conversation");
    let outgoing = store
        .relationships(&reply.message_id)
        .expect("reply relationships");
    let incoming = store
        .relationships(&second.message_id)
        .expect("referenced relationships");
    let root_incoming = store
        .relationships(&first.message_id)
        .expect("reply parent relationships");

    assert_eq!(conversation.messages.len(), 2);
    assert_eq!(
        conversation.messages[1].reply_to_message_id,
        Some(first.message_id.clone())
    );
    let mut direct_reference_ids = conversation.messages[1].reference_ids.clone();
    direct_reference_ids.sort();
    let mut expected_reference_ids = vec![second.message_id.clone(), nested.message_id.clone()];
    expected_reference_ids.sort();
    assert_eq!(direct_reference_ids, expected_reference_ids);
    let mut preview_texts = conversation.messages[1]
        .reference_previews
        .iter()
        .map(|preview| preview.text.as_str())
        .collect::<Vec<_>>();
    preview_texts.sort();
    assert_eq!(preview_texts, vec!["带引用的笔记", "第二条笔记"]);
    assert!(
        !conversation.messages[1]
            .reference_ids
            .contains(&third.message_id)
    );
    assert!(conversation.messages[1].created_at > 0);
    assert!(conversation.messages[1].updated_at >= conversation.messages[1].created_at);
    let mut outgoing_ids = outgoing.references;
    outgoing_ids.sort();
    let mut expected_outgoing_ids = expected_reference_ids;
    expected_outgoing_ids.push(first.message_id.clone());
    expected_outgoing_ids.sort();
    assert_eq!(outgoing_ids, expected_outgoing_ids);
    let mut incoming_ids = incoming.referenced_by;
    incoming_ids.sort();
    let mut expected_incoming_ids = vec![nested.message_id, reply.message_id.clone()];
    expected_incoming_ids.sort();
    assert_eq!(incoming_ids, expected_incoming_ids);
    assert_eq!(root_incoming.referenced_by, vec![reply.message_id]);
}

#[test]
fn soft_delete_returns_a_tombstone_without_losing_history_or_relations() {
    let root = TestRoot::new("message-delete");
    let store = MessageStore::open(&root.0).expect("open store");
    let source = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: Some("原始笔记".into()),
        })
        .expect("create source");
    let reply = store
        .reply(ReplyDraft {
            conversation_id: source.conversation_id.clone(),
            reply_to_message_id: source.message_id.clone(),
            text: "保留回复".into(),
            rich_text: None,
            reference_ids: Vec::new(),
        })
        .expect("create reply");

    store
        .delete(&source.message_id, &source.revision_id)
        .expect("soft delete");
    let conversation = store
        .conversation(&source.conversation_id)
        .expect("load conversation");
    let revisions = store.revisions(&source.message_id).expect("load history");

    assert_eq!(conversation.messages.len(), 2);
    assert!(conversation.messages[0].deleted);
    assert_eq!(conversation.messages[0].text, "");
    assert!(conversation.messages[0].source.is_none());
    assert_eq!(conversation.messages[1].id, reply.message_id);
    assert_eq!(revisions[0].text, "原始笔记");
}

#[test]
fn search_indexes_only_current_undeleted_revisions_and_filters_sections() {
    let root = TestRoot::new("message-search");
    let store = MessageStore::open(&root.0).expect("open store");
    let first = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: Some("这是一条原始笔记".into()),
        })
        .expect("create first");
    let mut second_anchor = anchor();
    second_anchor.section = "section-2".into();
    set_anchor_text(&mut second_anchor, "代数结构");
    let second = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: second_anchor,
            snapshot: snapshot_for("代数结构"),
            text: Some("即将删除的笔记".into()),
        })
        .expect("create second");
    store
        .revise(
            &first.message_id,
            &first.revision_id,
            Some("更新后的几何笔记"),
        )
        .expect("revise first");
    store
        .delete(&second.message_id, &second.revision_id)
        .expect("delete second");

    let current = store
        .search(MessageSearch {
            edition_id: edition().content_version,
            text: "更新后的几何".into(),
            section: Some("section-1".into()),
        })
        .expect("search current");
    let old = store
        .search(MessageSearch {
            edition_id: edition().content_version,
            text: "原始笔记".into(),
            section: None,
        })
        .expect("search old");
    let deleted = store
        .search(MessageSearch {
            edition_id: edition().content_version,
            text: "即将删除".into(),
            section: None,
        })
        .expect("search deleted");

    assert_eq!(current.len(), 1);
    assert_eq!(current[0].message_id, first.message_id);
    assert_eq!(current[0].section, "section-1");
    assert!(old.is_empty());
    assert!(deleted.is_empty());
}

#[test]
fn source_snapshot_resources_are_content_addressed_and_retrievable() {
    let root = TestRoot::new("message-snapshot-resource");
    let store = MessageStore::open(&root.0).expect("open store");
    let mut captured = snapshot();
    captured.fragment_html = "<p>算术与几何<img src=\"images/formula.png\"></p>".into();
    captured.resources.push(SnapshotResourceInput {
        path: "images/formula.png".into(),
        media_type: "image/png".into(),
        bytes: b"safe local image".to_vec(),
    });

    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: captured,
            text: None,
        })
        .expect("create source snapshot");
    let captures = store
        .source_captures(&created.message_id)
        .expect("load captures");
    let resource = store
        .read_snapshot_resource(&captures[0].source.id, "images/formula.png")
        .expect("read snapshot resource");

    assert_eq!(captures.len(), 1);
    assert!(captures[0].current);
    assert_eq!(
        captures[0].snapshot.fragment_html,
        "<p>算术与几何<img src=\"images/formula.png\"></p>"
    );
    assert_eq!(captures[0].snapshot.resources.len(), 1);
    assert_eq!(captures[0].snapshot.resources[0].content_hash.len(), 64);
    assert_eq!(resource.media_type, "image/png");
    assert_eq!(resource.bytes, b"safe local image");

    let source_id = captures[0].source.id.clone();
    let asset = root
        .0
        .join("Messages/Assets")
        .join(&captures[0].snapshot.resources[0].content_hash);
    let linked_asset = root.0.join("linked-asset");
    fs::write(&linked_asset, b"safe local image").expect("write linked asset");
    fs::remove_file(&asset).expect("remove original asset");
    link_file(&linked_asset, &asset);
    assert!(!store.health().expect("corrupt health").integrity);
    assert_eq!(
        store.read_snapshot_resource(&source_id, "images/formula.png"),
        Err(atha_backend::messages::MessageError::CorruptData)
    );
    let archive = root.0.join("linked-resource-export.zip");
    assert_eq!(
        store.export_edition(&edition().content_version, &archive),
        Err(atha_backend::messages::MessageError::CorruptData)
    );
    assert!(!archive.exists());
    drop(store);

    let reopened = MessageStore::open(&root.0).expect("reopen corrupt store");
    assert!(asset.is_file());
    assert_eq!(
        reopened.read_snapshot_resource(&source_id, "images/formula.png"),
        Err(atha_backend::messages::MessageError::CorruptData)
    );
}

#[test]
fn message_store_rejects_linked_assets_directory() {
    let root = TestRoot::new("message-linked-assets-directory");
    let messages = root.0.join("Messages");
    let outside = root.0.join("outside-assets");
    let assets = messages.join("Assets");
    fs::create_dir_all(&messages).expect("create messages directory");
    fs::create_dir_all(&outside).expect("create outside assets directory");
    link_directory(&outside, &assets);

    assert!(matches!(
        MessageStore::open(&root.0),
        Err(atha_backend::messages::MessageError::InvalidRoot)
    ));
    fs::remove_dir(&assets).expect("remove linked assets directory");
}

#[test]
fn source_snapshot_rejects_active_markup_unbound_assets_and_wrong_edition() {
    let root = TestRoot::new("message-snapshot-validation");
    let store = MessageStore::open(&root.0).expect("open store");
    for fragment_html in [
        "<script>alert(1)</script>",
        "<p onclick=\"alert(1)\">算术与几何</p>",
        "<img src=\"images/missing.png\">",
    ] {
        let mut invalid = snapshot();
        invalid.fragment_html = fragment_html.into();
        assert_eq!(
            store.create_root(RootMessageDraft {
                edition: edition(),
                anchor: anchor(),
                snapshot: invalid,
                text: None,
            }),
            Err(atha_backend::messages::MessageError::InvalidInput)
        );
    }
    for change in [
        |snapshot: &mut SourceSnapshotInput| snapshot.presentation_json = "{}".into(),
        |snapshot: &mut SourceSnapshotInput| {
            snapshot.book_css = "p { background: url(images/a.png); }".into()
        },
        |snapshot: &mut SourceSnapshotInput| {
            snapshot.user_css = "p { background: image-set('a.png' 1x); }".into()
        },
        |snapshot: &mut SourceSnapshotInput| {
            snapshot.book_css = "p { background: src('a.png'); }".into()
        },
        |snapshot: &mut SourceSnapshotInput| {
            snapshot.book_css = "p { background: image('a.png'); }".into()
        },
        |snapshot: &mut SourceSnapshotInput| {
            snapshot.book_css = r"p { background: u\72l('a.png'); }".into()
        },
    ] {
        let mut invalid = snapshot();
        change(&mut invalid);
        assert_eq!(
            store.create_root(RootMessageDraft {
                edition: edition(),
                anchor: anchor(),
                snapshot: invalid,
                text: None,
            }),
            Err(atha_backend::messages::MessageError::InvalidInput)
        );
    }
    let mut wrong_edition = edition();
    wrong_edition.content_version = "22".repeat(32);
    assert_eq!(
        store.create_root(RootMessageDraft {
            edition: wrong_edition,
            anchor: anchor(),
            snapshot: snapshot(),
            text: None,
        }),
        Err(atha_backend::messages::MessageError::InvalidInput)
    );
}

#[test]
fn reselect_switches_the_current_source_without_rewriting_old_captures() {
    let root = TestRoot::new("message-reselect");
    let store = MessageStore::open(&root.0).expect("open store");
    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: Some("关联两处内容".into()),
        })
        .expect("create root");
    let original = store
        .source_captures(&created.message_id)
        .expect("load original");
    let mut replacement_anchor = anchor();
    replacement_anchor.section = "section-2".into();
    set_anchor_text(&mut replacement_anchor, "代数结构");
    let mut replacement_snapshot = snapshot();
    replacement_snapshot.fragment_html = "<p>代数结构</p>".into();

    let replacement = store
        .reselect(ReselectDraft {
            message_id: created.message_id.clone(),
            expected_source_id: original[0].source.id.clone(),
            anchor: replacement_anchor,
            snapshot: replacement_snapshot,
        })
        .expect("reselect source");
    let captures = store
        .source_captures(&created.message_id)
        .expect("load captures");
    let conversation = store
        .conversation(&created.conversation_id)
        .expect("load conversation");

    assert_eq!(captures.len(), 2);
    assert!(!captures[0].current);
    assert_eq!(captures[0].source.selected_text, "算术与几何");
    assert!(captures[1].current);
    assert_eq!(captures[1].source.id, replacement.source_id);
    assert_eq!(captures[1].snapshot.fragment_html, "<p>代数结构</p>");
    assert_eq!(
        conversation.messages[0]
            .source
            .as_ref()
            .expect("current source")
            .selected_text,
        "代数结构"
    );
}

#[test]
fn reopening_the_same_edition_restores_the_message_conversation() {
    let root = TestRoot::new("message-reopen");
    let store = MessageStore::open(&root.0).expect("open store");
    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: None,
        })
        .expect("create highlight");
    store
        .revise(
            &created.message_id,
            &created.revision_id,
            Some("重启后保留的笔记"),
        )
        .expect("add note");
    store
        .reply(ReplyDraft {
            conversation_id: created.conversation_id.clone(),
            reply_to_message_id: created.message_id.clone(),
            text: "重启后保留的回复".into(),
            rich_text: None,
            reference_ids: Vec::new(),
        })
        .expect("add reply");
    let original = store
        .source_captures(&created.message_id)
        .expect("load source");
    let mut replacement_anchor = anchor();
    set_anchor_text(&mut replacement_anchor, "勾股定理");
    store
        .reselect(ReselectDraft {
            message_id: created.message_id.clone(),
            expected_source_id: original[0].source.id.clone(),
            anchor: replacement_anchor,
            snapshot: snapshot_for("勾股定理"),
        })
        .expect("reselect source");
    drop(store);

    let reopened = MessageStore::open(&root.0).expect("reopen store");
    let conversation = reopened
        .conversation(&created.conversation_id)
        .expect("restore conversation");
    let captures = reopened
        .source_captures(&created.message_id)
        .expect("restore captures");

    assert_eq!(conversation.messages.len(), 2);
    assert_eq!(conversation.messages[0].text, "重启后保留的笔记");
    assert_eq!(conversation.messages[1].text, "重启后保留的回复");
    assert_eq!(
        conversation.messages[0]
            .source
            .as_ref()
            .expect("current source")
            .selected_text,
        "勾股定理"
    );
    assert_eq!(captures.len(), 2);
}

#[test]
fn legacy_annotation_import_is_atomic_and_idempotent() {
    let root = TestRoot::new("message-legacy-import");
    let store = MessageStore::open(&root.0).expect("open store");
    let input = LegacyImport {
        edition: edition(),
        source_key: "atha.reader.annotations.math-history.v1".into(),
        record_hash: "33".repeat(32),
        items: vec![
            LegacyAnnotationInput {
                id: "highlight-1".into(),
                anchor: anchor(),
                note: None,
                created_at: 100,
                updated_at: 100,
                deleted_at: None,
            },
            LegacyAnnotationInput {
                id: "note-2".into(),
                anchor: anchor(),
                note: Some("旧版笔记".into()),
                created_at: 200,
                updated_at: 300,
                deleted_at: Some(400),
            },
        ],
    };

    let mut invalid = input.clone();
    invalid.items[1].anchor.content_hash = "00".repeat(32);
    assert_eq!(
        store.import_legacy_annotations(invalid),
        Err(atha_backend::messages::MessageError::InvalidInput)
    );

    let imported = store
        .import_legacy_annotations(input.clone())
        .expect("import legacy annotations");
    let repeated = store
        .import_legacy_annotations(input)
        .expect("repeat legacy import");
    let active = store
        .conversation(&imported.items[0].conversation_id)
        .expect("load active import");
    let deleted = store
        .conversation(&imported.items[1].conversation_id)
        .expect("load deleted import");

    assert_eq!(imported.imported, 2);
    assert!(!imported.already_complete);
    assert_eq!(repeated.imported, 0);
    assert!(repeated.already_complete);
    assert_eq!(repeated.items, imported.items);
    assert!(!active.messages[0].deleted);
    assert_eq!(active.messages[0].kind, "source-only");
    assert!(deleted.messages[0].deleted);
    assert_eq!(
        store
            .revisions(&imported.items[1].message_id)
            .expect("load imported revision")[0]
            .text,
        "旧版笔记"
    );
}

#[test]
fn database_migrations_and_required_capabilities_are_verified_on_open() {
    let root = TestRoot::new("message-database-health");
    let first = MessageStore::open(&root.0).expect("open empty database");
    let health = first.health().expect("database health");
    drop(first);
    MessageStore::open(&root.0).expect("repeat open");

    assert_eq!(health.schema_version, 2);
    assert!(health.foreign_keys);
    assert!(health.fts5);
    assert!(health.integrity);

    let future = TestRoot::new("message-future-database");
    let database_root = future.0.join("Messages");
    fs::create_dir_all(&database_root).expect("create database root");
    let connection = rusqlite::Connection::open(database_root.join("Messages.sqlite3"))
        .expect("open future database");
    connection
        .pragma_update(None, "user_version", 999)
        .expect("set future version");
    drop(connection);
    assert!(matches!(
        MessageStore::open(&future.0),
        Err(atha_backend::messages::MessageError::FutureDatabase)
    ));
}

#[test]
fn outbox_failure_recovers_snapshot_assets_on_reopen() {
    let root = TestRoot::new("message-outbox-rollback");
    let store = MessageStore::open(&root.0).expect("open store");
    let database = root.0.join("Messages/Messages.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open fault injector");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_message_outbox BEFORE INSERT ON outbox_event
             BEGIN SELECT RAISE(ABORT, 'forced outbox failure'); END;",
        )
        .expect("install outbox fault");
    drop(connection);
    let mut failed_anchor = anchor();
    set_anchor_text(&mut failed_anchor, "事务回滚原文");
    let resource_bytes = b"orphaned snapshot resource".to_vec();
    let asset_name = text_hash("orphaned snapshot resource");
    let assets = root.0.join("Messages/Assets");
    let asset = assets.join(&asset_name);
    let temporary = assets.join(".atha-asset-interrupted.tmp");
    let unknown = assets.join("leave-me-alone");
    let mut failed_snapshot = snapshot_for("事务回滚原文");
    failed_snapshot.fragment_html =
        "<p>事务回滚原文<img src=\"images/interrupted.png\"></p>".into();
    failed_snapshot.resources.push(SnapshotResourceInput {
        path: "images/interrupted.png".into(),
        media_type: "image/png".into(),
        bytes: resource_bytes.clone(),
    });

    assert_eq!(
        store.create_root(RootMessageDraft {
            edition: edition(),
            anchor: failed_anchor.clone(),
            snapshot: failed_snapshot,
            text: Some("不能留下来的笔记".into()),
        }),
        Err(atha_backend::messages::MessageError::Database)
    );
    assert!(asset.is_file());
    fs::write(&asset, b"truncated").expect("simulate interrupted final asset");
    fs::write(&temporary, b"partial").expect("simulate interrupted temporary asset");
    fs::write(&unknown, b"not managed by Atha").expect("write unknown asset file");
    drop(store);

    let store = MessageStore::open(&root.0).expect("recover store");
    assert!(!asset.exists());
    assert!(!temporary.exists());
    assert!(unknown.is_file());
    let connection = rusqlite::Connection::open(database).expect("reopen fault injector");
    connection
        .execute_batch("DROP TRIGGER fail_message_outbox;")
        .expect("remove outbox fault");
    drop(connection);
    let matches = store
        .search(MessageSearch {
            edition_id: edition().content_version,
            text: "不能留下来的笔记".into(),
            section: None,
        })
        .expect("search rolled back fact");

    assert!(matches.is_empty());

    let mut recovered_snapshot = snapshot_for("事务回滚原文");
    recovered_snapshot.fragment_html =
        "<p>事务回滚原文<img src=\"images/interrupted.png\"></p>".into();
    recovered_snapshot.resources.push(SnapshotResourceInput {
        path: "images/interrupted.png".into(),
        media_type: "image/png".into(),
        bytes: resource_bytes.clone(),
    });
    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: failed_anchor,
            snapshot: recovered_snapshot,
            text: Some("重试后的笔记".into()),
        })
        .expect("retry recovered snapshot");
    let captures = store
        .source_captures(&created.message_id)
        .expect("load recovered capture");
    assert_eq!(
        store
            .read_snapshot_resource(&captures[0].source.id, "images/interrupted.png")
            .expect("read recovered resource")
            .bytes,
        resource_bytes
    );
    assert!(store.health().expect("recovered health").integrity);
    drop(store);

    let reopened = MessageStore::open(&root.0).expect("reopen recovered store");
    assert!(unknown.is_file());
    assert!(reopened.health().expect("reopened health").integrity);
}

#[test]
fn edition_export_is_self_contained_and_passes_public_inspection() {
    let root = TestRoot::new("message-export");
    let store = MessageStore::open(&root.0).expect("open store");
    let mut first_snapshot = snapshot();
    first_snapshot.fragment_html = "<p>算术与几何<img src=\"images/proof.png\"></p>".into();
    first_snapshot.resources.push(SnapshotResourceInput {
        path: "images/proof.png".into(),
        media_type: "image/png".into(),
        bytes: b"exported image".to_vec(),
    });
    let first = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: first_snapshot,
            text: Some("第一版笔记".into()),
        })
        .expect("create first");
    store
        .revise(&first.message_id, &first.revision_id, Some("第二版笔记"))
        .expect("revise first");
    let mut second_anchor = anchor();
    set_anchor_text(&mut second_anchor, "数学证明");
    let second = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: second_anchor,
            snapshot: snapshot_for("数学证明"),
            text: None,
        })
        .expect("create second");
    let second_reply = store
        .reply(ReplyDraft {
            conversation_id: second.conversation_id.clone(),
            reply_to_message_id: second.message_id.clone(),
            text: "另一条标注的首层回复".into(),
            rich_text: None,
            reference_ids: Vec::new(),
        })
        .expect("create first nested reply");
    let nested_reply = store
        .reply(ReplyDraft {
            conversation_id: second.conversation_id.clone(),
            reply_to_message_id: second_reply.message_id,
            text: "另一条标注的二层回复".into(),
            rich_text: None,
            reference_ids: Vec::new(),
        })
        .expect("create second nested reply");
    store
        .reply(ReplyDraft {
            conversation_id: first.conversation_id.clone(),
            reply_to_message_id: first.message_id.clone(),
            text: "引用另一条标注".into(),
            rich_text: None,
            reference_ids: vec![nested_reply.message_id],
        })
        .expect("create referenced reply");
    let mut unrelated_anchor = anchor();
    set_anchor_text(&mut unrelated_anchor, "无关章节");
    store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: unrelated_anchor,
            snapshot: snapshot_for("无关章节"),
            text: Some("不应进入单对话导出".into()),
        })
        .expect("create unrelated conversation");
    let archive = root.0.join("math-history.atha-messages.zip");
    let conversation_archive = root.0.join("conversation.atha-messages.zip");

    store
        .export_edition(&edition().content_version, &archive)
        .expect("export edition");
    store
        .export_conversation(&first.conversation_id, &conversation_archive)
        .expect("export conversation");
    let inspected = MessageStore::inspect_export(&archive).expect("inspect export");
    let conversation_inspected =
        MessageStore::inspect_export(&conversation_archive).expect("inspect conversation export");

    assert_eq!(inspected.edition_id, edition().content_version);
    assert_eq!(inspected.conversations, 3);
    assert_eq!(inspected.messages, 6);
    assert_eq!(inspected.revisions, 7);
    assert_eq!(inspected.sources, 3);
    assert_eq!(inspected.snapshots, 3);
    assert_eq!(inspected.relationships, 1);
    assert_eq!(inspected.resources, 1);
    assert_eq!(conversation_inspected.conversations, 2);
    assert_eq!(conversation_inspected.messages, 5);
    assert_eq!(conversation_inspected.relationships, 1);
    assert_eq!(conversation_inspected.resources, 1);
    assert!(
        !String::from_utf8_lossy(&fs::read(archive).expect("read export"))
            .contains(root.0.to_string_lossy().as_ref())
    );

    for corruption in [
        "revision",
        "snapshot",
        "source",
        "relationship",
        "root-parent",
        "reply-cycle",
        "reply-source",
        "parent-reference",
    ] {
        let tampered = root.0.join(format!("tampered-{corruption}.zip"));
        tamper_export_manifest(
            &conversation_archive,
            &tampered,
            |manifest| match corruption {
                "revision" => manifest["revisions"][0]["content"]["schema"] = 9.into(),
                "snapshot" => manifest["snapshots"][0]["presentation"]["theme"] = "system".into(),
                "source" => manifest["sources"][0]["contentHash"] = "00".repeat(32).into(),
                "relationship" => manifest["relationships"][0]["kind"] = "recursive".into(),
                "root-parent" => {
                    let root = manifest["messages"]
                        .as_array_mut()
                        .expect("messages")
                        .iter_mut()
                        .find(|message| message["replyToMessageId"].is_null())
                        .expect("root message");
                    let id = root["id"].clone();
                    root["replyToMessageId"] = id;
                }
                "reply-cycle" => {
                    let reply = manifest["messages"]
                        .as_array_mut()
                        .expect("messages")
                        .iter_mut()
                        .find(|message| !message["replyToMessageId"].is_null())
                        .expect("reply message");
                    let id = reply["id"].clone();
                    reply["replyToMessageId"] = id;
                }
                "reply-source" => {
                    let reply_id = manifest["messages"]
                        .as_array()
                        .expect("messages")
                        .iter()
                        .find(|message| !message["replyToMessageId"].is_null())
                        .expect("reply message")["id"]
                        .clone();
                    let mut source = manifest["sources"][0].clone();
                    source["id"] = "ab".repeat(16).into();
                    source["messageId"] = reply_id;
                    manifest["sources"]
                        .as_array_mut()
                        .expect("sources")
                        .push(source);
                }
                "parent-reference" => {
                    let source_id = manifest["relationships"][0]["sourceMessageId"].clone();
                    let parent_id = manifest["messages"]
                        .as_array()
                        .expect("messages")
                        .iter()
                        .find(|message| message["id"] == source_id)
                        .expect("relationship source")["replyToMessageId"]
                        .clone();
                    manifest["relationships"][0]["targetMessageId"] = parent_id;
                }
                _ => unreachable!(),
            },
        );
        assert_eq!(
            MessageStore::inspect_export(&tampered),
            Err(atha_backend::messages::MessageError::InvalidExport),
            "accepts tampered {corruption}"
        );
    }
}

#[test]
fn edition_roots_are_a_single_section_filterable_projection() {
    let root = TestRoot::new("message-roots");
    let store = MessageStore::open(&root.0).expect("open store");
    let first = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: Some("第一章笔记".into()),
        })
        .expect("create first");
    let mut second_anchor = anchor();
    second_anchor.section = "section-2".into();
    set_anchor_text(&mut second_anchor, "第二章原文");
    store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: second_anchor,
            snapshot: snapshot_for("第二章原文"),
            text: None,
        })
        .expect("create second");
    std::thread::sleep(std::time::Duration::from_millis(2));
    store
        .reply(ReplyDraft {
            conversation_id: first.conversation_id.clone(),
            reply_to_message_id: first.message_id.clone(),
            text: "回复不应成为根消息列表项".into(),
            rich_text: None,
            reference_ids: Vec::new(),
        })
        .expect("create reply");

    let all = store
        .roots(&edition().content_version, None)
        .expect("list roots");
    let section = store
        .roots(&edition().content_version, Some("section-2"))
        .expect("filter roots");
    let conversations = store
        .conversations(&edition().content_version, None)
        .expect("list conversations");
    let first_section = store
        .conversations(&edition().content_version, Some("section-1"))
        .expect("filter conversations");

    assert_eq!(all.len(), 2);
    assert_eq!(all[0].conversation_id, first.conversation_id);
    assert_eq!(section.len(), 1);
    assert_eq!(section[0].kind, "source-only");
    assert_eq!(section[0].source.selected_text, "第二章原文");
    assert_eq!(conversations.len(), 2);
    assert_eq!(first_section.len(), 1);
    assert_eq!(first_section[0].id, first.conversation_id);
    assert_eq!(first_section[0].messages.len(), 2);
}

#[test]
fn automatic_reanchor_updates_only_the_current_locator() {
    let root = TestRoot::new("message-reanchor");
    let store = MessageStore::open(&root.0).expect("open store");
    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: None,
        })
        .expect("create root");
    let before = store
        .source_captures(&created.message_id)
        .expect("load source");
    let replacement = r#"{"schema":1,"contentVersion":"1111111111111111111111111111111111111111111111111111111111111111","start":{"section":"section-1","offset":20},"end":{"section":"section-1","offset":25}}"#;

    store
        .reanchor_source(
            &before[0].source.id,
            &before[0].source.canonical_locator,
            replacement,
        )
        .expect("update current locator");
    let after = store
        .source_captures(&created.message_id)
        .expect("reload source");

    assert_eq!(after.len(), 1);
    assert_eq!(after[0].source.original_locator, anchor().canonical_locator);
    assert_eq!(after[0].source.canonical_locator, replacement);
    assert_eq!(after[0].snapshot.fragment_html, snapshot().fragment_html);
    assert_eq!(
        store.reanchor_source(
            &after[0].source.id,
            &before[0].source.canonical_locator,
            &anchor().canonical_locator,
        ),
        Err(atha_backend::messages::MessageError::RevisionConflict)
    );
}

#[test]
fn rich_text_is_validated_and_keeps_a_plain_projection() {
    let root = TestRoot::new("message-rich-text");
    let store = MessageStore::open(&root.0).expect("open store");
    let created = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: anchor(),
            snapshot: snapshot(),
            text: None,
        })
        .expect("create root");
    let document = serde_json::json!({
        "type": "doc",
        "content": [
            {"type": "paragraph", "content": [
                {"type": "text", "marks": [{"type": "bold"}], "text": "第一行"}
            ]},
            {"type": "paragraph", "content": [
                {"type": "text", "marks": [{"type": "link", "attrs": {
                    "href": "https://example.com", "target": null,
                    "rel": "noopener noreferrer", "class": null, "title": null
                }}], "text": "第二行"}
            ]}
        ]
    });

    store
        .revise_rich(
            &created.message_id,
            &created.revision_id,
            RichTextInput {
                schema: 1,
                document,
            },
        )
        .expect("revise with rich text");
    let conversation = store
        .conversation(&created.conversation_id)
        .expect("read conversation");

    assert_eq!(conversation.messages[0].text, "第一行\n第二行");
    assert!(conversation.messages[0].content_json.contains("richText"));
    assert_eq!(
        store.revise_rich(
            &created.message_id,
            &conversation.messages[0].revision_id,
            RichTextInput {
                schema: 1,
                document: serde_json::json!({
                    "type": "doc",
                    "content": [{"type": "paragraph", "content": [{
                        "type": "text",
                        "marks": [{"type": "link", "attrs": {"href": "javascript:alert(1)"}}],
                        "text": "危险链接"
                    }]}]
                }),
            },
        ),
        Err(atha_backend::messages::MessageError::InvalidInput)
    );
}
