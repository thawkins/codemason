//! The per-install credential key file (audit CO-02).
//!
//! Every stored credential is encrypted under this key, so it must be minted
//! exactly once: two processes starting together must end up with the same
//! key, and an existing key file must never be replaced.

use codemason_storage::storage::load_or_create_credential_key_in;

/// A fresh scratch directory under the git-ignored `target/temp`.
fn scratch(name: &str) -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("scratch root");
    tempfile::Builder::new()
        .prefix(name)
        .tempdir_in(root)
        .expect("scratch dir")
}

#[test]
fn test_credential_key_file_concurrent_first_use_agrees_on_one_key() {
    let dir = scratch("credkey-race-");
    let threads = 16;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(threads));
    let keys: Vec<Option<[u8; 32]>> = (0..threads)
        .map(|_| {
            let barrier = std::sync::Arc::clone(&barrier);
            let dir = dir.path().to_path_buf();
            std::thread::spawn(move || {
                barrier.wait();
                load_or_create_credential_key_in(&dir)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| handle.join().expect("thread"))
        .collect();

    let first = keys[0].expect("a key is minted");
    assert!(
        keys.iter().all(|key| *key == Some(first)),
        "every caller sees the one key that was published"
    );
    let on_disk = std::fs::read(dir.path().join("credential.key")).expect("key file");
    assert_eq!(on_disk, first, "the published key is the one in use");
}

#[test]
fn test_credential_key_file_existing_key_is_reused() {
    let dir = scratch("credkey-reuse-");
    let first = load_or_create_credential_key_in(dir.path()).expect("minted");
    let second = load_or_create_credential_key_in(dir.path()).expect("loaded");
    assert_eq!(first, second);
}

#[test]
fn test_credential_key_file_damaged_key_is_not_replaced() {
    // A key file of the wrong length (a crash mid-write, or damage) may still
    // be recoverable; replacing it would orphan every stored credential.
    let dir = scratch("credkey-damaged-");
    let path = dir.path().join("credential.key");
    std::fs::write(&path, b"short").expect("damaged key");

    assert_eq!(load_or_create_credential_key_in(dir.path()), None);
    assert_eq!(std::fs::read(&path).expect("key file"), b"short");
}
