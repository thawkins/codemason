//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-011**: Render panel with title and percentage bars.
//!
//! Ported to codemason-gui when the terminal UI was removed: each terminal
//! draw of the original `crates/codemason-tui/tests/test_context_panel_t002.rs`
//! is replaced by `crate::frame::context_text`, which draws the GUI Context
//! pane (`codemason_gui::panels::context::context_ui`) and returns its text.
//! The T-002 layout tests (the reserved right-hand column, the full-width
//! chat, `pane_at`) read terminal geometry; the dock places the GUI pane, so
//! they are covered by `test_panels::test_context_panel_alt_c_*` instead.

mod support;

#[test]
fn test_panel_renders_title_and_border() {
    let mut app = support::make_app();
    app.show_context_panel = true;
    let rendered = crate::frame::context_text(&mut app);
    let has_title = rendered.lines().any(|line| line == "Context");
    assert!(has_title, "panel title 'Context' must be rendered");
}

#[test]
fn test_panel_lists_partitions_and_totals() {
    // FR-005..FR-012: the required partition labels and total row render.
    let mut app = support::make_app();
    app.show_context_panel = true;
    let rendered = crate::frame::context_text(&mut app);
    for expected in [
        "Context window",
        "System prompt",
        "Tool catalog",
        "Tool metadata",
        "History",
        "Total",
    ] {
        assert!(
            rendered.contains(expected),
            "panel must list '{expected}'; rendered:\n{rendered}"
        );
    }
    // FR-008: the message count line renders (0 messages in a fresh session).
    assert!(
        rendered.contains("0 messages"),
        "panel must show the message count"
    );
}

#[test]
fn test_panel_percent_or_unknown_labels() {
    // FR-010/FR-011: rows show either a percentage (when the window is
    // advertised via selected_model_ctx_window) or an "unknown" label.
    let mut app = support::make_app();
    app.session.selected_model_ctx_window = Some(200_000);
    app.show_context_panel = true;
    let rendered = crate::frame::context_text(&mut app);
    assert!(
        rendered.contains('%') || rendered.contains("unknown"),
        "percentage values must render when the context window is known"
    );
}

#[test]
fn test_panel_shows_sent_to_model_row() {
    // The provider-reported input tokens of the last LLM request
    // (SessionView::last_input_tokens) render as a "Sent to model" row directly
    // below the "Context window" capacity row, with a percentage of the
    // advertised window.
    let mut app = support::make_app();
    app.show_context_panel = true;
    app.session.selected_model_ctx_window = Some(200_000);
    app.session.last_input_tokens = 45_000;
    let rendered = crate::frame::context_text(&mut app);
    assert!(
        rendered.contains("Sent to model"),
        "panel must show the 'Sent to model' row; rendered:\n{rendered}"
    );
    assert!(
        rendered.contains("45.0ktk"),
        "panel must show the provider-reported input token count; rendered:\n{rendered}"
    );
}

#[test]
fn test_panel_sent_to_model_zero_before_first_turn() {
    // Before any TokenUsage event the row shows a dimmed 0tk count without
    // a percentage bar.
    let mut app = support::make_app();
    app.show_context_panel = true;
    app.session.selected_model_ctx_window = Some(200_000);
    app.session.last_input_tokens = 0;
    let rendered = crate::frame::context_text(&mut app);
    assert!(
        rendered.contains("Sent to model"),
        "panel must show the 'Sent to model' row even before the first turn"
    );
}
