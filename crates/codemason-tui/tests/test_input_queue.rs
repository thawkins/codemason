//! Tests for the message input queue foundation and counter render
//! (spec `inputqueue`).
//!
//! T-001 covers FR-001 (in-memory FIFO queue preserving submission order),
//! FR-004 (bounded to 32 entries), NFR-001 (O(1) enqueue/dequeue backed by a
//! deque) and NFR-005 (cleared on session reset, never persisted).
//!
//! T-004 covers FR-008/FR-009/FR-010 (two-digit zero-padded counter before `>`
//! while entries are pending; bare prompt when empty) and NFR-002/NFR-003 (a
//! fixed-width prefix that repaints on a queue change).

#[path = "support/mod.rs"]
mod support;

use ratatui::{Terminal, backend::TestBackend};

use codemason_tui::app::QueuedInput;
use codemason_tui::layout;

use codemason_tui::App;

/// Render one frame and return the text painted on the input area's first row.
fn input_row_text(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");

    let area = app.input_area;
    let buffer = terminal.backend().buffer();
    // Row of text starts one column inside the block border.
    (area.x + 1..area.x + area.width.saturating_sub(1))
        .map(|x| buffer[(x, area.y + 1)].symbol().to_string())
        .collect()
}

/// Build a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

// -- T-004: two-digit queue counter render --------------------------------

#[test]
fn test_prompt_is_bare_when_queue_is_empty() {
    let mut app = support::make_app();

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("> "),
        "an empty queue renders the bare prompt with no counter (FR-010): {row:?}"
    );
}

#[test]
fn test_counter_is_shown_when_queue_is_non_empty() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("one"));

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("01> "),
        "one queued entry renders a zero-padded counter immediately before `>` (FR-009): {row:?}"
    );
}

#[test]
fn test_counter_is_two_digits_zero_padded() {
    let mut app = support::make_app();
    for i in 0..3 {
        app.session.input_queue.push_back(entry(&format!("m{i}")));
    }

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("03> "),
        "the counter is zero-padded to two digits (FR-009): {row:?}"
    );
}

#[test]
fn test_counter_is_rendered_before_typed_text() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("one"));
    app.session.input_queue.push_back(entry("two"));
    app.session.input = "hello".to_string();
    app.session.input_cursor = app.session.input.chars().count();

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("02> hello"),
        "the counter renders immediately before the typed text (FR-009): {row:?}"
    );
}

#[test]
fn test_counter_is_clamped_to_two_digits() {
    let mut app = support::make_app();
    // Push directly past the cap to exercise the clamp (the render never
    // depends on the enqueue path enforcing the bound).
    for i in 0..120 {
        app.session.input_queue.push_back(entry(&format!("m{i}")));
    }

    let row = input_row_text(&mut app);

    assert!(
        row.starts_with("99> "),
        "the counter stays a fixed two columns wide and clamps at 99 (NFR-002): {row:?}"
    );
}

#[test]
fn test_prompt_reverts_when_queue_empties() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("one"));
    assert!(input_row_text(&mut app).starts_with("01> "));

    app.session.input_queue.pop_front();

    let row = input_row_text(&mut app);
    assert!(
        row.starts_with("> "),
        "the counter disappears once the queue is empty (FR-008, FR-010): {row:?}"
    );
}

#[test]
fn test_render_reflects_enqueue_on_next_frame() {
    let mut app = support::make_app();
    assert!(input_row_text(&mut app).starts_with("> "));

    app.session.input_queue.push_back(entry("one"));

    assert!(
        input_row_text(&mut app).starts_with("01> "),
        "the counter updates on the next frame after an enqueue (FR-009, NFR-003)"
    );
}

#[test]
fn test_input_render_cache_tracks_queue_len() {
    let mut app = support::make_app();
    let _ = input_row_text(&mut app);
    assert_eq!(app.session.input_render_cache.queue_len, 0);

    app.session.input_queue.push_back(entry("one"));
    let _ = input_row_text(&mut app);

    assert_eq!(
        app.session.input_render_cache.queue_len, 1,
        "the render cache must rebuild when the counter width changes (NFR-002)"
    );
}

// ---------------------------------------------------------------------------
// T-011 - configurable capacity with a 32 default (FR-015)
// ---------------------------------------------------------------------------
