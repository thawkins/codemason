//! Tests for the inline diff under an edit and the Diff pane: Open diff,
//! hunk and file keys, split layout and the reviewed mark.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::diff_view::DiffLayout;
use codemason_app_core::transcript_prefs::Verbosity;
use codemason_gui::Shell;
use codemason_gui::panels::diff::{
    DIFF_PANE_LABEL, MARK_REVIEWED, NEW_COLUMN, OLD_COLUMN, OPEN_DIFF, REVIEWED_NOTE,
};
use egui::accesskit::Role as A11yRole;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Twenty numbered lines.
fn twenty() -> String {
    (1..=20).map(|i| format!("line {i}\n")).collect()
}

/// An assistant message with a finished Edit of `a.rs` that changes lines 2
/// and 18 (two hunks), and a Write of `b.txt`.
fn edits() -> Message {
    let old = twenty();
    let new = old
        .replace("line 2\n", "line two\n")
        .replace("line 18\n", "line eighteen\n");
    let call = |id: &str, tool: &str, input: serde_json::Value| MessagePart::ToolCall {
        tool: tool.to_string(),
        call_id: id.to_string(),
        state: Box::new(ToolCallState {
            status: ToolCallStatus::Completed,
            input,
            output: None,
            error: None,
            duration_ms: None,
        }),
    };
    Message::new(
        "s1",
        Role::Assistant,
        vec![
            call(
                "c1",
                "Edit",
                serde_json::json!({"file_path": "a.rs", "old_string": old, "new_string": new}),
            ),
            call(
                "c2",
                "Write",
                serde_json::json!({"path": "b.txt", "content": "new file\n"}),
            ),
        ],
    )
}

/// Whether the Diff pane marks the diff of `a.rs` reviewed.
fn reviewed_a(harness: &Harness<'static, Shell>) -> bool {
    let pane = &harness.state().app().diff_pane;
    pane.files
        .iter()
        .find(|f| f.path == "a.rs")
        .is_some_and(|f| pane.is_reviewed(f))
}

fn shell() -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    // Each call is its own block, so the diffs are drawn without opening
    // an activity row.
    app.transcript_prefs.verbosity = Verbosity::High;
    app.session.messages = vec![edits()];
    let harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 1000.0))
        .with_max_steps(64)
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), Shell::new(app));
    (runtime, harness)
}

/// Click the Diff pane so it takes the keyboard, then press `key`.
fn press_in_pane(harness: &mut Harness<'static, Shell>, key: egui::Key) {
    harness
        .get_by_role_and_label(A11yRole::Pane, DIFF_PANE_LABEL)
        .click();
    harness.run();
    harness.key_press(key);
    harness.run();
}

#[test]
fn test_inline_diff_shows_changed_lines_and_open_diff() {
    let (_rt, mut harness) = shell();
    harness.run();
    assert_eq!(harness.query_all_by_label(OPEN_DIFF).count(), 2);
    harness.get_by_label_contains("- line 2");
    harness.get_by_label_contains("+ line two");
    harness.get_by_label("1 more hunk");
    harness.get_by_label_contains("+ new file");
}

#[test]
fn test_open_diff_shows_the_pane_and_keys_move_through_it() {
    let (_rt, mut harness) = shell();
    harness.run();
    let first = harness
        .query_all_by_label(OPEN_DIFF)
        .next()
        .expect("an Open diff button");
    first.click();
    harness.run();
    {
        let pane = &harness.state().app().diff_pane;
        assert!(pane.shown);
        assert_eq!(pane.files.len(), 2);
        assert_eq!(pane.selected, 0);
        assert_eq!(pane.hunk, 0);
    }
    harness.get_by_role_and_label(A11yRole::Pane, DIFF_PANE_LABEL);

    press_in_pane(&mut harness, egui::Key::CloseBracket);
    assert_eq!(harness.state().app().diff_pane.hunk, 1);
    press_in_pane(&mut harness, egui::Key::OpenBracket);
    assert_eq!(harness.state().app().diff_pane.hunk, 0);
    press_in_pane(&mut harness, egui::Key::N);
    assert_eq!(harness.state().app().diff_pane.selected, 1);
    press_in_pane(&mut harness, egui::Key::P);
    assert_eq!(harness.state().app().diff_pane.selected, 0);
    // The keys did not type into the prompt.
    assert_eq!(harness.state().app().session.input, "");

    press_in_pane(&mut harness, egui::Key::V);
    assert_eq!(harness.state().app().diff_pane.layout, DiffLayout::Split);
    // One pair of columns per hunk.
    assert_eq!(
        harness
            .query_all_by_role_and_label(A11yRole::Group, OLD_COLUMN)
            .count(),
        2
    );
    assert_eq!(
        harness
            .query_all_by_role_and_label(A11yRole::Group, NEW_COLUMN)
            .count(),
        2
    );
}

#[test]
fn test_reviewed_mark_collapses_the_file_and_survives_reopening() {
    let (_rt, mut harness) = shell();
    harness.run();
    harness
        .query_all_by_label(OPEN_DIFF)
        .next()
        .expect("an Open diff button")
        .click();
    harness.run();
    press_in_pane(&mut harness, egui::Key::M);
    assert!(reviewed_a(&harness));
    harness.get_by_label(REVIEWED_NOTE);

    // Close the pane, then open the diff again: the mark is kept.
    harness.state_mut().app_mut().diff_pane.shown = false;
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(A11yRole::Pane, DIFF_PANE_LABEL)
            .is_none()
    );
    harness
        .query_all_by_label(OPEN_DIFF)
        .next()
        .expect("an Open diff button")
        .click();
    harness.run();
    assert!(reviewed_a(&harness));
    harness.get_by_label(REVIEWED_NOTE);
    harness.get_by_role_and_label(A11yRole::Button, MARK_REVIEWED);
}
