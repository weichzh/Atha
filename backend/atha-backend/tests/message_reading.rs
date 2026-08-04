use std::{fs, path::PathBuf, time::SystemTime};

use atha_backend::messages::{
    EditionInput, LegacyAnnotationInput, LegacyImport, MessageSearch, MessageStore, ReplyDraft,
    ReselectDraft, RootMessageDraft, SnapshotResourceInput, SourceAnchorInput, SourceSnapshotInput,
};
use sha2::{Digest, Sha256};

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
        presentation_json: r#"{"schema":1,"theme":"paper","fontSize":32}"#.into(),
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

    let reply = store
        .reply(ReplyDraft {
            conversation_id: first.conversation_id.clone(),
            reply_to_message_id: first.message_id.clone(),
            text: "把两处内容联系起来".into(),
            reference_ids: vec![second.message_id.clone()],
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

    assert_eq!(conversation.messages.len(), 2);
    assert_eq!(
        conversation.messages[1].reply_to_message_id,
        Some(first.message_id)
    );
    assert_eq!(
        conversation.messages[1].reference_ids,
        vec![second.message_id.clone()]
    );
    assert_eq!(outgoing.references, vec![second.message_id.clone()]);
    assert_eq!(incoming.referenced_by, vec![reply.message_id]);
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
fn outbox_failure_rolls_back_the_message_fact() {
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

    assert_eq!(
        store.create_root(RootMessageDraft {
            edition: edition(),
            anchor: failed_anchor,
            snapshot: snapshot_for("事务回滚原文"),
            text: Some("不能留下来的笔记".into()),
        }),
        Err(atha_backend::messages::MessageError::Database)
    );
    let connection = rusqlite::Connection::open(database).expect("reopen fault injector");
    connection
        .execute_batch("DROP TRIGGER fail_message_outbox;")
        .expect("remove outbox fault");
    let matches = store
        .search(MessageSearch {
            edition_id: edition().content_version,
            text: "不能留下来的笔记".into(),
            section: None,
        })
        .expect("search rolled back fact");

    assert!(matches.is_empty());
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
    store
        .reply(ReplyDraft {
            conversation_id: first.conversation_id.clone(),
            reply_to_message_id: first.message_id.clone(),
            text: "引用另一条标注".into(),
            reference_ids: vec![second.message_id],
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
    assert_eq!(inspected.messages, 4);
    assert_eq!(inspected.revisions, 5);
    assert_eq!(inspected.sources, 3);
    assert_eq!(inspected.snapshots, 3);
    assert_eq!(inspected.relationships, 1);
    assert_eq!(inspected.resources, 1);
    assert_eq!(conversation_inspected.conversations, 2);
    assert_eq!(conversation_inspected.messages, 3);
    assert_eq!(conversation_inspected.relationships, 1);
    assert_eq!(conversation_inspected.resources, 1);
    assert!(
        !String::from_utf8_lossy(&fs::read(archive).expect("read export"))
            .contains(root.0.to_string_lossy().as_ref())
    );
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
            reference_ids: Vec::new(),
        })
        .expect("create reply");

    let all = store
        .roots(&edition().content_version, None)
        .expect("list roots");
    let section = store
        .roots(&edition().content_version, Some("section-2"))
        .expect("filter roots");

    assert_eq!(all.len(), 2);
    assert_eq!(all[0].conversation_id, first.conversation_id);
    assert_eq!(section.len(), 1);
    assert_eq!(section[0].kind, "source-only");
    assert_eq!(section[0].source.selected_text, "第二章原文");
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
