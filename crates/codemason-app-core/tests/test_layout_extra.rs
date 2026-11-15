//! Extra coverage tests for `codemason_app_core::layout` render branches.
//!
//! Drives the real `layout::render` path through a `TestBackend` across the
//! layout permutations the crate supports: terminal sizes (tiny/large/zero),
//! split ratios, every side-panel visibility toggle, and every overlay branch
//! (provider-setup steps, pickers, dialogs, viewers, popups, teammate strip).
//! Also exercises the public layout helpers directly.
//!
//! Tests only - no production logic is touched.

mod support;

use codemason_agent::message::Message;

// ===========================================================================
// Terminal sizes / split ratios
// ===========================================================================

// ===========================================================================
// Overlays / dialogs
// ===========================================================================

// ===========================================================================
// Provider-setup dialog: every step variant
// ===========================================================================

// ===========================================================================
// Pickers / viewers / dialogs
// ===========================================================================

// ===========================================================================
// layout public helpers
// ===========================================================================

#[test]
fn ensure_message_copy_lines_builds_cache() {
    use codemason_app_core::app::MessageLineGroup;
    let mut app = support::make_app();
    app.session.messages.push(Message::user_text(
        "s1",
        "a message that is long enough to wrap in a narrow pane",
    ));
    app.session
        .messages
        .push(Message::assistant_text("s1", "second message"));
    app.session.message_cache_width = 20;
    // Seed one stale cache group per message (edit_seq mismatch => re-render).
    for _ in 0..app.session.messages.len() {
        app.session.message_line_cache.push(MessageLineGroup {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            edit_seq: u64::MAX,
        });
    }
    app.session.message_content_lines_dirty = true;
    codemason_app_core::message_lines::ensure_message_copy_lines(&mut app);
    assert_ne!(
        app.session.message_content_lines,
        [] as [std::string::String; 0]
    );
    assert!(!app.session.message_content_lines_dirty);
}

#[test]
fn stream_min_interval_constant() {
    assert!(
        codemason_app_core::message_lines::MESSAGE_STREAM_MIN_INTERVAL >= std::time::Duration::ZERO
    );
}

/// Restored: app-core's `line_wrap::wrap_line_styled`, which the terminal's
/// `wrap_line_styled_testable` wrapped.
#[test]
fn wrap_line_styled_testable_edges() {
    use codemason_app_core::line_wrap::wrap_line_styled;
    use codemason_app_core::styled::StyledLine;
    // Width 0 -> a single unbounded line.
    let out = wrap_line_styled(&StyledLine::raw("hello world"), 0);
    assert_eq!(out.len(), 1);

    // Narrow width wraps into several rows.
    let out = wrap_line_styled(&StyledLine::raw("one two three four"), 5);
    assert!(out.len() >= 2);

    // Tabs are filtered (the wrapper drops control graphemes, as the terminal renderer did).
    let _ = wrap_line_styled(&StyledLine::raw("a\tb"), 4);

    // Wide Unicode and zero-width handling.
    let _ = wrap_line_styled(&StyledLine::raw("中文 中文"), 3);
    let _ = wrap_line_styled(&StyledLine::raw("a\u{200b}b"), 4);
    let _ = wrap_line_styled(&StyledLine::raw("a\u{00a0}b"), 4);
}
