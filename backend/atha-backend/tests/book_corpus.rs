//! Explicit corpus probe. Normal tests use only self-created public text fixtures.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{library::LocalLibrary, resources::BookRoot};
use serde::{Deserialize, Serialize};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct TestRoot(PathBuf);
impl TestRoot {
    fn new() -> Self {
        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
        fs::create_dir_all(&parent).expect("create test parent");
        assert!(
            !fs::symlink_metadata(&parent)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let parent = match std::env::var_os("ATHA_CORPUS_WORK_ROOT") {
            Some(value) => {
                let chosen = PathBuf::from(value);
                let resolved = fs::canonicalize(&chosen).expect("resolve isolated corpus parent");
                let temporary = fs::canonicalize(&parent).expect("resolve test parent");
                assert!(
                    fs::symlink_metadata(&chosen).unwrap().file_type().is_dir()
                        && resolved.starts_with(&temporary)
                        && resolved != temporary,
                    "corpus parent must be an owned temporary directory"
                );
                resolved
            }
            None => parent,
        };
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!(
            "book-corpus-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&path)
            .expect("reserve isolated corpus directory");
        Self(path)
    }
}
impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
struct Failure {
    phase: &'static str,
    code: &'static str,
}
fn failure(phase: &'static str, code: &'static str) -> Failure {
    Failure { phase, code }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    schema: u8,
    content_version: String,
    sections: Vec<Section>,
    resources: Vec<String>,
    #[serde(default)]
    toc: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
    id: String,
    href: String,
}

#[derive(Debug, Serialize)]
struct Counts {
    sections: usize,
    resources: usize,
}
fn verify_resources(root: &BookRoot, id: &str) -> Result<(Vec<u8>, Counts), Failure> {
    let raw = root
        .read("/.atha-reader.json")
        .map_err(|e| failure("manifest", e.code()))?;
    let manifest: Manifest =
        serde_json::from_slice(&raw.bytes).map_err(|_| failure("manifest", "invalid-manifest"))?;
    if manifest.schema != 1
        || manifest.content_version != id
        || manifest.sections.is_empty()
        || manifest.sections.len() > 2_000
        || manifest.resources.len() > 10_000
        || manifest.toc.len() > 2_000
    {
        return Err(failure("manifest", "invalid-manifest"));
    }
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    for section in &manifest.sections {
        if section.id.is_empty() || !ids.insert(&section.id) || !paths.insert(&section.href) {
            return Err(failure("manifest", "duplicate-section"));
        }
        let entry = root
            .read(&format!("/{}", section.href))
            .map_err(|e| failure("section", e.code()))?;
        if !entry.content_type.starts_with("application/xhtml+xml")
            || entry.bytes.is_empty()
            || std::str::from_utf8(&entry.bytes).is_err()
        {
            return Err(failure("section", "invalid-section"));
        }
    }
    let mut resources = HashSet::new();
    for href in &manifest.resources {
        if !resources.insert(href) {
            return Err(failure("manifest", "duplicate-resource"));
        }
        let entry = root
            .read(&format!("/{href}"))
            .map_err(|e| failure("resource", e.code()))?;
        if entry.bytes.is_empty() {
            return Err(failure("resource", "empty-resource"));
        }
    }
    Ok((
        raw.bytes,
        Counts {
            sections: manifest.sections.len(),
            resources: manifest.resources.len(),
        },
    ))
}

fn probe(source: &Path, data: &Path) -> Result<Counts, Failure> {
    let library = LocalLibrary::open(data).map_err(|e| failure("setup", e.code()))?;
    let staged = library
        .stage_with_title_hint(source, None)
        .map_err(|e| failure("stage", e.code()))?;
    let opened = library
        .open_book(&staged.id)
        .map_err(|e| failure("open", e.code()))?;
    if !opened.book.prepared || opened.book.id != staged.id {
        return Err(failure("open", "identity-mismatch"));
    }
    let (manifest, counts) = verify_resources(&opened.root, &staged.id)?;
    if opened.book.has_cover {
        let cover = library
            .cover(&staged.id)
            .map_err(|e| failure("cover", e.code()))?;
        if cover.bytes.is_empty() {
            return Err(failure("cover", "empty-cover"));
        }
    }
    let repeated = library
        .stage_with_title_hint(source, None)
        .map_err(|e| failure("duplicate", e.code()))?;
    let books = library.list().map_err(|e| failure("duplicate", e.code()))?;
    if repeated.id != staged.id || books.len() != 1 || books[0].id != staged.id {
        return Err(failure("duplicate", "identity-mismatch"));
    }
    drop(opened);
    drop(library);
    let reopened = LocalLibrary::open(data).map_err(|e| failure("reopen", e.code()))?;
    let book = reopened
        .open_book(&staged.id)
        .map_err(|e| failure("reopen", e.code()))?;
    let (reloaded, _) = verify_resources(&book.root, &staged.id)?;
    if reloaded != manifest {
        return Err(failure("reopen", "manifest-changed"));
    }
    Ok(counts)
}

#[test]
fn corpus_probe_round_trips_markdown_and_cjk_text_without_changing_sources() {
    let root = TestRoot::new();
    for (index, extension) in ["md", "txt"].iter().enumerate() {
        let source = root.0.join(format!("书籍 with spaces.{extension}"));
        let bytes =
            "# Public fixture\n\n第一章 起点\n合成正文。\n\n第二章 继续\n这不是用户书籍。\n";
        fs::write(&source, bytes).unwrap();
        let counts = probe(&source, &root.0.join(format!("data-{index}"))).unwrap();
        assert!(counts.sections > 0);
        assert_eq!(fs::read_to_string(source).unwrap(), bytes);
    }
}

#[test]
fn corpus_probe_reports_invalid_source_at_the_actual_stage() {
    let root = TestRoot::new();
    let source = root.0.join("invalid.epub");
    fs::write(&source, b"not an EPUB").unwrap();
    let result = probe(&source, &root.0.join("data")).unwrap_err();
    assert!(matches!(result.phase, "stage" | "open"));
    assert!(!result.code.is_empty());
}

#[test]
fn corpus_probe_rejects_missing_resources_and_empty_sections() {
    let root = TestRoot::new();
    let source = root.0.join("public.md");
    fs::write(&source, "# Fixture\nbody").unwrap();
    let data = root.0.join("data");
    let library = LocalLibrary::open(&data).unwrap();
    let book = library.import(&source).unwrap();
    let opened = library.open_book(&book.id).unwrap();
    let path = data
        .join("ImportedBooks")
        .join(&book.id)
        .join(".atha-reader.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["resources"] = serde_json::json!(["missing.png"]);
    fs::write(&path, manifest.to_string()).unwrap();
    assert_eq!(
        verify_resources(&opened.root, &book.id).unwrap_err().phase,
        "resource"
    );
    manifest["resources"] = serde_json::json!([]);
    manifest["sections"] = serde_json::json!([]);
    fs::write(&path, manifest.to_string()).unwrap();
    assert_eq!(
        verify_resources(&opened.root, &book.id).unwrap_err().phase,
        "manifest"
    );
}

#[test]
#[ignore = "requires one explicit source; use scripts/check-book-corpus.mjs"]
fn private_book_probe() {
    let source = PathBuf::from(
        std::env::var_os("ATHA_CORPUS_SOURCE").expect("explicit corpus source required"),
    );
    let root = TestRoot::new();
    let result = probe(&source, &root.0.join("data"));
    let report = match &result {
        Ok(counts) => {
            serde_json::json!({"schema":1,"status":"passed","phase":"complete","code":"ok","sections":counts.sections,"resources":counts.resources})
        }
        Err(error) => {
            serde_json::json!({"schema":1,"status":"failed","phase":error.phase,"code":error.code,"sections":0,"resources":0})
        }
    };
    println!("book_probe={report}");
    assert!(
        result.is_ok(),
        "book corpus probe failed; see the content-free result"
    );
}
