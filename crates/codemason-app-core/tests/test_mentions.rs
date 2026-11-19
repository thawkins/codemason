//! Tests for `@file#range` mentions in the prompt: the `@` file menu offers
//! files while the path is typed and closes once a `#range` follows, so
//! accepting it cannot replace the mention and drop the range.

mod support;

#[test]
fn test_mentions_file_menu_closes_while_a_range_is_typed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("mkdir target/temp");
    let dir = tempfile::Builder::new()
        .prefix("mentions-menu-")
        .tempdir_in(root)
        .expect("tempdir");
    std::fs::write(dir.path().join("alpha.rs"), "fn main() {}\n").expect("write");

    let mut app = support::make_app();
    app.cwd = dir.path().display().to_string();
    app.cwd_path = dir.path().to_path_buf();

    app.session.input = "look @alpha".to_string();
    app.session.input_cursor = app.session.input.chars().count();
    app.update_file_menu();
    assert!(
        app.file_menu
            .as_ref()
            .is_some_and(|m| m.matches.iter().any(|e| e.display == "alpha.rs")),
        "the menu offers the file while the path is typed"
    );

    app.session.input = "look @alpha.rs#2-3".to_string();
    app.session.input_cursor = app.session.input.chars().count();
    app.update_file_menu();
    assert!(app.file_menu.is_none(), "a range closes the menu");
}
