use std::{fs, path::PathBuf, time::SystemTime};

use atha_backend::messages::{
    EditionInput, MessageSearch, MessageStore, ReplyDraft, ReselectDraft, RootMessageDraft,
    SnapshotResourceInput, SourceAnchorInput, SourceSnapshotInput,
};

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

fn anchor() -> SourceAnchorInput {
    SourceAnchorInput {
        canonical_locator: r#"{"schema":1,"contentVersion":"1111111111111111111111111111111111111111111111111111111111111111","start":{"section":"section-1","offset":10},"end":{"section":"section-1","offset":18}}"#.into(),
        section: "section-1".into(),
        selected_text: "算术与几何".into(),
        prefix_text: "第一章".into(),
        suffix_text: "之间".into(),
        content_hash: "22".repeat(32),
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
    second_anchor.selected_text = "有理数".into();
    let second = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: second_anchor,
            snapshot: snapshot(),
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
    second_anchor.selected_text = "代数结构".into();
    let second = store
        .create_root(RootMessageDraft {
            edition: edition(),
            anchor: second_anchor,
            snapshot: snapshot(),
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
    assert_eq!(captures[0].snapshot.fragment_html, "<p>算术与几何</p>");
    assert_eq!(captures[0].snapshot.resources.len(), 1);
    assert_eq!(captures[0].snapshot.resources[0].content_hash.len(), 64);
    assert_eq!(resource.media_type, "image/png");
    assert_eq!(resource.bytes, b"safe local image");
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
    replacement_anchor.selected_text = "代数结构".into();
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
