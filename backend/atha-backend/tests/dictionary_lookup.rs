use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::dictionary::{DictionaryError, LocalDictionaries};
use mdict_rs::{MddFile, MdxFile};

#[test]
fn invalid_mdict_never_registers_a_dictionary() {
    let root = test_root("invalid");
    let source = root.join("invalid.mdx");
    fs::create_dir_all(&root).expect("create test root");
    fs::write(&source, b"not an mdict file").expect("write invalid source");
    let dictionaries = LocalDictionaries::open(&root).expect("open dictionaries");

    assert_eq!(
        dictionaries.import_mdict(&source, &[]),
        Err(DictionaryError::CorruptSource)
    );
    assert!(dictionaries.list().expect("list dictionaries").is_empty());

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn malformed_kindle_record_tables_never_register_a_dictionary() {
    let root = test_root("invalid-kindle");
    fs::create_dir_all(&root).expect("create test root");
    let source = root.join("invalid.mobi");
    let dictionaries = LocalDictionaries::open(&root).expect("open dictionaries");

    let mut duplicate_offsets = vec![0_u8; 110];
    duplicate_offsets[60..68].copy_from_slice(b"BOOKMOBI");
    duplicate_offsets[76..78].copy_from_slice(&2_u16.to_be_bytes());
    duplicate_offsets[78..82].copy_from_slice(&94_u32.to_be_bytes());
    duplicate_offsets[86..90].copy_from_slice(&94_u32.to_be_bytes());
    fs::write(&source, duplicate_offsets).expect("write duplicate record table");
    assert_eq!(
        dictionaries.import_kindle(&source),
        Err(DictionaryError::CorruptSource)
    );

    let mut truncated_table = vec![0_u8; 94];
    truncated_table[60..68].copy_from_slice(b"BOOKMOBI");
    truncated_table[76..78].copy_from_slice(&100_u16.to_be_bytes());
    fs::write(&source, truncated_table).expect("write truncated record table");
    assert_eq!(
        dictionaries.import_kindle(&source),
        Err(DictionaryError::CorruptSource)
    );
    assert!(dictionaries.list().expect("list dictionaries").is_empty());

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn private_mdict_sample_imports_and_looks_up_without_content_artifacts() {
    let Some(fixture_root) = env::var_os("ATHA_PRIVATE_DICTIONARY_ROOT").map(PathBuf::from) else {
        return;
    };
    let (mdx_path, mdd_paths) = private_mdict_files(&fixture_root);
    let source = MdxFile::open(&mdx_path).expect("open private MDX source");
    let queries = [0, source.len() / 2, source.len().saturating_sub(1)].map(|ordinal| {
        source
            .key_at(ordinal.into())
            .expect("read key ordinal")
            .expect("key exists")
    });
    let root = test_root("private-mdict");
    fs::create_dir_all(&root).expect("create test root");
    let dictionaries = LocalDictionaries::open(&root).expect("open dictionaries");

    let imported = dictionaries
        .import_mdict(&mdx_path, &mdd_paths)
        .expect("import private MDict sample");
    assert_eq!(imported.entry_count, source.len());
    assert_eq!(imported.resource_count, mdd_paths.len());
    assert_eq!(
        dictionaries
            .import_mdict(&mdx_path, &mdd_paths)
            .expect("repeat private import")
            .id,
        imported.id
    );
    for query in queries {
        let result = dictionaries
            .lookup(&imported.id, &query)
            .expect("lookup private MDict sample");
        assert!(
            result.is_some(),
            "ordinal-derived exact lookup must resolve"
        );
    }
    assert!(
        dictionaries
            .lookup(&imported.id, "atha-private-dictionary-missing-entry")
            .expect("lookup missing entry")
            .is_none()
    );
    if let Some(mdd_path) = mdd_paths.first() {
        let source = MddFile::open(mdd_path).expect("open private MDD source");
        let key = source
            .key_at((source.len() / 2).into())
            .expect("read resource ordinal")
            .expect("resource key exists");
        assert!(
            dictionaries
                .resource(&imported.id, &key)
                .expect("read private MDD resource")
                .is_some(),
            "ordinal-derived resource lookup must resolve"
        );
    }
    dictionaries
        .remove(&imported.id)
        .expect("remove dictionary");
    assert!(dictionaries.list().expect("list after remove").is_empty());

    fs::remove_dir_all(root).expect("remove test root");
}

fn private_mdict_files(root: &Path) -> (PathBuf, Vec<PathBuf>) {
    let mut pending = vec![root.to_path_buf()];
    let mut mdx = Vec::new();
    let mut mdd = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("read private fixture directory") {
            let path = entry.expect("read private fixture entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case("mdx"))
            {
                mdx.push(path);
            } else if path
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case("mdd"))
            {
                mdd.push(path);
            }
        }
    }
    assert_eq!(mdx.len(), 1, "private root must contain exactly one MDX");
    mdd.sort();
    (mdx.pop().expect("one MDX"), mdd)
}

fn test_root(label: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".tmp")
        .join(format!(
            "dictionary-test-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ))
}
