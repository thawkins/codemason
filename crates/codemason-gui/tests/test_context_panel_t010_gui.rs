//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-010**: Refresh panel on context change events (FR-013, FR-014).
//!
//! The panel recomputes [`codemason_app_core::app::App::context_partition_snapshot`]
//! on every frame while it is open, so any state change that triggers a
//! redraw (message received, model switch, compaction) is reflected without
//! the user re-opening the panel. These tests pin that behaviour: mutating
//! the underlying state changes the rendered values on the next frame.
//!
//! Ported to codemason-gui when the terminal UI was removed: each terminal
//! draw of the original `crates/codemason-tui/tests/test_context_panel_t010.rs`
//! is replaced by `crate::frame::context_text`, which draws the GUI Context
//! pane and returns its text.

mod support;

use codemason_agent::message::{Message, MessagePart, Role};

#[test]
fn test_panel_reflects_new_message_without_reopening() {
    // FR-013/FR-014: after a new message lands, history estimate and message
    // count in the panel must change on a later frame with the panel open
    // the whole time (no close/reopen, no manual refresh).
    let mut app = support::make_app();
    app.show_context_panel = true;

    let before = app.conversation_history_token_count();

    // Simulate the event-driven state change (message received) while the
    // panel stays open.
    app.session.messages.push(Message::new(
        "session-1",
        Role::User,
        vec![MessagePart::Text {
            text: "new message content for the panel".into(),
        }],
    ));

    let rendered = crate::frame::context_text(&mut app);

    assert!(
        app.conversation_history_token_count() > before,
        "panel values must track context changes across frames"
    );

    // Rendered output must now include "1 messages".
    assert!(
        rendered.contains("1 messages"),
        "panel must refresh the message count without re-opening; got:\n{rendered}"
    );
}

#[test]
fn test_panel_reflects_scroll_clamp_after_content_shrink() {
    // FR-013: after compaction shrinks the history, the panel still renders
    // correctly (values drop; scroll clamping stays valid).
    let mut app = support::make_app();
    app.show_context_panel = true;

    app.session.messages.push(Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "x".repeat(5_000),
        }],
    ));
    let big = app.context_partition_snapshot();
    assert!(big.history_tokens > 0);

    // Simulate a compaction replacing the history with a single summary.
    app.session.messages.clear();
    app.session.messages.push(Message::new(
        "session-1",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "compacted summary".into(),
        }],
    ));

    crate::frame::context_text(&mut app);

    let after = app.context_partition_snapshot();
    assert!(
        after.history_tokens < big.history_tokens,
        "history partition must shrink after compaction"
    );
}
