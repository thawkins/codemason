//! Tests for the `/telemetry` slash-command family.
//!
//! These tests exercise the non-interactive subcommands (`help`, `counters`,
//! and input validation) without launching a full terminal backend, plus the
//! `/telemetry setup` dialog's display order and its save-and-reconfigure
//! path.
//!
//! The save test enters a scratch project directory and redirects the config
//! there with the Rust 2024 `unsafe` `std::env` API; that is the sanctioned
//! test use of `unsafe` in this workspace (see `AGENTS.md`).

#![allow(unsafe_code)]
// The save test drives the async `handle_key_event` while holding the crate's
// `global_env_lock`, so the process-global cwd and env vars cannot change under
// it. `std::sync::MutexGuard` is not `Send`, but nextest runs every test in
// its own process and the future is polled on one thread, so the guard cannot
// cross a thread boundary. This mirrors the other env-mutating test files here.
#![allow(clippy::await_holding_lock)]

#[path = "support/mod.rs"]
mod support;

use codemason_tui::App;
use codemason_tui::crossterm_input::CrosstermAppExt;

/// Enter an empty scratch project with no global config, for as long as the
/// returned guard lives.
///
/// `/telemetry setup` prefills its fields from `Config::load()`, which reads the
/// operator's user-global `codemason.json`. Asserting the *defaults* therefore
/// asserted whatever the developer had configured - the test passed on a CI box
/// with no config and failed on a workstation whose `telemetry.otel.logs.level`
/// was `all`. Redirecting `XDG_CONFIG_HOME` into a scratch tree and the cwd into
/// an empty project makes the loaded config the built-in default.
///
/// Both mutations are process-global, so this returns the crate-wide
/// [`global_env_lock`](crate::global_env_lock) guard; the caller binds it for the
/// length of the test and must not take the lock again (it is not reentrant).
/// The cwd and env are restored on drop.

/// Restores the process-global cwd and config env vars on drop.

/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

/// Walk the rendered telemetry dialog and return, in screen order, the field
/// index whose row currently carries the `>` marker.
///
/// The dialog paints the active field with a leading `"> "` gutter and every
/// inactive field with `"  "`, one `label` line above each value line. This
/// helper maps the marker back to the field index so a test can assert that
/// display order matches the `Tab` order.
fn marked_field_order(app: &mut App) -> Vec<(String, u8)> {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(90, 34);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("render frame");
    let buffer = terminal.backend().buffer().clone();

    // Field index by the label line that precedes its value line.
    let labels: [(&str, u8); 6] = [
        ("OTLP endpoint:", 0),
        ("Transport protocol:", 1),
        ("Export interval (s):", 2),
        ("Export timeout (s):", 3),
        ("Internal Prometheus port:", 4),
        // Spec `otel` FR-036: the logs-signal level is the last field.
        ("Logs level:", 5),
    ];

    let mut rows: Vec<String> = Vec::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        rows.push(line.trim_end().to_string());
    }

    let mut order = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        // The value row is painted as `<border> <gutter> <value>`, where the
        // gutter is `"> "` for the active field and `"  "` otherwise.
        if !row.contains("> ") {
            continue;
        }
        // The marker can only be on a value row; walk back to the nearest
        // label row above it to name the field.
        // The dialog is centred inside a bordered box, so a label row is
        // `... <border> <label> ... <border> ...`; match on containment.
        let label = rows[..index].iter().rev().find_map(|candidate| {
            labels
                .iter()
                .find(|(text, _)| candidate.contains(text))
                .map(|(text, idx)| ((*text).to_string(), *idx))
        });
        if let Some(entry) = label {
            order.push(entry);
        }
    }
    order
}

/// Bug fix regression: the telemetry dialog used to render the protocol row
/// last while `Tab` cycled it at index 1, so the highlight jumped forwards to
/// the bottom of the form and then backwards. Display order must now match
/// `Tab` order, i.e. consecutive presses move the marker exactly one row down.
#[tokio::test]
async fn test_telemetry_setup_display_order_matches_tab_order() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = make_app();
    app.execute_slash_command("/telemetry setup").await;

    let mut previous: Option<u8> = None;
    for expected in 0u8..6 {
        let order = marked_field_order(&mut app);
        let (label, index) = order
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("step {expected}: no marked row found, order={order:?}"));
        assert_eq!(
            index, expected,
            "field {expected} should be marked at step {expected}; got {label} ({index}) - \
             display order must follow the Tab order 0,1,2,3,4,5"
        );
        if let Some(prev) = previous {
            assert_eq!(
                index,
                prev + 1,
                "Tab must move the marker one row down, {prev} -> {index}"
            );
        }
        previous = Some(index);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .await;
    }

    // One more Tab wraps back to the first field, still one row from "end".
    let order = marked_field_order(&mut app);
    assert_eq!(
        order.first().map(|(_, idx)| *idx),
        Some(0),
        "Tab from the last field must wrap to the first"
    );
}
