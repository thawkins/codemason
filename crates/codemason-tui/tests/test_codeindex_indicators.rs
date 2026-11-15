//! Tests for the codeindex busy indicators on the status bar (line 2,
//! top-right) and the background codeindex task result poller.
//!
//! Covers:
//! - `BUSY:cdx-idx` renders when `code_index_busy` is latched (FTS indexing in
//!   progress).
//! - `BUSY:cdx-graph` renders when `code_index_graph_busy` is latched
//!   (semantic graph being built) and does NOT render when idle.
//! - Both indicators appear on the second status-bar line.
//! - `poll_codeindex_bg_result` drains the off-thread completion payload,
//!   clears the spawned latches, and sets the carried status text.
//! - The graph-busy latch self-clears when no index is attached.

#[path = "support/mod.rs"]
mod support;

use codemason_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// Open an in-memory CodeIndex and attach it to the app, mirroring how the
/// real app attaches the index after `/codeindex on`.

/// Age out the refresh throttle so the refresh body actually runs.

/// Build an App mirroring the make_app() pattern used by the other TUI tests.
/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

/// Width of the test terminal. Wide enough that the provider/model label
/// (whose length depends on the resolved default provider) can never push the
/// busy tags past the right edge and clip them - the status bar itself has no
/// truncation stage for the left/center/right sections.
const TEST_WIDTH: u16 = 200;

/// Height of the test terminal.
const TEST_HEIGHT: u16 = 40;

/// Render the app and return the visible frame text.
fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    let rows: Vec<String> = (0..TEST_HEIGHT)
        .map(|y| {
            (0..TEST_WIDTH)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect();
    rows.join("\n")
}

#[test]
fn test_statusbar_renders_graph_busy_indicator() {
    let mut app = make_app();
    app.code_index_graph_busy = true;
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("cdx-graph"),
        "second status-bar line must contain the graph-busy indicator when the \
         graph is building; line was: {line2}"
    );
}

#[test]
fn test_statusbar_renders_index_busy_indicator() {
    let mut app = make_app();
    app.code_index_busy = true;
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("cdx-idx"),
        "second status-bar line must contain the index-busy indicator while \
         indexing; line was: {line2}"
    );
}

#[test]
fn test_statusbar_renders_both_indicators_together() {
    let mut app = make_app();
    app.code_index_busy = true;
    app.code_index_graph_busy = true;
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("cdx-idx") && line2.contains("cdx-graph"),
        "both busy indicators must render when indexing and graph building \
         overlap; line was: {line2}"
    );
}

#[test]
fn test_statusbar_hides_indicators_when_idle() {
    let mut app = make_app();
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        !line2.contains("cdx-idx") && !line2.contains("cdx-graph"),
        "no busy indicators may render when the index is idle; line was: {line2}"
    );
}

#[test]
fn test_poll_codeindex_bg_result_drains_completion() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    app.code_index_graph_spawned = true;
    app.status = "[wait] codeindex: building graph…".to_string();

    let payload = Ok((
        "\u{2705} Graph built: 42 edges (30 EXTRACTED, 12 INFERRED) in 100ms.".to_string(),
        "codeindex: graph built (42 edges)".to_string(),
    ));
    {
        let mut guard = app.code_index_bg_result.lock().unwrap();
        *guard = Some(payload);
    }

    app.poll_codeindex_bg_result();

    assert_eq!(app.status, "codeindex: graph built (42 edges)");
    assert!(
        !app.code_index_graph_spawned,
        "spawned latch must clear once the result is drained"
    );
    // The rendered message window is repopulated on each render pass.
    let _frame = render_app_to_string(&mut app);
    let window = app.session.message_content_lines.join("\n");
    assert!(
        window.contains("Graph built: 42 edges"),
        "message window must show the completion message; got: {window}"
    );
}

#[test]
fn test_poll_codeindex_bg_result_reports_error() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    app.code_index_reindex_spawned = true;
    {
        let mut guard = app.code_index_bg_result.lock().unwrap();
        *guard = Some(Err("Re-index failed: disk on fire".to_string()));
    }

    app.poll_codeindex_bg_result();

    assert!(!app.code_index_reindex_spawned);
    assert!(
        app.status.contains("failed"),
        "status must surface the failure; got: {}",
        app.status
    );
    // The rendered message window is repopulated on each render pass.
    let _frame = render_app_to_string(&mut app);
    let window = app.session.message_content_lines.join("\n");
    assert!(
        window.contains("Re-index failed"),
        "message window must show the error; got: {window}"
    );
}
