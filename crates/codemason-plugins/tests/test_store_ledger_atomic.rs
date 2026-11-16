//! The `_state.json` ledger is written atomically (temp file plus rename),
//! never through `fs::write` (deep-refactor SI-09).

#![cfg(unix)]

use std::path::PathBuf;

use codemason_plugins::{STATE_FILE, StoreLedger};

/// A fresh scratch directory under the git-ignored `target/temp`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/plugins-ledger-atomic")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("mkdir scratch");
    dir
}

#[test]
fn test_store_ledger_save_replaces_a_symlink_instead_of_writing_through_it() {
    let dir = scratch("symlink");
    let outside = dir.join("outside.json");
    std::fs::write(&outside, b"untouched").expect("seed outside file");
    let store = dir.join("store");
    std::fs::create_dir_all(&store).expect("mkdir store");
    std::os::unix::fs::symlink(&outside, store.join(STATE_FILE)).expect("symlink ledger");

    StoreLedger::default().save(&store).expect("save ledger");

    assert_eq!(std::fs::read(&outside).expect("read outside"), b"untouched");
    let ledger = store.join(STATE_FILE);
    assert!(
        !ledger
            .symlink_metadata()
            .expect("stat")
            .file_type()
            .is_symlink()
    );
    let leftovers: Vec<_> = std::fs::read_dir(&store)
        .expect("read store")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != STATE_FILE)
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}
