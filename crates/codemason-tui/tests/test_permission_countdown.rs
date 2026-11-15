//! Test for permission dialog countdown timer display

#[path = "support/mod.rs"]
mod support;

use std::time::{SystemTime, UNIX_EPOCH};

use codemason_agent::permission::PermissionRequest;
use codemason_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render permission dialog");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn test_permission_dialog_renders_initial_120_second_countdown() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut app = make_app();
    app.session.permission_queue.push_back(PermissionRequest {
        id: "req-1".to_string(),
        session_id: "session-1".to_string(),
        permission: "file:write".to_string(),
        patterns: vec![
            "create: crates/codemason-tui/tests/test_permission_countdown.rs".to_string(),
        ],
        metadata: serde_json::json!({
            "created_at": now,
            "timeout_secs": 120u64,
        }),
        tool_call_id: None,
    });

    let rendered = render_app_to_string(&mut app);

    assert!(
        rendered.contains("Permission Required (2:00 remaining)")
            || rendered.contains("Permission Required (1:59 remaining)")
    );
}
