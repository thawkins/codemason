//! AA-03: corrupt-mailbox recovery must move the file aside while holding the
//! mailbox's exclusive lock, and every recovery must get its own aside file.
//!
//! Before the fix `read_all` / `peek_unread` / `drain_unread` dropped the lock
//! and then renamed the mailbox, so a concurrent `push` (JSONL append) could
//! return `Ok` while its line landed only in the moved-aside file; two
//! recoveries in the same second also overwrote each other's aside file.
//!
//! The lock interleaving is made deterministic by holding a *shared* flock on
//! the companion `.lock` file from the test: the readers' own shared lock is
//! compatible with it, so the read proceeds, but the move-aside (which must
//! take the exclusive lock) cannot happen until the test releases it.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use codemason_agent::team::{Mailbox, MailboxMessage, MessageType};

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_workspace;

/// Write a corrupt mailbox for `tm-001` under `team_dir` and return its path.
fn write_corrupt_mailbox(team_dir: &Path) -> PathBuf {
    let path = team_dir.join("mailbox").join("tm-001.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ this is not valid json }\n").unwrap();
    path
}

/// Hold a shared flock on the mailbox's companion lock file.
fn hold_shared_lock(mailbox_path: &Path) -> File {
    let mut name = mailbox_path.as_os_str().to_owned();
    name.push(".lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(PathBuf::from(name))
        .unwrap();
    file.lock_shared().unwrap();
    file
}

/// Names of the `*.corrupt.*` aside files in the mailbox directory.
fn aside_files(mailbox_path: &Path) -> Vec<String> {
    std::fs::read_dir(mailbox_path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".corrupt."))
        .collect()
}

/// Run `op` (a recovering read) on a thread while the test holds a shared
/// lock; assert the mailbox is not moved aside until the lock is released.
fn assert_move_aside_waits_for_lock(op: fn(&Mailbox)) {
    let (_tmp, dir) = setup_workspace();
    let team_dir = dir.join("aa03-team");
    let path = write_corrupt_mailbox(&team_dir);
    let guard = hold_shared_lock(&path);

    let (tx, rx) = mpsc::channel();
    let thread_dir = team_dir.clone();
    let handle = std::thread::spawn(move || {
        let mailbox = Mailbox::open(&thread_dir, "tm-001").unwrap();
        op(&mailbox);
        tx.send(()).unwrap();
    });

    let finished_while_locked = rx.recv_timeout(Duration::from_millis(1500)).is_ok();
    assert!(
        !finished_while_locked && path.exists(),
        "corrupt mailbox was moved aside without the exclusive lock \
         (finished_while_locked={finished_while_locked}, live_exists={})",
        path.exists()
    );

    drop(guard);
    handle.join().unwrap();
    assert!(!path.exists(), "corrupt mailbox should be moved aside");
    assert_eq!(aside_files(&path).len(), 1);

    // A push after recovery lands in the live mailbox and is readable.
    let mailbox = Mailbox::open(&team_dir, "tm-001").unwrap();
    mailbox
        .push(MailboxMessage::new(
            "tm-002",
            "tm-001",
            MessageType::Message,
            "after",
        ))
        .unwrap();
    let msgs = mailbox.read_all().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].content, "after");
}

#[test]
fn test_mailbox_corrupt_read_all_moves_aside_under_exclusive_lock() {
    assert_move_aside_waits_for_lock(|m| {
        assert!(m.read_all().unwrap().is_empty());
    });
}

#[test]
fn test_mailbox_corrupt_peek_unread_moves_aside_under_exclusive_lock() {
    assert_move_aside_waits_for_lock(|m| {
        assert!(m.peek_unread().unwrap().is_empty());
    });
}

#[test]
fn test_mailbox_corrupt_drain_unread_moves_aside_under_exclusive_lock() {
    assert_move_aside_waits_for_lock(|m| {
        assert!(m.drain_unread().unwrap().is_empty());
    });
}

/// Two recoveries in quick succession (same second) must not overwrite each
/// other's aside file.
#[test]
fn test_mailbox_corrupt_recoveries_get_unique_aside_names() {
    let (_tmp, dir) = setup_workspace();
    let team_dir = dir.join("aa03-unique");
    let mailbox = Mailbox::open(&team_dir, "tm-001").unwrap();

    let path = write_corrupt_mailbox(&team_dir);
    assert!(mailbox.read_all().unwrap().is_empty());
    std::fs::write(&path, "[still not json").unwrap();
    assert!(mailbox.peek_unread().unwrap().is_empty());
    std::fs::write(&path, "{ again not json }\n").unwrap();
    assert!(mailbox.drain_unread().unwrap().is_empty());

    let asides = aside_files(&path);
    assert_eq!(
        asides.len(),
        3,
        "each recovery keeps its own file: {asides:?}"
    );
}

/// What the move-aside probe saw: 0 = not run, 1 = lock held (correct),
/// 2 = lock free at the rename (the message-loss race).
static PROBE_SAW: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Runs just before the rename: a writer must not be able to take the lock.
fn probe_lock_is_held(lock_path: &Path) {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(lock_path)
        .unwrap();
    let saw = match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            2
        }
        Err(_) => 1,
    };
    PROBE_SAW.store(saw, std::sync::atomic::Ordering::SeqCst);
}

/// Review of AA-03: blocking the initial lock does not prove the lock is
/// still held at the rename. The probe runs at the rename itself and must
/// find the lock taken, for every recovering entry point.
#[test]
fn test_mailbox_corrupt_rename_happens_while_the_lock_is_held() {
    let _env = crate::global_env_lock();
    codemason_agent::team::mailbox::set_move_aside_probe(Some(probe_lock_is_held));
    let ops: [(&str, fn(&Mailbox)); 3] = [
        ("read_all", |m| assert!(m.read_all().unwrap().is_empty())),
        ("peek_unread", |m| {
            assert!(m.peek_unread().unwrap().is_empty())
        }),
        ("drain_unread", |m| {
            assert!(m.drain_unread().unwrap().is_empty())
        }),
    ];
    for (name, op) in ops {
        let (_tmp, dir) = setup_workspace();
        let team_dir = dir.join("aa03-probe");
        let path = write_corrupt_mailbox(&team_dir);
        PROBE_SAW.store(0, std::sync::atomic::Ordering::SeqCst);
        op(&Mailbox::open(&team_dir, "tm-001").unwrap());
        assert!(!path.exists(), "{name}: the corrupt mailbox is moved aside");
        assert_eq!(
            PROBE_SAW.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "{name}: the mailbox lock must still be held at the rename"
        );
    }
    codemason_agent::team::mailbox::set_move_aside_probe(None);
}
