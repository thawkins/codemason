//! Removing a staged image attachment: the close mark on a thumbnail in the
//! GUI's prompt calls `App::remove_pending_attachment`, so the image is not
//! sent with the next message. The file itself is never deleted.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

fn staged(names: &[&str]) -> (codemason_app_core::app::App, Vec<PathBuf>) {
    let mut app = support::make_app();
    let paths: Vec<PathBuf> = names.iter().map(PathBuf::from).collect();
    app.session.pending_attachments = paths.clone();
    (app, paths)
}

#[test]
fn test_remove_pending_attachment_drops_only_that_image() {
    let (mut app, paths) = staged(&["shots/first.png", "shots/second.png", "shots/third.png"]);

    assert!(
        app.remove_pending_attachment(1),
        "the middle one is removed"
    );

    assert_eq!(
        app.session.pending_attachments,
        vec![paths[0].clone(), paths[2].clone()],
        "the others keep their order"
    );
}

#[test]
fn test_remove_pending_attachment_reports_a_missing_index() {
    let (mut app, paths) = staged(&["shots/only.png"]);

    assert!(
        !app.remove_pending_attachment(1),
        "an index past the end removes nothing"
    );
    assert_eq!(
        app.session.pending_attachments, paths,
        "the list is unchanged"
    );
}

#[test]
fn test_remove_pending_attachment_on_an_empty_list() {
    let (mut app, _) = staged(&[]);

    assert!(!app.remove_pending_attachment(0));
    assert_eq!(app.session.pending_attachments.len(), 0);
}

#[test]
fn test_remove_pending_attachment_removes_one_at_a_time() {
    let (mut app, paths) = staged(&["shots/a.png", "shots/b.png"]);

    assert!(app.remove_pending_attachment(0));
    assert_eq!(app.session.pending_attachments, vec![paths[1].clone()]);
    assert!(app.remove_pending_attachment(0));
    assert_eq!(app.session.pending_attachments.len(), 0);
}

#[test]
fn test_remove_pending_attachment_logs_the_removal() {
    let (mut app, _) = staged(&["shots/a.png"]);
    let before = app.log_entries.len();

    assert!(app.remove_pending_attachment(0));

    let logged =
        app.log_entries.iter().skip(before).any(|entry| {
            entry.message.contains("Image removed") && entry.message.contains("a.png")
        });
    assert!(logged, "the removal is reported in the log");
}

#[test]
fn test_remove_pending_attachment_leaves_the_file_on_disk() {
    // The staged image is the user's own file: removing it from the message
    // must not delete it.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/remove-attachment")
        .join(format!("{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch");
    let file = dir.join("keep.png");
    std::fs::write(&file, b"png").expect("write image");

    let mut app = support::make_app();
    app.session.pending_attachments = vec![file.clone()];

    assert!(app.remove_pending_attachment(0));
    assert!(file.exists(), "the image file is not deleted");
}
