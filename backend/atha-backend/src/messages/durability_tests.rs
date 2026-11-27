//! Isolated I/O-failure tests; these do not simulate loss of device power.
use super::*;
use crate::messages::{
    EditionInput, ReselectDraft, RootMessageDraft, SourceAnchorInput, SourceSnapshotInput,
};
use std::{cell::RefCell, io, time::SystemTime};

struct TestRoot(PathBuf);
impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-data")
            .join(format!("asset-durability-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn draft(resource: Option<u8>) -> RootMessageDraft {
    let version = "11".repeat(32);
    RootMessageDraft {
        edition: EditionInput { content_version: version.clone(), title: "Public durability fixture".into(), authors: vec![] },
        anchor: SourceAnchorInput {
            canonical_locator: serde_json::json!({
                "schema": 1, "contentVersion": version,
                "start": {"section": "section-1", "offset": 0},
                "end": {"section": "section-1", "offset": 6}
            }).to_string(),
            section: "section-1".into(), selected_text: "source".into(),
            prefix_text: String::new(), suffix_text: String::new(),
            content_hash: encode_hex(&Sha256::digest(b"source")),
        },
        snapshot: SourceSnapshotInput {
            fragment_html: if resource.is_some() { "<p>source<img src=\"asset.png\"></p>" } else { "<p>source</p>" }.into(),
            reader_css: String::new(), book_css: String::new(), user_css: String::new(),
            presentation_json: r#"{"schema":1,"theme":"paper","brightness":100,"fontSize":19,"fontFamily":"book","density":"standard"}"#.into(),
            resources: resource.into_iter().map(|value| SnapshotResourceInput {
                path: "asset.png".into(), media_type: "image/png".into(), bytes: vec![value; 32],
            }).collect(),
        },
        text: Some("Public note".into()),
    }
}
// A separate connection observes committed facts, not the active writer's changes.
fn facts(database: &Path) -> (Vec<i64>, String) {
    let connection = Connection::open(database).unwrap();
    let counts = [
        "work",
        "edition",
        "conversation",
        "message",
        "message_revision",
        "source_anchor",
        "source_snapshot",
        "snapshot_resource",
        "outbox_event",
        "message_search",
    ]
    .into_iter()
    .map(|table| {
        connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    })
    .collect();
    let identities = connection.query_row(
        "SELECT COALESCE(group_concat(identity), '') FROM (SELECT hex(id) || ':' || hex(current_revision_id) || ':' || hex(current_source_anchor_id) AS identity FROM message ORDER BY id)",
        [], |row| row.get(0)).unwrap();
    (counts, identities)
}
struct SyncProbe {
    assets: PathBuf,
    database: PathBuf,
    before: (Vec<i64>, String),
    bytes: Vec<u8>,
    fail: bool,
    calls: usize,
}
thread_local! { static PROBE: RefCell<Option<SyncProbe>> = const { RefCell::new(None) }; }
struct ProbeGuard;
impl ProbeGuard {
    fn install(store: &MessageStore, value: u8, fail: bool) -> Self {
        PROBE.with(|slot| {
            assert!(slot.borrow().is_none());
            *slot.borrow_mut() = Some(SyncProbe {
                assets: store.assets.clone(),
                database: store.database.clone(),
                before: facts(&store.database),
                bytes: vec![value; 32],
                fail,
                calls: 0,
            });
        });
        Self
    }
    fn calls(&self) -> usize {
        PROBE.with(|slot| slot.borrow().as_ref().unwrap().calls)
    }
}
impl Drop for ProbeGuard {
    fn drop(&mut self) {
        PROBE.with(|slot| *slot.borrow_mut() = None);
    }
}
// Only Unix unit-test builds call this path-scoped, thread-local fault seam.
// No environment flag or failure-injection API is compiled into the product.
pub(super) fn before_directory_sync(path: &Path) -> io::Result<()> {
    PROBE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(probe) = slot.as_mut().filter(|probe| probe.assets == path) else {
            return Ok(());
        };
        probe.calls += 1;
        let name = encode_hex(&Sha256::digest(&probe.bytes));
        assert_eq!(
            fs::read(path.join(name)).unwrap(),
            probe.bytes,
            "asset must already be published"
        );
        assert_eq!(
            facts(&probe.database),
            probe.before,
            "directory sync must precede the database commit"
        );
        if probe.fail {
            Err(io::Error::other("injected directory sync failure"))
        } else {
            Ok(())
        }
    })
}
#[test]
fn failed_create_and_same_process_retry_cannot_skip_directory_sync() {
    let root = TestRoot::new();
    let store = MessageStore::open(&root.0).unwrap();
    let before = facts(&store.database);
    let probe = ProbeGuard::install(&store, 1, true);
    for _ in 0..2 {
        assert_eq!(
            store.create_root(draft(Some(1))).unwrap_err(),
            MessageError::InvalidRoot
        );
        assert_eq!(facts(&store.database), before);
    }
    assert_eq!(
        probe.calls(),
        2,
        "reusing a failed publication must retry the sync"
    );
    drop(probe);
    let probe = ProbeGuard::install(&store, 1, false);
    store.create_root(draft(Some(1))).unwrap();
    assert_eq!(probe.calls(), 1);
    drop(probe);
    assert!(
        MessageStore::open(&root.0)
            .unwrap()
            .health()
            .unwrap()
            .integrity
    );
}
#[test]
fn new_and_previously_committed_assets_are_synced_before_new_references() {
    let root = TestRoot::new();
    let store = MessageStore::open(&root.0).unwrap();
    for _ in 0..2 {
        let probe = ProbeGuard::install(&store, 2, false);
        store.create_root(draft(Some(2))).unwrap();
        assert_eq!(probe.calls(), 1);
    }
    assert!(store.health().unwrap().integrity);
}
#[test]
fn failed_reselect_preserves_original_capture_and_reopen_cleans_orphan() {
    let root = TestRoot::new();
    let store = MessageStore::open(&root.0).unwrap();
    let created = store.create_root(draft(Some(3))).unwrap();
    let source = store
        .roots(&"11".repeat(32), None)
        .unwrap()
        .remove(0)
        .source
        .id;
    let before = facts(&store.database);
    let replacement = draft(Some(4));
    let probe = ProbeGuard::install(&store, 4, true);
    assert_eq!(
        store
            .reselect(ReselectDraft {
                message_id: created.message_id,
                expected_source_id: source,
                anchor: replacement.anchor,
                snapshot: replacement.snapshot
            })
            .unwrap_err(),
        MessageError::InvalidRoot
    );
    assert_eq!(probe.calls(), 1);
    assert_eq!(facts(&store.database), before);
    drop(probe);
    let reopened = MessageStore::open(&root.0).unwrap();
    assert!(reopened.health().unwrap().integrity);
    assert!(
        !reopened
            .assets
            .join(encode_hex(&Sha256::digest([4; 32])))
            .exists()
    );
    assert!(
        reopened
            .assets
            .join(encode_hex(&Sha256::digest([3; 32])))
            .exists()
    );
}
#[test]
fn failed_restore_keeps_current_database_and_retries_existing_asset() {
    let root = TestRoot::new();
    let source = MessageStore::open(root.0.join("source")).unwrap();
    source.create_root(draft(Some(5))).unwrap();
    let backup = root.0.join("fixture.atha-backup");
    source.create_backup(&backup).unwrap();
    let destination = MessageStore::open(root.0.join("destination")).unwrap();
    destination.create_root(draft(Some(6))).unwrap();
    let before = facts(&destination.database);
    let probe = ProbeGuard::install(&destination, 5, true);
    for _ in 0..2 {
        assert_eq!(
            destination.restore_backup(&backup).unwrap_err(),
            MessageError::Restore
        );
        assert_eq!(facts(&destination.database), before);
        assert!(destination.health().unwrap().integrity);
    }
    assert_eq!(probe.calls(), 2);
    drop(probe);
    let probe = ProbeGuard::install(&destination, 5, false);
    destination.restore_backup(&backup).unwrap();
    assert_eq!(probe.calls(), 1);
    drop(probe);
    assert_eq!(facts(&destination.database), facts(&source.database));
    assert!(destination.health().unwrap().integrity);
}
#[test]
fn source_only_message_needs_no_asset_directory_sync() {
    let root = TestRoot::new();
    let store = MessageStore::open(&root.0).unwrap();
    let probe = ProbeGuard::install(&store, 7, true);
    store.create_root(draft(None)).unwrap();
    assert_eq!(probe.calls(), 0);
}
