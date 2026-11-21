//! Tests for the inline diff under an edit, the diff overlay Open diff
//! shows, and the Diff pane: hunk and file keys, split layout and the
//! reviewed mark.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::diff_view::DiffLayout;
use codemason_app_core::transcript_prefs::Verbosity;
use codemason_gui::Shell;
use codemason_gui::dialogs::diff_view::{DIFF_VIEW_TITLE, is_open};
use codemason_gui::material::icon_button::DENSE_SIZE;
use codemason_gui::panels::diff::{
    CLOSE_DIFF, DIFF_PANE_LABEL, INLINE_MAX_LINES, MARK_REVIEWED, NEW_COLUMN, NEXT_FILE, NEXT_HUNK,
    NO_DIFF_HINT, NO_DIFF_OPEN, OLD_COLUMN, OPEN_DIFF, PREV_FILE, PREV_HUNK, REVIEWED_NOTE, SPLIT,
    more_lines_label,
};
use egui::accesskit::Role as A11yRole;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

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

/// Press `key` while the diff overlay is open.
fn press(harness: &mut Harness<'static, Shell>, key: egui::Key) {
    harness.key_press(key);
    harness.run();
}

/// Click the transcript's Open diff, which opens the overlay.
fn open_overlay(harness: &mut Harness<'static, Shell>) {
    harness.get_by_label(OPEN_DIFF).click();
    harness.run();
    assert!(is_open(&harness.ctx), "Open diff opens the overlay");
}

/// The overlay's own rectangle.
fn overlay_rect(harness: &Harness<'static, Shell>) -> egui::Rect {
    harness
        .get_by_role_and_label(A11yRole::Dialog, DIFF_VIEW_TITLE)
        .rect()
}

#[test]
fn test_inline_diff_shows_changed_lines_and_open_diff() {
    let (_rt, mut harness) = shell();
    harness.run();
    // Only a.rs runs past the line limit, so only it has a banner and a
    // button; b.txt's one line is shown whole.
    assert_eq!(harness.query_all_by_label(OPEN_DIFF).count(), 1);
    harness.get_by_label_contains("- line 2");
    harness.get_by_label_contains("+ line two");
    harness.get_by_label_contains("+ new file");
    // The second hunk's change is past the limit.
    assert!(harness.query_by_label_contains("+ line eighteen").is_none());
    let pane = &harness.state().app().diff_pane;
    assert!(pane.files.is_empty(), "nothing is opened yet");
}

#[test]
fn test_inline_diff_banner_counts_the_lines_left() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    let total: usize = harness.state().app().diff_pane.files[0]
        .hunks
        .iter()
        .map(|hunk| hunk.lines.len())
        .sum();
    assert!(total > INLINE_MAX_LINES);
    harness.get_by_label(&more_lines_label(total - INLINE_MAX_LINES));
    // The banner and its button share a row.
    let banner = harness
        .get_by_label(&more_lines_label(total - INLINE_MAX_LINES))
        .rect();
    let button = harness.get_by_label(OPEN_DIFF).rect();
    assert!((banner.center().y - button.center().y).abs() < 4.0);
}

#[test]
fn test_open_diff_shows_the_overlay_and_keys_move_through_it() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    {
        let pane = &harness.state().app().diff_pane;
        // The overlay, not the right-hand Diff pane.
        assert!(!pane.shown);
        assert_eq!(pane.files.len(), 2);
        assert_eq!(pane.selected, 0);
        assert_eq!(pane.hunk, 0);
    }
    harness.get_by_role_and_label(A11yRole::Dialog, DIFF_VIEW_TITLE);
    assert!(
        harness
            .query_by_role_and_label(A11yRole::Pane, DIFF_PANE_LABEL)
            .is_none()
    );
    // The whole diff is in the overlay.
    harness.get_by_label_contains("+ line eighteen");

    press(&mut harness, egui::Key::CloseBracket);
    assert_eq!(harness.state().app().diff_pane.hunk, 1);
    press(&mut harness, egui::Key::OpenBracket);
    assert_eq!(harness.state().app().diff_pane.hunk, 0);
    press(&mut harness, egui::Key::N);
    assert_eq!(harness.state().app().diff_pane.selected, 1);
    press(&mut harness, egui::Key::P);
    assert_eq!(harness.state().app().diff_pane.selected, 0);
    // The keys did not type into the prompt.
    assert_eq!(harness.state().app().session.input, "");

    press(&mut harness, egui::Key::V);
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
fn test_diff_pane_keys_act_while_it_has_the_focus() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    harness
        .get_by_role_and_label(A11yRole::Button, CLOSE_DIFF)
        .click();
    harness.run();
    assert!(!is_open(&harness.ctx));
    harness.state_mut().app_mut().diff_pane.shown = true;
    harness.run();
    press_in_pane(&mut harness, egui::Key::CloseBracket);
    assert_eq!(harness.state().app().diff_pane.hunk, 1);
}

#[test]
fn test_reviewed_mark_collapses_the_file_and_survives_reopening() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    press(&mut harness, egui::Key::M);
    assert!(reviewed_a(&harness));
    harness.get_by_label(REVIEWED_NOTE);

    // Close the overlay, then open the diff again: the mark is kept.
    harness
        .get_by_role_and_label(A11yRole::Button, CLOSE_DIFF)
        .click();
    harness.run();
    assert!(!is_open(&harness.ctx));
    open_overlay(&mut harness);
    assert!(reviewed_a(&harness));
    harness.get_by_label(REVIEWED_NOTE);
    harness.get_by_role_and_label(A11yRole::Button, MARK_REVIEWED);
}

#[test]
fn test_close_diff_button_closes_the_overlay() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    // The close control is at the overlay's top right.
    let overlay = overlay_rect(&harness);
    let close = harness
        .get_by_role_and_label(A11yRole::Button, CLOSE_DIFF)
        .rect();
    assert!(close.right() > overlay.center().x && close.top() < overlay.center().y);
    harness
        .get_by_role_and_label(A11yRole::Button, CLOSE_DIFF)
        .click();
    harness.run();
    assert!(!is_open(&harness.ctx));
    assert!(
        harness
            .query_by_role_and_label(A11yRole::Dialog, DIFF_VIEW_TITLE)
            .is_none()
    );
    // The files stay, so opening the diff again shows them.
    assert_eq!(harness.state().app().diff_pane.files.len(), 2);
}

#[test]
fn test_close_diff_button_shows_with_no_file_open() {
    let (_rt, mut harness) = shell();
    harness.run();
    // Shown by app-core with nothing to show: the button still closes it.
    {
        let pane = &mut harness.state_mut().app_mut().diff_pane;
        pane.files.clear();
        pane.shown = true;
    }
    harness.run();
    assert!(harness.state().app().diff_pane.shown);
    let node = harness.get_by_role_and_label(A11yRole::Button, CLOSE_DIFF);
    node.click();
    harness.run();
    assert!(!harness.state().app().diff_pane.shown);
}

/// The pane's own rectangle, for a test that has to know it is narrow: the
/// default layout gives the Diff pane the right column's width, about 270
/// points, not the whole window.
fn pane_rect(harness: &Harness<'static, Shell>) -> egui::Rect {
    harness
        .get_by_role_and_label(A11yRole::Pane, DIFF_PANE_LABEL)
        .rect()
}

/// Every control of the pane's toolbar, which the test then compares.
fn toolbar_height(harness: &Harness<'static, Shell>) -> f32 {
    let mut tops: Vec<f32> = [
        PREV_FILE,
        NEXT_FILE,
        PREV_HUNK,
        NEXT_HUNK,
        SPLIT,
        MARK_REVIEWED,
    ]
    .into_iter()
    .filter_map(|label| harness.query_by_label(label).map(|node| node.rect().top()))
    .collect();
    assert_eq!(tops.len(), 6, "every toolbar button is drawn");
    tops.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    tops.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    assert_eq!(tops.len(), 1, "the toolbar is one row, not {tops:?}");
    tops[0]
}

#[test]
fn test_diff_toolbar_is_one_row_of_icon_buttons() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    let pane = overlay_rect(&harness);
    let row = toolbar_height(&harness);
    // The six controls were six text buttons that wrapped onto two rows in
    // this pane's ~270 points; they are one row of 32-point icon buttons now.
    let mut rights = Vec::new();
    for label in [
        PREV_FILE,
        NEXT_FILE,
        PREV_HUNK,
        NEXT_HUNK,
        SPLIT,
        MARK_REVIEWED,
    ] {
        let node = harness.get_by_label(label);
        let rect = node.rect();
        assert!(
            (rect.height() - DENSE_SIZE).abs() < 0.5,
            "{label}: {rect:?}"
        );
        assert!(rect.left() >= pane.left() && rect.right() <= pane.right());
        rights.push(rect.right());
    }
    // Every one of them starts to the right of the one before it: the row is
    // in reading order, with nothing pushed out of the pane.
    for pair in rights.windows(2) {
        assert!(pair[0] < pair[1], "{rights:?}");
    }
    assert!(row > pane.top() && row < pane.bottom());
}

#[test]
fn test_diff_position_reads_the_hunk_and_file_counts() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    // a.rs changes two hunks, and the set holds two files, so both counts
    // are drawn: the pane used to say only "file 1 of 2", which never showed
    // that `]` had moved to the second hunk of the file.
    harness.get_by_label_contains("hunk 1/2");
    harness.get_by_label_contains("file 1/2");
    press(&mut harness, egui::Key::CloseBracket);
    harness.get_by_label_contains("hunk 2/2");
    // The next file has one hunk of its own, so only its file count shows.
    press(&mut harness, egui::Key::N);
    harness.get_by_label_contains("file 2/2");
    // A one-hunk file has no hunk count to read: "hunk 1/1" would be noise.
    // The toolbar's own buttons are named "Previous hunk" and "Next hunk", so
    // the readout is told apart by its own wording.
    assert!(
        harness.query_by_label_contains("hunk ").is_none(),
        "a one-hunk file has no hunk count to read"
    );
}

#[test]
fn test_diff_step_buttons_are_disabled_at_the_ends() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    let is_disabled = |harness: &Harness<'static, Shell>, label: &str| {
        harness.get_by_label(label).accesskit_node().is_disabled()
    };
    // At the first file and its first hunk, back is dead and forward is not.
    assert!(is_disabled(&harness, PREV_FILE), "first file");
    assert!(is_disabled(&harness, PREV_HUNK), "first hunk");
    assert!(!is_disabled(&harness, NEXT_FILE), "there is a second file");
    assert!(!is_disabled(&harness, NEXT_HUNK), "there is a second hunk");
    // A press on a dead button changes nothing, rather than looking like a
    // press that did nothing.
    harness.get_by_label(PREV_HUNK).click();
    harness.run();
    assert_eq!(harness.state().app().diff_pane.hunk, 0);
    press(&mut harness, egui::Key::N);
    // The last file: forward is dead now.
    assert!(!is_disabled(&harness, PREV_FILE));
    assert!(is_disabled(&harness, NEXT_FILE), "last file");
}

#[test]
fn test_diff_empty_state_does_not_cover_its_close_button() {
    let (_rt, mut harness) = shell();
    harness.run();
    {
        let pane = &mut harness.state_mut().app_mut().diff_pane;
        pane.files.clear();
        pane.shown = true;
    }
    harness.run();
    let pane_rect = pane_rect(&harness);
    let close = harness
        .get_by_role_and_label(A11yRole::Button, CLOSE_DIFF)
        .rect();
    // The empty hint is a long sentence in a ~270-point pane. Drawn beside
    // the header's close button it was laid out past the pane's own edge and
    // covered the button, so the pane could not be closed at all.
    for label in [NO_DIFF_OPEN, NO_DIFF_HINT] {
        let rect = harness.get_by_label(label).rect();
        assert!(
            rect.right() <= pane_rect.right() + 0.5,
            "{label} runs past the pane: {rect:?} in {pane_rect:?}"
        );
        assert!(
            !rect.intersects(close),
            "{label} covers the close button: {rect:?} over {close:?}"
        );
    }
    harness
        .get_by_role_and_label(A11yRole::Button, CLOSE_DIFF)
        .click();
    harness.run();
    assert!(!harness.state().app().diff_pane.shown);
}

/// The height the inline diff of `new` (replacing `old`) takes in a narrow
/// transcript.
fn inline_height(old: &str, new: &str) -> f32 {
    use codemason_gui::palette::Palette;
    use codemason_gui::panels::diff::inline_diff_ui;
    let diff = codemason_app_core::diff_view::file_diff("w.rs", old, new);
    let tokens = codemason_app_core::theme_tokens::ThemeCatalog::builtin()
        .get("document-light")
        .expect("built-in");
    let palette = Palette::from_tokens(&tokens);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(
            |ui, height: &mut f32| {
                let top = ui.cursor().top();
                inline_diff_ui(ui, &diff, &palette);
                *height = ui.cursor().top() - top;
            },
            0.0_f32,
        );
    harness.run();
    *harness.state()
}

#[test]
fn test_inline_diff_scroll_bar_does_not_cover_the_last_line() {
    let wide = "x".repeat(400);
    let narrow = inline_height("a\n", "b\n");
    let scrolled = inline_height("a\n", &format!("{wide}\n"));
    // The wide diff scrolls sideways: its bar takes a row of its own under
    // the lines instead of being drawn over the last one.
    let bar = egui::style::ScrollStyle::solid().bar_width;
    assert!(
        scrolled >= narrow + bar,
        "no room for the bar: narrow {narrow}, wide {scrolled}, bar {bar}"
    );
}

#[test]
fn test_escape_closes_the_diff_overlay() {
    let (_rt, mut harness) = shell();
    harness.run();
    open_overlay(&mut harness);
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!is_open(&harness.ctx), "Esc closes the overlay");
    // The transcript is still there behind it.
    harness.get_by_label(OPEN_DIFF);
}
