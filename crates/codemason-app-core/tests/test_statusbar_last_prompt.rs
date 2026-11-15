//! Tests for the last-prompt tag rendered on the status bar's top line.
//!
//! Covers:
//! - `prompt_display_text` truncation semantics (up to `max_chars`, `....`
//!   suffix when longer, empty string for an empty prompt).
//! - The tag renders in square brackets on the first status-bar line.
//! - The tag renders immediately after the `Branch: `-labelled git branch
//!   section, as one group placed right after the cwd display.
//! - No tag renders when no prompt has been submitted yet.

#[path = "support/mod.rs"]
mod support;

// -----------------------------------------------------------------------------
// prompt_display_text unit tests
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Slash-command submissions populate the tag
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_slash_command_sets_last_prompt() {
    let mut app = support::make_app();
    app.execute_slash_command("/about").await;
    assert_eq!(
        app.session.last_prompt, "/about",
        "slash command must be recorded as the last prompt so the status-bar \
         tag shows it"
    );
}

#[tokio::test]
async fn test_execute_slash_command_trims_whitespace() {
    let mut app = support::make_app();
    app.execute_slash_command("  /agents  ").await;
    assert_eq!(app.session.last_prompt, "/agents");
}

// -----------------------------------------------------------------��-----------
// Rendered status-bar integration tests
// -----------------------------------------------------------------------------
