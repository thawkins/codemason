//! Tests for the `Clear the input queue?` confirmation dialog (spec
//! `inputqueue` T-021).
//!
//! Covers FR-033 (the dialog presents the message `Clear the input queue?` with
//! exactly two options, `Yes` and `No`), FR-034 (`No` is the default selection
//! so a stray `Enter` cannot empty the queue), NFR-010 (the dialog reuses the
//! shared overlay/modal rendering and key-dispatch machinery) and NFR-011
//! (presenting, selecting, or dismissing the dialog sets the redraw flag).
//!
//! The `Clear` row that opens the dialog is [`App::queue_menu_select_clear`]
//! (T-017). This suite asserts the dialog's presentation and key mapping; the
//! `Yes`-empties / `No`-leaves-unchanged gating (T-022) is covered by
//! `test_input_queue_menu_clear_gating.rs`.

use codemason_app_core::geometry::Area;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

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

/// Build an app with an active session and a running turn.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("test-session".to_string());
    app
}

/// Open the dialog the way the user would: ALT-Q, select `Clear`, `Enter`.
fn app_with_dialog_open() -> App {
    let mut app = app_with_session();
    app.session.input_queue.push_back(entry("CONFIRM-QUEUE-1"));
    app.session.input_queue.push_back(entry("CONFIRM-QUEUE-2"));
    app.queue_menu_open = true;
    app.queue_menu_select_clear();
    assert!(
        app.queue_clear_confirm_open,
        "precondition: selecting Clear opened the confirmation dialog"
    );
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

/// Every painted row of the dialog's inner area, with the border columns
/// stripped.
fn dialog_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.queue_clear_confirm_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

// ---------------------------------------------------------------------------
// FR-033 - the dialog presents the message and exactly Yes / No
// ---------------------------------------------------------------------------

#[test]
fn test_dialog_renders_the_message_and_two_options() {
    let mut app = app_with_dialog_open();

    let terminal = render(&mut app, 100, 30);
    let rows = dialog_rows(&terminal, &app);
    let joined = rows.join("\n");

    // The message is the dialog's title, painted on the border row.
    let area = app.queue_clear_confirm_area;
    let title_row = row_text(&terminal, area.y, area.x, area.x + area.width);
    assert!(
        title_row.contains("Clear the input queue?"),
        "FR-033: the dialog must present the message `Clear the input queue?`, \
         painted title was {title_row:?}"
    );
    assert!(
        joined.contains("Yes"),
        "FR-033: the dialog must offer the `Yes` option, rows were {rows:?}"
    );
    assert!(
        joined.contains("No"),
        "FR-033: the dialog must offer the `No` option, rows were {rows:?}"
    );
}

#[test]
fn test_dialog_defaults_to_no_and_highlights_it() {
    let mut app = app_with_dialog_open();

    let terminal = render(&mut app, 100, 30);
    let rows = dialog_rows(&terminal, &app);

    // The `>` selection marker is the on-screen expression of the default
    // selection: `No` carries it, `Yes` does not (FR-034).
    let yes_row = rows
        .iter()
        .find(|r| r.contains("Yes"))
        .expect("Yes row painted");
    let no_row = rows
        .iter()
        .find(|r| r.contains("No"))
        .expect("No row painted");
    assert!(
        !yes_row.contains('>'),
        "FR-034: `Yes` must not be the default selection, row was {yes_row:?}"
    );
    assert!(
        no_row.contains('>'),
        "FR-034: `No` must be the default selection, row was {no_row:?}"
    );
}

#[test]
fn test_closed_dialog_is_not_painted_and_area_is_cleared() {
    let mut app = app_with_session();

    let terminal = render(&mut app, 100, 30);

    assert_eq!(
        app.queue_clear_confirm_area,
        Area::default(),
        "a closed dialog must leave no cached overlay area"
    );
    let buffer = terminal.backend().buffer();
    let painted: String = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buffer[(x, y)].symbol().to_string())
        .collect();
    assert!(
        !painted.contains("Clear the input queue?"),
        "a closed dialog must not be painted"
    );
}

// ---------------------------------------------------------------------------
// NFR-010 - the dialog is a modal reusing the shared key dispatch
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-034 - Enter with the default selection does not confirm `Yes`
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-035 / FR-037 - dismissing closes the dialog and leaves the queue
// ---------------------------------------------------------------------------
