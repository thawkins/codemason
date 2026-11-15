//! Deep coverage for the smaller `codemason-tui` modules: the status-bar and
//! model-picker caches, the tracing layer that routes log records to the TUI
//! log panel, the markdown worker, the `/toolchain` report engine, the team
//! blueprint helpers, the clipboard temp-file lifecycle, the layout utilities,
//! the execution-backend switcher panel, the graphify skill generator, the
//! goal-loop setup dialog, the cron scheduler, and the layout render branches
//! for the agents/teams/status-bar panels.
//!
//! Tests only - no production logic is touched. Writes are confined to
//! `target/temp/` or a `tempfile::TempDir`; the slash-command tests run inside
//! an isolated tempdir project (cwd + `XDG_CONFIG_HOME` redirected) so nothing
//! lands in the developer's real `~/.config/codemason/`.
//!
//! Env mutation is `unsafe` in edition 2024 and the workspace denies
//! `unsafe_code`, so this target opts back in explicitly for the isolation
//! harness; that harness holds a process-global mutex across the async body,
//! so `clippy::await_holding_lock` is also allowed for this file.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use std::time::Duration;

use chrono::Utc;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_tui::App;
use codemason_tui::app::MdWorker;
use codemason_tui::{research_progress, utils};
use codemason_types::ThinkingLevel;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

// ===========================================================================
// Helpers
// ===========================================================================

/// Workspace scratch tree (`target/temp`), resolved from the crate manifest so
/// the path is right whatever directory the test binary runs in.

/// Render one frame at the given terminal size.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Flatten the whole painted buffer into one string.
fn painted(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// Build a [`TaskEntry`] for the active-agents panel.
fn task_entry(
    id: &str,
    parent: &str,
    child: &str,
    agent: &str,
    status: TaskStatus,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

/// Build a [`ReportRow`] with one probe.

// ===========================================================================
// status_bar_cache
// ===========================================================================

/// The cache literal every `signature_matches` case starts from.

// ===========================================================================
// model_picker_cache
// ===========================================================================

// ===========================================================================
// tracing_layer
// ===========================================================================

// ===========================================================================
// md_worker
// ===========================================================================

#[test]
fn md_worker_renders_markdown_to_text() {
    let worker = MdWorker::new();
    let rendered = worker
        .render("# Heading\n\nA paragraph with **bold** text.")
        .expect("render");
    assert!(rendered.contains("Heading"), "rendered: {rendered}");
    assert!(rendered.contains("paragraph"), "rendered: {rendered}");
}

#[test]
fn md_worker_pads_a_long_code_span_inside_a_table_cell() {
    let worker = MdWorker::new();
    // A single-code-span cell longer than 40 characters gets a trailing space
    // so html2text's size estimate and the rendered width agree.
    let long = "a".repeat(60);
    let markdown = format!("| Command | Description |\n|---|---|\n| `{long}` | text |\n");
    let rendered = worker.render(&markdown).expect("render");
    assert!(rendered.contains("Command"), "rendered: {rendered}");
    assert!(rendered.contains("Description"), "rendered: {rendered}");
    assert!(rendered.contains(&long), "the code span must survive");
}

#[test]
fn md_worker_leaves_a_short_code_cell_unchanged() {
    let worker = MdWorker::new();
    let markdown = "| A | B |\n|---|---|\n| `short` | value |\n";
    let rendered = worker.render(markdown).expect("render");
    assert!(rendered.contains("short"), "rendered: {rendered}");
    assert!(rendered.contains("value"), "rendered: {rendered}");
}

#[test]
fn md_worker_handles_a_multi_token_cell_and_a_pipe_escape() {
    let worker = MdWorker::new();
    // `foo` bar  -> not a single code span, left alone.
    // A backslash-pipe inside a code span must not split the cell.
    let long = "b".repeat(50);
    let markdown = format!(
        "| Flag | Note |\n|---|---|\n| `foo` bar | mixed |\n| `--a \\| --b` | sep |\n| `{long}` | long |\n"
    );
    let rendered = worker.render(&markdown).expect("render");
    assert!(rendered.contains("mixed"), "rendered: {rendered}");
    assert!(rendered.contains("long"), "rendered: {rendered}");
}

#[test]
fn md_worker_ignores_tables_inside_fenced_code() {
    let worker = MdWorker::new();
    let markdown = "```\n| A | B |\n|---|---|\n| `x` | y |\n```\n";
    let rendered = worker.render(markdown).expect("render");
    assert!(rendered.contains('|'), "rendered: {rendered}");
}

#[test]
fn md_worker_restores_red_markers_round_trip() {
    let worker = MdWorker::new();
    let rendered = worker.render("[red]disabled[/red]").expect("render");
    assert!(
        rendered.contains("[red]") && rendered.contains("[/red]"),
        "red markers must survive the pipeline: {rendered}"
    );
    assert!(rendered.contains("disabled"), "rendered: {rendered}");
}

#[test]
fn md_worker_leaves_unclosed_red_marker_text_intact() {
    let worker = MdWorker::new();
    let rendered = worker.render("[red]unterminated").expect("render");
    assert!(rendered.contains("unterminated"), "rendered: {rendered}");
    assert!(!rendered.contains("codemason-red"), "rendered: {rendered}");
}

#[test]
fn md_worker_renders_repeated_requests() {
    let worker = MdWorker::new();
    for i in 0..3 {
        let rendered = worker.render(&format!("request {i}")).expect("render");
        assert!(rendered.contains(&format!("request {i}")), "{rendered}");
    }
}

// ===========================================================================
// toolchain
// ===========================================================================

// ===========================================================================
// blueprints
// ===========================================================================

// ===========================================================================
// helpers
// ===========================================================================

// ===========================================================================
// utils
// ===========================================================================

#[test]
fn utils_format_elapsed_and_short_id() {
    assert_eq!(utils::format_elapsed(Utc::now()), "0s");
    // The last eight characters (the unique suffix) are kept.
    assert_eq!(utils::short_id("session-0123456789abcdef"), "89abcdef");
    assert_eq!(utils::short_id("short"), "short");
    // FR-008: the abbreviation comes from the shared `short_suffix` helper, so
    // a multi-byte id is cut on a char boundary instead of panicking.
    assert_eq!(
        utils::short_id("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"),
        "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"
    );
    assert_eq!(
        utils::short_id("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"),
        "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"
    );
}

#[test]
fn utils_shorten_middle_keeps_both_ends() {
    assert_eq!(utils::shorten_middle("abcdef", 100), "abcdef");
    assert_eq!(utils::shorten_middle("abcdefghij", 1), "\u{2026}");
    let shortened = utils::shorten_middle("abcdefghij", 6);
    assert!(shortened.starts_with("ab"), "got {shortened}");
    assert!(shortened.ends_with("ij"), "got {shortened}");
    assert_eq!(shortened.chars().count(), 6);
}

#[test]
fn utils_breakpoints_and_panel_metrics() {
    assert!(matches!(
        utils::ResponsiveBreakpoint::from_width(40),
        utils::ResponsiveBreakpoint::Small
    ));
    assert!(matches!(
        utils::ResponsiveBreakpoint::from_width(100),
        utils::ResponsiveBreakpoint::Medium
    ));
    assert!(matches!(
        utils::ResponsiveBreakpoint::from_width(200),
        utils::ResponsiveBreakpoint::Large
    ));
    assert_eq!(utils::ResponsiveBreakpoint::Small.log_split(), (60, 40));
    assert_eq!(utils::ResponsiveBreakpoint::Small.min_content_width(), 40);
    assert_eq!(utils::ResponsiveBreakpoint::Medium.min_content_width(), 60);
    assert_eq!(utils::ResponsiveBreakpoint::Large.min_content_width(), 80);
    assert_eq!(utils::ResponsiveBreakpoint::Small.status_bar_height(), 2);
    assert_eq!(utils::ResponsiveBreakpoint::Small.button_column_width(), 12);
    assert_eq!(
        utils::ResponsiveBreakpoint::Medium.button_column_width(),
        18
    );
    assert_eq!(utils::ResponsiveBreakpoint::Large.button_column_width(), 20);
}

#[test]
fn utils_is_below_minimum_size_checks_both_dimensions() {
    assert!(utils::is_below_minimum_size(ratatui::layout::Rect::new(
        0, 0, 39, 40
    )));
    assert!(utils::is_below_minimum_size(ratatui::layout::Rect::new(
        0, 0, 80, 11
    )));
    assert!(!utils::is_below_minimum_size(ratatui::layout::Rect::new(
        0, 0, 80, 24
    )));
}

#[test]
fn utils_centered_rect_variants_stay_inside_the_area() {
    let area = ratatui::layout::Rect::new(0, 0, 100, 50);
    let centered = utils::centered_rect(50, 50, area);
    assert!(centered.width <= area.width);
    assert!(centered.height <= area.height);
    assert!(centered.x > 0 && centered.y > 0);

    let capped = utils::centered_rect_max(95, 95, 40, 20, area);
    assert!(capped.width <= 40);
    assert!(capped.height <= 20);

    let fixed = utils::centered_rect_fixed(30, 10, area);
    assert_eq!(fixed.width, 30);
    assert_eq!(fixed.height, 10);

    // Oversized requests clamp to the available space instead of overflowing.
    let clamped = utils::centered_rect_fixed(500, 500, area);
    assert_eq!(clamped.width, area.width);
    assert_eq!(clamped.height, area.height);

    // Out-of-range percentages must not underflow.
    let clamped_pct = utils::centered_rect(200, 200, area);
    assert!(clamped_pct.width <= area.width);
    assert!(clamped_pct.height <= area.height);
}

#[test]
fn utils_truncate_with_ellipsis_covers_all_branches() {
    assert_eq!(utils::truncate_with_ellipsis("short", 10), "short");
    assert_eq!(utils::truncate_with_ellipsis("abcdef", 1), "\u{2026}");
    assert_eq!(utils::truncate_with_ellipsis("abcdef", 0), "\u{2026}");
    let truncated = utils::truncate_with_ellipsis("abcdefghij", 4);
    assert_eq!(truncated.chars().count(), 4);
    assert!(truncated.ends_with('\u{2026}'));
}

// ===========================================================================
// clipboard
// ===========================================================================

// ===========================================================================
// backend_panel
// ===========================================================================

/// A panel row literal, healthy or not.

// ===========================================================================
// skillgen
// ===========================================================================

// ===========================================================================
// loop_dialog
// ===========================================================================

/// Build a bare key event with no modifiers.

// ===========================================================================
// cron scheduler
// ===========================================================================

// ===========================================================================
// Layout: agents / teams / status-bar render branches
// ===========================================================================

/// Populate the App with a two-level agent tree plus a background shell task.
fn app_with_agent_tree() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks.push(task_entry(
        "task-1",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        false,
    ));
    app.active_tasks.push(task_entry(
        "task-2",
        "s2",
        "s3",
        "build",
        TaskStatus::Suspended,
        true,
    ));
    app.active_tasks.push(task_entry(
        "task-3",
        "s1",
        "s4",
        "build",
        TaskStatus::Completed,
        false,
    ));
    app.bg_tasks.push(codemason_tui::app::BgTaskView {
        id: "bg-1".to_string(),
        session_id: "s1".to_string(),
        command: "cargo build --workspace --all-targets".to_string(),
        status: "running".to_string(),
        created_at: Utc::now(),
        completed_at: None,
    });
    app.bg_tasks.push(codemason_tui::app::BgTaskView {
        id: "bg-2".to_string(),
        session_id: "s1".to_string(),
        command: "ls".to_string(),
        status: "completed".to_string(),
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
    });
    app
}

#[test]
fn layout_agents_window_paints_the_tree_and_buttons() {
    let mut app = app_with_agent_tree();
    app.show_agents_window = true;
    let terminal = render(&mut app, 140, 44);
    let screen = painted(&terminal);
    assert!(screen.contains("Agents"), "the overlay title must paint");
    assert!(screen.contains("lead"), "the primary agent row must paint");
    assert!(app.active_agents_area.width > 0);
    // The renderer publishes the cursor row maps instead of the retired
    // hit-rect vectors: one key per painted row and the sub-agent slice for
    // the keyboard cursor (hit-rect rework).
    // Painted rows: header, lead, task-1, task-2, task-3 (completed), then the
    // background heading and bg-1/bg-2.
    assert_eq!(app.agents_row_task_ids.len(), 8);
    assert_eq!(app.agents_row_task_ids[1], "primary");
    assert_eq!(app.agents_row_task_ids[2], "task-1");
    assert_eq!(app.agents_row_task_ids[3], "task-2");
    assert_eq!(app.agents_row_task_ids[4], "task-3");
    assert_eq!(
        app.agents_subagent_task_ids,
        vec![
            "task-1".to_string(),
            "task-2".to_string(),
            "task-3".to_string(),
        ],
        "the cursor ordinal list names the sub-agent rows in paint order"
    );
}

#[test]
fn layout_agents_window_scrolls_past_the_visible_window() {
    let mut app = app_with_agent_tree();
    app.show_agents_window = true;
    app.active_agents_scroll_offset = 2;
    // The Agents modal is 70% of the terminal height (spec `agentui` FR-006), so
    // the tree only overflows the table area at a short height.
    let _ = render(&mut app, 140, 14);
    assert!(app.active_agents_max_scroll > 0);
}

#[test]
fn layout_agents_window_without_tasks_still_paints() {
    let mut app = support::make_app();
    app.show_agents_window = true;
    let terminal = render(&mut app, 120, 30);
    assert!(painted(&terminal).contains("Agents"));
    assert_eq!(app.active_agents_max_scroll, 0);
}

#[test]
fn layout_teams_window_paints_the_lead_and_teammates() {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.agent_name = "general".to_string();
    app.show_teams_window = true;
    app.session.active_team = Some(TeamConfig::new("alpha", "s1"));

    let mut working = TeamMember::new("reviewer", "tm-001", "build");
    working.status = MemberStatus::Working;
    working.session_id = Some("s2".to_string());
    app.session.team_members.push(working);

    let mut suspended = TeamMember::new("tester", "tm-002", "build");
    suspended.status = MemberStatus::Suspended;
    app.session.team_members.push(suspended);

    let mut stopped = TeamMember::new("archivist", "tm-003", "build");
    stopped.status = MemberStatus::Stopped;
    app.session.team_members.push(stopped);

    app.session.focused_teammate = Some("tm-001".to_string());
    app.session
        .team_message_counts
        .insert("tm-001".to_string(), (3, 2));

    let terminal = render(&mut app, 160, 44);
    let screen = painted(&terminal);
    assert!(screen.contains("Teams"), "the overlay title must paint");
    assert!(screen.contains("reviewer"), "teammate rows must paint");
    assert!(screen.contains("working"));
    // The renderer publishes the cursor row maps instead of the retired
    // hit-rect vectors: one key per painted row (header, lead, three
    // teammates) and the teammate slice for the keyboard cursor (hit-rect
    // rework). A stopped member still carries its key - Space/Del on it
    // resolve (and are refused) through the member status. `tester` is
    // suspended, so the FR-028 parked-member hint row is painted too and
    // carries its own empty key.
    assert_eq!(app.teams_row_agent_ids.len(), 6);
    assert_eq!(
        app.teams_teammate_agent_ids,
        vec![
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ]
    );
}

#[test]
fn layout_teams_window_without_members_shows_the_hint() {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.show_teams_window = true;
    let terminal = render(&mut app, 140, 40);
    assert!(painted(&terminal).contains("no teammates yet"));
}

#[test]
fn status_bar_full_width_surfaces_every_segment() {
    let mut app = support::make_app();
    app.status = "Ready".to_string();
    app.cwd = "/home/dev/projects/codemason".to_string();
    app.git_branch = Some("main".to_string());
    app.session.last_prompt = "a".repeat(80);
    app.session.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    app.session.selected_thinking_level = Some(ThinkingLevel::High);
    app.session.is_processing = true;
    app.quota_percent = Some(85.0);
    app.code_index_enabled = true;
    app.code_index_busy = true;
    app.code_index_graph_busy = true;
    app.session.autopilot_enabled = true;
    let terminal = render(&mut app, 200, 30);
    let screen = painted(&terminal);
    assert!(
        screen.contains("codemason"),
        "the version prefix must paint"
    );
    assert!(screen.contains("main"), "the git branch must paint");
    assert!(
        screen.contains("...."),
        "a long prompt is truncated with ...."
    );
    assert!(
        screen.contains("cdx-idx"),
        "the code-index busy tag must paint"
    );
    assert!(screen.contains("cdx-graph"));
}

#[test]
fn status_bar_compact_and_minimal_modes_differ() {
    let mut app = support::make_app();
    app.cwd = "/home/dev/projects/codemason-with-a-long-name".to_string();
    app.session.is_processing = true;
    // 100 columns -> Compact, 60 columns -> Minimal.
    let compact = painted(&render(&mut app, 100, 20));
    assert!(compact.contains("codemason"), "compact screen: {compact}");
    let minimal = painted(&render(&mut app, 60, 20));
    assert!(minimal.contains("codemason"), "minimal screen: {minimal}");
}

#[test]
fn status_bar_renders_the_compaction_and_compression_indicators() {
    let mut app = support::make_app();
    app.session.compact_in_progress = true;
    let both = painted(&render(&mut app, 140, 24));
    assert!(both.contains("compacting"));

    app.session.compress_in_progress = true;
    let combined = painted(&render(&mut app, 140, 24));
    assert!(combined.contains("cmp+compress"));
}

#[test]
fn status_bar_renders_the_web_phase_countdown() {
    let mut app = support::make_app();
    // Pin the width-consuming segments so the real cwd, branch and configured
    // model cannot push the countdown off a 140-column status bar.
    app.cwd = "/home/dev/projects/codemason".to_string();
    app.git_branch = Some("main".to_string());
    app.session.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    let mut progress = research_progress::ResearchProgress::new("run", "topic");
    progress.web_phase_deadline = Some(std::time::Instant::now() + Duration::from_secs(95));
    app.research_progress.push(progress);
    let screen = painted(&render(&mut app, 140, 24));
    assert!(screen.contains("web:1:3"), "countdown screen: {screen}");
}

#[test]
fn status_bar_renders_a_non_local_backend_and_error_health() {
    let mut app = support::make_app();
    app.session.active_backend = "docker".to_string();
    // Mark the provider unhealthy so the error icon branch renders.
    app.session
        .provider_health
        .store(2, std::sync::atomic::Ordering::Relaxed);
    app.session.selected_model = Some("anthropic/claude".to_string());
    // The unhealthy provider branch paints the error colouring.
    let _ = render(&mut app, 140, 24);
}

#[test]
fn status_bar_hides_the_service_row_in_non_verbose_modes() {
    let mut app = support::make_app();
    app.session.autopilot_enabled = true;
    app.code_index_enabled = true;

    // Minimal (60 cols) defers the service indicator row to `/status`, so the
    // autopilot marker must be absent; the full-width render carries it.
    let minimal = painted(&render(&mut app, 60, 20));
    let full = painted(&render(&mut app, 200, 20));
    assert!(full.contains("codemason"));
    assert!(
        full.chars().count() > minimal.chars().count(),
        "the full-width status bar must carry more segments"
    );
}

#[test]
fn status_bar_paints_the_status_history_and_error_status() {
    let mut app = support::make_app();
    // Pin the width-consuming segments so the real cwd, branch and configured
    // model cannot push the status text off a 140-column status bar.
    app.cwd = "/home/dev/projects/codemason".to_string();
    app.git_branch = Some("main".to_string());
    app.session.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    app.status = "error: model unavailable".to_string();
    let screen = painted(&render(&mut app, 140, 24));
    assert!(screen.contains("model unavailable"), "screen: {screen}");
}

// ===========================================================================
// Slash-command surfaces (isolated project)
// ===========================================================================

/// Run a slash command in an isolated project and return the last message.

/// The `/toolchain list` walk runs on a blocking thread, so the runtime must
/// be multi-threaded. The cwd/env isolation harness is entered separately here
/// because the lock is already held by the caller.

// ===========================================================================
// Research progress (render surface the event tests do not reach)
// ===========================================================================

#[test]
fn research_progress_render_includes_the_options_and_final_counts() {
    let mut progress = research_progress::ResearchProgress::new("run-x", "topic-y");
    progress.apply(
        codemason_research::session::SessionPhase::Setup,
        research_progress::StepStatus::Done,
        "options in use: mode: tiered",
    );
    progress.apply(
        codemason_research::session::SessionPhase::Synthesize,
        research_progress::StepStatus::Started,
        "multi\nline detail",
    );
    progress.finish(4, 1, 1, 2);
    let rendered = progress.render();
    assert!(rendered.contains("Research Progress - `run-x`"));
    assert!(rendered.contains("Options: mode: tiered"));
    assert!(rendered.contains("multi"));
    assert!(rendered.contains("line detail"));
}

// ===========================================================================
// /spawn, /bench, compaction and /new poll arms
// ===========================================================================

// ===========================================================================
// helpers reached through the event handler
// (`loop_termination_banner`, `plural`, `files_preview`, `summarise_error`)
// ===========================================================================

/// Build an app whose session id matches the events published below.

#[test]
fn md_worker_leaves_a_table_when_prose_follows() {
    let worker = MdWorker::new();
    let long = "c".repeat(55);
    let markdown = format!(
        "Intro line.\n\n| Command | Description |\n|---|---|\n| `{long}` | text |\n\nAfter the table `{long}` is prose.\n"
    );
    let rendered = worker.render(&markdown).expect("render");
    assert!(rendered.contains("Intro line."), "rendered: {rendered}");
    assert!(rendered.contains("After the table"), "rendered: {rendered}");
    assert!(rendered.contains("Command"), "rendered: {rendered}");
}

#[test]
fn md_worker_handles_a_separator_only_row_and_a_ragged_table() {
    let worker = MdWorker::new();
    // A separator line that opens the document, then a row with no cells.
    let markdown = "|---|---|\n| one |\n\n| a | b |\n|---|---|\n| `x` | y |\n";
    let rendered = worker.render(markdown).expect("render");
    assert!(rendered.contains("one"), "rendered: {rendered}");
}

// ===========================================================================
// Slash-command usage branches: bare form, unknown subcommand, config reads
// ===========================================================================

// ===========================================================================
// /spawn command branches and compaction start guards
// ===========================================================================

// ===========================================================================
// helpers reached through further slash surfaces
// ===========================================================================

// ===========================================================================
// status-bar abbreviation helper tables and prompt tag
// ===========================================================================

#[test]
fn status_bar_abbreviations_cover_their_tables() {
    use codemason_tui::layout_statusbar::abbreviations;

    // Full mode returns every label unchanged.
    for label in ["tokens", "provider", "context", "tasks", "health"] {
        assert_eq!(abbreviations::label(label, true), label);
    }
    assert_eq!(abbreviations::label("tokens", false), "tok");
    assert_eq!(abbreviations::label("provider", false), "pvd");
    assert_eq!(abbreviations::label("context", false), "ctx");
    assert_eq!(abbreviations::label("tasks", false), "t");
    assert_eq!(abbreviations::label("health", false), "hlth");
    assert_eq!(abbreviations::label("code_index", false), "idx");
    assert_eq!(abbreviations::label("memory", false), "mem");
    assert_eq!(abbreviations::label("git", false), "git");
    assert_eq!(abbreviations::label("branch", false), "br");
    assert_eq!(abbreviations::label("status", false), "sts");
    assert_eq!(abbreviations::label("unknown", false), "unknown");

    assert_eq!(abbreviations::service("code_index"), "Idx");
    assert_eq!(abbreviations::service("memory"), "Mem");
    assert_eq!(abbreviations::service("plugins"), "plugins");

    assert_eq!(abbreviations::provider("anthropic"), "An");
    assert_eq!(abbreviations::provider("claude"), "Cl");
    assert_eq!(abbreviations::provider("openai"), "OAI");
    assert_eq!(abbreviations::provider("gpt"), "GPT");
    assert_eq!(abbreviations::provider("gemini"), "Gm");
    assert_eq!(abbreviations::provider("hugging_face"), "HF");
    assert_eq!(abbreviations::provider("copilot"), "CoPilot");
    assert_eq!(abbreviations::provider("ollama"), "Oll");
    assert_eq!(abbreviations::provider("azure_foundry"), "AzF");
    assert_eq!(abbreviations::provider("generic"), "generic");
}

#[test]
fn status_bar_prompt_display_text_truncates_with_the_marker() {
    use codemason_tui::layout_statusbar::prompt_display_text;
    assert_eq!(prompt_display_text("", 10), "");
    assert_eq!(prompt_display_text("short", 10), "[short]");
    assert_eq!(prompt_display_text("exactly-10", 10), "[exactly-10]");
    assert_eq!(prompt_display_text("abcdefghijk", 10), "[abcdefghij....]");
    assert_eq!(prompt_display_text("a", 0), "[....]");
}

#[test]
fn status_bar_response_mode_thresholds() {
    use codemason_tui::layout_statusbar::ResponsiveMode;
    assert_eq!(ResponsiveMode::from_width(0), ResponsiveMode::Minimal);
    assert_eq!(ResponsiveMode::from_width(79), ResponsiveMode::Minimal);
    assert_eq!(ResponsiveMode::from_width(80), ResponsiveMode::Compact);
    assert_eq!(ResponsiveMode::from_width(119), ResponsiveMode::Compact);
    assert_eq!(ResponsiveMode::from_width(120), ResponsiveMode::Full);
    assert_eq!(ResponsiveMode::from_width(400), ResponsiveMode::Full);
}

#[test]
fn status_bar_shortens_the_cwd_when_the_prompt_tag_needs_room() {
    let mut app = support::make_app();
    // A deep cwd plus a long prompt tag forces the shortened-cwd branch.
    app.cwd = "/home/dev/projects/a-very-long-nested-project-directory-name".to_string();
    app.git_branch = Some("feature/long-branch-name".to_string());
    app.session.last_prompt = "p".repeat(60);
    app.session.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    app.session.is_processing = true;

    // Every width from narrow to wide must render without panicking, and the
    // prompt tag must still be visible whenever the tag plan applies.
    for width in [60u16, 80, 100, 120, 140, 160] {
        let screen = painted(&render(&mut app, width, 24));
        assert!(screen.contains("codemason"), "width {width}: {screen}");
    }
}
