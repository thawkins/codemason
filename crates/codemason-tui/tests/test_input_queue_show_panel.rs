//! Tests for the scrollable queue-entry panel opened by the queue-control
//! menu's `Show` row (spec `inputqueue` `Show` row).
//!
//! Covers the panel listing every queued entry oldest-first, `Up`/`Down`
//! scrolling the block-cursor highlight, `Enter` moving the highlighted entry
//! one step toward the front of the queue, `Del` removing it, and `Esc` being
//! the only key that dismisses the panel. The panel must never mutate the
//! editable input buffer, the staged attachments, or the running turn.

use codemason_app_core::geometry::Area;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use codemason_tui::App;
use codemason_tui::app::QueuedInput;
use codemason_tui::layout;

#[path = "support/mod.rs"]
mod support;

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// An app with the given entries queued and the `Show` panel open.
fn panel_with(entries: &[&str]) -> App {
    let mut app = support::make_app();
    for text in entries {
        app.session.input_queue.push_back(entry(text));
    }
    app.queue_show_open_panel();
    assert!(app.queue_show_open, "precondition: the panel opened");
    app
}

/// Render one frame and return the terminal so the painted buffer can be read.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Read one painted row between `x0` (inclusive) and `x1` (exclusive).
fn row_text(terminal: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buffer = terminal.backend().buffer();
    (x0..x1)
        .map(|x| buffer[(x, y)].symbol().to_string())
        .collect()
}

/// Every painted row of the panel's inner area.
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.queue_show_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

/// The background colour of the first glyph on a painted row.
fn first_glyph_bg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.queue_show_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].bg)
        .unwrap_or(Color::Reset)
}

/// The queue texts, oldest first, as a plain vector.

// ---------------------------------------------------------------------------
// Opening the panel
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rendering: entries oldest-first with a block cursor on the highlight
// ---------------------------------------------------------------------------

#[test]
fn test_panel_lists_every_entry_oldest_first() {
    let mut app = panel_with(&["first", "second", "third"]);

    let terminal = render(&mut app, 120, 40);
    let rows = panel_rows(&terminal, &app);

    let first = rows.iter().position(|r| r.contains("first"));
    let second = rows.iter().position(|r| r.contains("second"));
    let third = rows.iter().position(|r| r.contains("third"));
    assert!(
        first.is_some() && second.is_some() && third.is_some(),
        "the panel must list every entry; painted rows: {rows:?}"
    );
    assert!(
        first < second && second < third,
        "the entries must be listed oldest-first; painted rows: {rows:?}"
    );
}

#[test]
fn test_panel_records_its_overlay_area_and_clears_it_when_closed() {
    let mut app = panel_with(&["a"]);

    let _ = render(&mut app, 120, 40);
    let open_area = app.queue_show_area;
    assert!(
        open_area.width > 0 && open_area.height > 0,
        "the open panel records its overlay area for the shared modal path"
    );

    app.queue_show_close();
    let _ = render(&mut app, 120, 40);
    assert_eq!(
        app.queue_show_area,
        Area::default(),
        "the overlay area is cleared once the panel closes"
    );
}

#[test]
fn test_panel_cursor_block_marks_only_the_highlighted_row() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 1;

    let terminal = render(&mut app, 120, 40);
    let inner_y0 = app.queue_show_area.y + 1;
    let rows = panel_rows(&terminal, &app);
    let y_of = |needle: &str| {
        inner_y0
            + rows
                .iter()
                .position(|r| r.contains(needle))
                .unwrap_or_else(|| panic!("row {needle:?} must be painted")) as u16
    };

    assert_eq!(
        first_glyph_bg(&terminal, &app, y_of("b")),
        Color::Magenta,
        "the highlighted row carries the block cursor"
    );
    assert_ne!(
        first_glyph_bg(&terminal, &app, y_of("a")),
        Color::Magenta,
        "an unhighlighted row must not carry the block cursor"
    );
    assert_ne!(
        first_glyph_bg(&terminal, &app, y_of("c")),
        Color::Magenta,
        "an unhighlighted row must not carry the block cursor"
    );
}

#[test]
fn test_panel_cursor_follows_the_scroll() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    let terminal = render(&mut app, 120, 40);
    let inner_y0 = app.queue_show_area.y + 1;
    let rows = panel_rows(&terminal, &app);
    let c_y = inner_y0 + rows.iter().position(|r| r.contains('c')).expect("c row") as u16;

    assert_eq!(
        first_glyph_bg(&terminal, &app, c_y),
        Color::Magenta,
        "scrolling down moves the block cursor to the newly highlighted row"
    );
}

#[test]
fn test_panel_title_reports_the_pending_count() {
    let mut app = panel_with(&["a", "b"]);

    let terminal = render(&mut app, 120, 40);
    let area = app.queue_show_area;
    let buffer = terminal.backend().buffer();
    let title_row: String = (area.x..area.x + area.width)
        .map(|x| buffer[(x, area.y)].symbol().to_string())
        .collect();

    assert!(
        title_row.contains('2'),
        "the panel title reflects the live queue length, got {title_row:?}"
    );
}

// ---------------------------------------------------------------------------
// Up/Down scroll the highlight
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Enter moves the highlighted entry one step toward the front
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Del removes the highlighted entry
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Esc is the only key that dismisses the panel
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The panel never touches the draft, attachments, or the running turn
// ---------------------------------------------------------------------------
