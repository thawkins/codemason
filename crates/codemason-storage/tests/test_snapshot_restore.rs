//! `restore_snapshot` puts a user's files back (audit CO-10).
//!
//! Each file is replaced atomically, so a crash or a full disk mid-restore
//! never leaves a truncated file; the replacement keeps the file's own
//! permissions and is written through a symlink to its target, as an in-place
//! write was.

use codemason_storage::snapshot::{restore_snapshot, take_snapshot};

#[test]
fn test_snapshot_restore_puts_content_back_and_leaves_no_temp_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "before").expect("write");
    let snap = take_snapshot("s", "m", std::slice::from_ref(&file)).expect("snapshot");
    std::fs::write(&file, "after, and longer than before").expect("edit");

    restore_snapshot(&snap).expect("restore");

    assert_eq!(std::fs::read_to_string(&file).expect("read"), "before");
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("notes.txt")]);
}

#[test]
fn test_snapshot_restore_recreates_a_deleted_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("sub").join("gone.txt");
    std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
    std::fs::write(&file, "kept").expect("write");
    let snap = take_snapshot("s", "m", std::slice::from_ref(&file)).expect("snapshot");
    std::fs::remove_dir_all(dir.path().join("sub")).expect("delete");

    restore_snapshot(&snap).expect("restore");

    assert_eq!(std::fs::read_to_string(&file).expect("read"), "kept");
}

#[cfg(unix)]
#[test]
fn test_snapshot_restore_keeps_the_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("run.sh");
    std::fs::write(&file, "#!/bin/sh\n").expect("write");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let snap = take_snapshot("s", "m", std::slice::from_ref(&file)).expect("snapshot");
    std::fs::write(&file, "edited").expect("edit");

    restore_snapshot(&snap).expect("restore");

    let mode = std::fs::metadata(&file).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "the script stays executable");
}

#[cfg(unix)]
#[test]
fn test_snapshot_restore_writes_through_a_symlink() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("real.txt");
    let link = dir.path().join("link.txt");
    std::fs::write(&target, "before").expect("write");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");
    let snap = take_snapshot("s", "m", std::slice::from_ref(&link)).expect("snapshot");
    std::fs::write(&target, "after").expect("edit");

    restore_snapshot(&snap).expect("restore");

    assert!(
        std::fs::symlink_metadata(&link)
            .expect("meta")
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "before");
}
