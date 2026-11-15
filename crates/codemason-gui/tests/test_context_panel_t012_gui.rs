//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-012**: Ensure panel content is excluded from LLM context (FR-016).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_context_panel_t012.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

#[test]
fn test_rendering_panel_does_not_mutate_conversation() {
    // FR-016: the context breakdown is a UI-only artifact. Rendering the
    // panel must not append messages, alter the history estimate, or change
    // the message count that feeds the LLM request.
    let mut app = support::make_app();
    app.show_context_panel = true;

    let before_messages = app.conversation_message_count();
    let before_history = app.conversation_history_token_count();

    crate::frame::publish(&mut app);

    assert_eq!(
        app.conversation_message_count(),
        before_messages,
        "panel render must not add messages"
    );
    assert_eq!(
        app.conversation_history_token_count(),
        before_history,
        "panel render must not change the history estimate"
    );
    assert!(
        app.session.messages.is_empty(),
        "panel content must never enter the session message list"
    );
}
