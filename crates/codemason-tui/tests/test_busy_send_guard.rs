//! Tests for how the input field behaves while the primary agent is busy.
//!
//! Spec `inputqueue` (FR-002, FR-011, FR-012, FR-017) relaxes the old guard so a
//! plain message is still accepted while a turn is executing. FR-017 was amended
//! to also queue slash commands the same way, while bang commands and
//! teammate-targeted messages keep their busy behaviour and are never queued.

use ratatui::{Terminal, backend::TestBackend, style::Color};

use codemason_tui::{App, layout};

#[path = "support/mod.rs"]
mod support;

fn render_and_get_input_border_color(app: &mut App) -> Color {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");

    let input_area = app.input_area;
    let buffer = terminal.backend().buffer();
    buffer[(input_area.x, input_area.y)].fg
}

#[test]
fn test_input_border_is_white_when_idle() {
    let mut app = support::make_app();

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(color, Color::White);
}

#[test]
fn test_input_border_stays_white_while_processing() {
    let mut app = support::make_app();
    app.session.is_processing = true;

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(
        color,
        Color::White,
        "the input border stays unlocked while the agent executes (FR-011)"
    );
}

#[test]
fn test_input_border_is_red_when_a_modal_is_open() {
    let mut app = support::make_app();
    app.session.is_processing = true;
    // An overlay that swallows keystrokes genuinely locks the input field.
    app.pending_memory_delete = Some(codemason_tui::app::PendingMemoryDelete {
        id: 1,
        preview: "preview".to_string(),
    });

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(color, Color::Red);
}
