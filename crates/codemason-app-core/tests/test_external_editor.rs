//! Tests for `/editor` (`codemason_app_core::external_editor`): running an
//! editor command on the prompt file, its failures, and the App locking the
//! prompt while the editor runs and taking the result back.
#![cfg(unix)]

use codemason_app_core::external_editor::edit_in_external_editor;

mod support;

/// A fresh directory under the git-ignored `target/temp/`.
fn scratch() -> tempfile::TempDir {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("mkdir target/temp");
    tempfile::Builder::new()
        .prefix("external-editor-")
        .tempdir_in(root)
        .expect("tempdir")
}

#[test]
fn test_external_editor_returns_what_the_editor_saved() {
    let dir = scratch();
    let edited = edit_in_external_editor("draft", r#"sh -c 'printf edited > "$0"'"#, dir.path())
        .expect("edit");
    assert_eq!(edited, "edited");
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("read dir").count(),
        0,
        "the prompt file is removed"
    );
}

#[test]
fn test_external_editor_keeps_the_text_and_drops_one_trailing_newline() {
    let dir = scratch();
    let edited = edit_in_external_editor("line one\nline two", "true", dir.path()).expect("edit");
    assert_eq!(edited, "line one\nline two");
    let edited =
        edit_in_external_editor("x", r#"sh -c 'printf "y\n" > "$0"'"#, dir.path()).expect("edit");
    assert_eq!(edited, "y");
}

#[test]
fn test_external_editor_failure_is_an_error() {
    let dir = scratch();
    let err = edit_in_external_editor("draft", "false", dir.path()).expect_err("fails");
    assert!(err.to_string().contains("exited with"), "{err}");
}

#[test]
fn test_external_editor_app_locks_the_prompt_until_it_finishes() {
    let mut app = support::make_app();
    app.session.input = "draft".to_string();
    assert_eq!(app.begin_external_edit().as_deref(), Some("draft"));
    assert!(
        app.is_input_locked(),
        "the prompt is locked while the editor runs"
    );
    assert_eq!(app.begin_external_edit(), None, "one editor at a time");

    app.finish_external_edit(Ok("edited text".to_string()));
    assert!(!app.is_input_locked());
    assert_eq!(app.session.input, "edited text");
    assert_eq!(app.session.input_cursor, "edited text".chars().count());
}

#[test]
fn test_external_editor_app_failure_keeps_the_prompt() {
    let mut app = support::make_app();
    app.session.input = "draft".to_string();
    let _ = app.begin_external_edit();
    app.finish_external_edit(Err(anyhow::anyhow!("editor 'vim' exited with 1")));
    assert!(!app.is_input_locked());
    assert_eq!(app.session.input, "draft");
    assert!(app.status.starts_with("editor: "), "{}", app.status);
}

#[tokio::test]
async fn test_external_editor_slash_editor_asks_the_frontend() {
    let mut app = support::make_app();
    app.execute_slash_command("/editor").await;
    assert_eq!(app.take_frontend_action().as_deref(), Some("prompt.editor"));
}
