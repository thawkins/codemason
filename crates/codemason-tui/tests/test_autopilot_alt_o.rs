//! Tests for the `Alt+O` autopilot toggle and its status-bar indicator.
//!
//! `Alt+O` used to toggle the Telemetry side panel; it now toggles autopilot
//! mode. These tests drive the real `handle_key_event` path (not a private
//! helper) and a full-frame `layout::render` on a ratatui `TestBackend`, so the
//! assertion covers the whole pipeline: key chord -> `InputAction` -> state ->
//! rendered status bar.
//!
//! Tests only - no production logic is touched.

mod support;

use codemason_tui::App;
use codemason_tui::crossterm_input::CrosstermAppExt;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Press the `Alt+O` chord the way the TUI main loop does.
async fn press_alt_o(app: &mut App) {
    app.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::ALT))
        .await;
}

/// Extract the full rendered screen text (one line per row) from a backend.
fn rendered_text(backend: &ratatui::backend::TestBackend) -> String {
    let buffer = backend.buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Draw one full frame and return the rendered screen text.
fn draw(app: &mut App, cols: u16, rows: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(cols, rows);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("draw");
    rendered_text(terminal.backend())
}

// -----------------------------------------------------------------------------
// Alt+O toggles autopilot
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Status bar reflects the state
// -----------------------------------------------------------------------------

#[test]
fn status_bar_shows_autopilot_off_before_enabling() {
    let mut app = support::make_app();
    let screen = draw(&mut app, 200, 40);

    assert!(
        screen.contains("AUTO:"),
        "the status bar carries the autopilot indicator:\n{screen}"
    );
    assert!(
        screen.contains("AUTO: \u{2717} "),
        "the autopilot indicator reads disabled while off:\n{screen}"
    );
    assert!(
        !screen.contains("(Alt+O)"),
        "the status bar carries no chord label; `?` documents the keys:\n{screen}"
    );
}

#[tokio::test]
async fn status_bar_shows_autopilot_on_after_alt_o() {
    let mut app = support::make_app();

    press_alt_o(&mut app).await;
    let screen = draw(&mut app, 200, 40);

    assert!(
        screen.contains("AUTO:"),
        "the autopilot indicator stays on the bar once enabled:\n{screen}"
    );
    // A green check mark appears beside the indicator; several other services
    // may also be green, so assert on the indicator pairing itself.
    assert!(
        screen.contains("AUTO: \u{2713} "),
        "the autopilot indicator reads enabled after Alt+O:\n{screen}"
    );
}

#[tokio::test]
async fn status_bar_counts_down_the_autopilot_time_limit() {
    let mut app = support::make_app();

    press_alt_o(&mut app).await;
    let screen = draw(&mut app, 200, 40);

    // The armed limit is 5000 s, rendered as a fixed-width HH:MM:SS countdown
    // right after the indicator. A frame drawn moments after the start stamp
    // is still inside the first second, so the full 01:23:20 shows.
    assert!(
        screen.contains("AUTO: \u{2713} [01:23:20] "),
        "the status bar counts down the armed autopilot limit:\n{screen}"
    );
}

#[tokio::test]
async fn status_bar_omits_the_countdown_without_a_time_limit() {
    let mut app = support::make_app();

    // `/autopilot on` with no flags runs unbounded, so there is nothing to
    // count down.
    app.execute_slash_command("/autopilot on").await;
    assert!(app.session.autopilot_time_limit_secs.is_none());
    let screen = draw(&mut app, 200, 40);

    assert!(
        screen.contains("AUTO: \u{2713} "),
        "the indicator still reads enabled:\n{screen}"
    );
    assert!(
        !screen.contains("AUTO: \u{2713} ["),
        "no countdown is drawn for an unbounded run:\n{screen}"
    );
}
