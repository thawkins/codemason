//! Tests for user-defined slash commands loaded from `commands/*.md`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use codemason_agent::user_command::{
    find_project_commands_dir, load_user_commands_from, render_user_command,
};

/// A fresh scratch directory under the git-ignored `target/temp/` tree.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/user_command")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn write(dir: &std::path::Path, file: &str, content: &str) {
    std::fs::create_dir_all(dir).expect("create dir");
    std::fs::write(dir.join(file), content).expect("write command file");
}

#[test]
fn test_user_command_loads_description_and_body() {
    let dir = scratch("loads");
    write(
        &dir,
        "review.md",
        "---\ndescription: Review a file\n---\nReview $1 carefully.\n",
    );
    write(&dir, "plain.md", "Just a prompt.\n");
    write(&dir, "notes.txt", "not a command");

    let (commands, diagnostics) =
        load_user_commands_from(std::slice::from_ref(&dir), &BTreeSet::new());

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["plain", "review"]);
    assert_eq!(commands[0].description, "");
    assert_eq!(commands[1].description, "Review a file");
    assert_eq!(commands[1].prompt, "Review $1 carefully.\n");
    assert_eq!(commands[1].path, dir.join("review.md"));
}

#[test]
fn test_user_command_later_directory_wins() {
    let global = scratch("precedence_global");
    let project = scratch("precedence_project");
    write(&global, "deploy.md", "global deploy\n");
    write(&global, "only-global.md", "global only\n");
    write(&project, "deploy.md", "project deploy\n");

    let (commands, _) = load_user_commands_from(&[global, project], &BTreeSet::new());

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].name, "deploy");
    assert_eq!(commands[0].prompt, "project deploy\n");
    assert_eq!(commands[1].name, "only-global");
}

#[test]
fn test_user_command_skips_collisions_bad_names_and_empty_bodies() {
    let dir = scratch("skips");
    write(&dir, "help.md", "shadow the built-in\n");
    write(&dir, "Bad Name.md", "bad\n");
    write(&dir, "empty.md", "---\ndescription: nothing\n---\n\n");
    write(&dir, "ok.md", "fine\n");
    let existing: BTreeSet<String> = ["help".to_string()].into();

    let (commands, diagnostics) = load_user_commands_from(&[dir], &existing);

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "ok");
    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert!(diagnostics.iter().any(|d| d.contains("already a built-in")));
    assert!(diagnostics.iter().any(|d| d.contains("command name must")));
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("prompt body is empty"))
    );
}

#[test]
fn test_user_command_missing_directory_is_empty() {
    let missing = scratch("missing").join("nope");
    let (commands, diagnostics) = load_user_commands_from(&[missing], &BTreeSet::new());
    assert_eq!(commands.len(), 0);
    assert_eq!(diagnostics, Vec::<String>::new());
}

#[test]
fn test_find_project_commands_dir_walks_up() {
    let root = scratch("walk_up");
    let commands = root.join(".codemason").join("commands");
    let nested = root.join("a").join("b");
    std::fs::create_dir_all(&commands).expect("commands dir");
    std::fs::create_dir_all(&nested).expect("nested dir");

    assert_eq!(find_project_commands_dir(&nested), Some(commands));
}

#[test]
fn test_render_user_command_placeholders() {
    assert_eq!(
        render_user_command(
            "all: $ARGUMENTS; first: $1; second: $2",
            "one \"two words\""
        ),
        "all: one \"two words\"; first: one; second: two words"
    );
    // A missing positional argument renders empty.
    assert_eq!(render_user_command("[$3]", "a b"), "[]");
    // `$0`, a lone `$` and a non-placeholder `$name` are left as written.
    assert_eq!(
        render_user_command("$0 costs $5.00 or $ and $HOME", "a b c d e"),
        "$0 costs e.00 or $ and $HOME"
    );
    // Substituted text is not scanned again.
    assert_eq!(render_user_command("$1", "$ARGUMENTS"), "$ARGUMENTS");
}
