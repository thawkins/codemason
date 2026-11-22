//! Coverage tests for `codemason-tui` modules that are otherwise lightly exercised
//! by the existing suite:
//!
//! * `app::models` - markdown rendering (`render_markdown_unconditionally`) and
//!   the model-list fallbacks reachable through the public `App` surface.
//! * `app::state` - the shared state helpers (snapshot maths, role modes, pane
//!   cycles, the plugin-store browser navigators) and the pure `App` poll/
//!   housekeeping helpers.
//! * `app::cron` - the background scheduler handle lifecycle (`start`/`stop`).
//! * `theme` - theme selection, colour/style lookup, spinner and progress-bar
//!   helpers, status categories/messages/history.
//! * `app::reverse` - the `/reverse` text grammar through the public test hooks.
//! * `widgets::button` - the button and button-bar widgets rendered through
//!   `ratatui::backend::TestBackend`.
//!
//! Tests only - no file under `crates/*/src/` is touched. Every cargo command is
//! run with an isolated `XDG_CONFIG_HOME` (see the task harness), so nothing here
//! reads or writes the developer's real `~/.config/codemason`.
#![allow(unsafe_code)]
#![allow(clippy::await_holding_lock)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use codemason_agent::message::Message;
use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_app_core::app::{
    ContextPartitionSnapshot, LogLevel, MemoryViewState, ModelLoadingState, OutputViewLineCache,
    OutputViewState, OutputViewTarget, PluginStoreBrowser, PluginStoreStatus, QuestionRequest,
    ResearchViewState, RoleMode, SelectionPane, StorePane, TextSelection, resolve_default_index,
};
use codemason_app_core::status::{StatusCategory, StatusHistory, StatusMessage};
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_connectors::CategoryFilter;
use codemason_plugins::{ManifestMeta, ManifestUpdate, StoreEntry};

#[path = "support/mod.rs"]
mod support;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn now() -> Instant {
    Instant::now()
}

fn running_task(id: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "parent".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: "explore".to_string(),
        task_prompt: "do the thing".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

fn entry(id: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: format!("Name {id}"),
        version: String::new(),
        source: format!("dir:{id}"),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        category: None,
        marketplace: String::new(),
    }
}

fn snapshot() -> ContextPartitionSnapshot {
    ContextPartitionSnapshot {
        system_prompt_tokens: 100,
        tool_catalog_tokens: 200,
        tool_metadata_tokens: 50,
        history_tokens: 400,
        history_message_count: 7,
        skills_tokens: 10,
        memory_tokens: 5,
        agents_md_tokens: 3,
        context_window_tokens: Some(1000),
        last_input_tokens: 42,
        items: Default::default(),
    }
}

// ===========================================================================
// theme.rs
// ===========================================================================

#[test]
fn theme_mode_from_str_and_display_name() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let themes = ThemeCatalog::builtin();
    let high_contrast = Some(ThemeSelection::Named("high-contrast".to_string()));
    assert_eq!(themes.parse("default"), Some(ThemeSelection::System));
    assert_eq!(themes.parse("SYSTEM"), Some(ThemeSelection::System));
    assert_eq!(themes.parse("high-contrast"), high_contrast);
    assert_eq!(themes.parse("high_contrast"), high_contrast);
    assert_eq!(themes.parse("HighContrast"), high_contrast);
    assert_eq!(themes.parse("nope"), None);
    assert_eq!(ThemeSelection::default(), ThemeSelection::System);
    assert_eq!(ThemeSelection::System.display_name(), "system");
    assert_eq!(
        ThemeSelection::Named("nord".to_string()).display_name(),
        "nord"
    );
}

#[test]
fn theme_status_message_constructors_and_accessors() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let m = StatusMessage::new("base", StatusCategory::Info);
    assert!(m.formatted().contains("base"));
    // age_secs on a just-created message is 0.
    assert_eq!(m.age_secs(), 0);

    assert!(StatusMessage::info("a").formatted().contains('a'));
    assert!(StatusMessage::success("b").formatted().contains('b'));
    assert!(StatusMessage::warning("c").formatted().contains('c'));
    assert!(StatusMessage::error("d").formatted().contains('d'));
    assert!(StatusMessage::working("e").formatted().contains('e'));
}

#[test]
fn theme_status_history_push_evict_recent_filter_clear() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut history = StatusHistory::with_capacity(3);
    assert!(history.is_empty());
    assert_eq!(history.len(), 0);

    history.push(StatusMessage::info("one"));
    history.push(StatusMessage::error("two"));
    history.push(StatusMessage::info("three"));
    // The fourth push evicts the oldest (capacity 3).
    history.push(StatusMessage::warning("four"));
    assert_eq!(history.len(), 3);
    assert!(!history.is_empty());
    assert_eq!(history.messages().len(), 3);

    let recent = history.recent(2);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].message, "four");
    assert_eq!(recent[1].message, "three");

    let infos = history.by_category(StatusCategory::Info);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].message, "three");

    history.clear();
    assert!(history.is_empty());

    // The default-constructed history uses the 100-entry cap.
    let default_history = StatusHistory::new();
    assert!(default_history.is_empty());
    assert_eq!(default_history.recent(5).len(), 0);
}

// ===========================================================================
// widgets/button.rs
// ===========================================================================

// ===========================================================================
// app/state.rs - pure helpers
// ===========================================================================

#[test]
fn state_context_partition_snapshot_maths() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let snap = snapshot();
    assert_eq!(snap.total_tokens(), 750); // 100 + 200 + 50 + 400
    assert_eq!(snap.remaining_tokens(), Some(250));
    assert_eq!(snap.percent_of_window(500), Some(50.0));
    // Clamped at 100%.
    assert_eq!(snap.percent_of_window(10_000), Some(100.0));
    assert_eq!(snap.total_percent(), Some(75.0));

    // Unknown window: every percentage helper is None.
    let mut unknown = snap.clone();
    unknown.context_window_tokens = None;
    assert_eq!(unknown.remaining_tokens(), None);
    assert_eq!(unknown.percent_of_window(10), None);
    assert_eq!(unknown.total_percent(), None);

    // Zero window: percent is None but remaining saturates at 0.
    let mut zero = snap.clone();
    zero.context_window_tokens = Some(0);
    assert_eq!(zero.percent_of_window(10), None);
    assert_eq!(zero.remaining_tokens(), Some(0));
    // Saturation: a window smaller than the estimate yields 0.
    let mut tiny = snap;
    tiny.context_window_tokens = Some(10);
    assert_eq!(tiny.remaining_tokens(), Some(0));
}

#[test]
fn state_resolve_default_index_clamps() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(resolve_default_index(3, Some(1)), 1);
    assert_eq!(resolve_default_index(3, Some(9)), 2);
    assert_eq!(resolve_default_index(3, None), 0);
    assert_eq!(resolve_default_index(0, Some(5)), 0);

    let req = QuestionRequest {
        id: "q1".to_string(),
        session_id: "s1".to_string(),
        question: "pick".to_string(),
        options: vec!["a".to_string(), "b".to_string()],
        default_index: Some(1),
        expire_at_ms: None,
        secret: false,
    };
    assert_eq!(req.resolved_default_index(), 1);
}

#[test]
fn state_role_mode_all_methods() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    for (mode, label) in [
        (RoleMode::Architect, "architect"),
        (RoleMode::Coder, "coder"),
        (RoleMode::Reviewer, "reviewer"),
        (RoleMode::Debugger, "debugger"),
        (RoleMode::Tester, "tester"),
    ] {
        assert_eq!(mode.label(), label);
        assert_ne!(mode.icon(), "");
        assert_ne!(mode.system_prompt_addition(), "");
    }
    assert_eq!(RoleMode::from_str("Architect"), Some(RoleMode::Architect));
    assert_eq!(RoleMode::from_str("CODER"), Some(RoleMode::Coder));
    assert_eq!(RoleMode::from_str("reviewer"), Some(RoleMode::Reviewer));
    assert_eq!(RoleMode::from_str("debugger"), Some(RoleMode::Debugger));
    assert_eq!(RoleMode::from_str("tester"), Some(RoleMode::Tester));
    assert_eq!(RoleMode::from_str("unknown"), None);
}

#[test]
fn state_store_pane_cycles_both_ways() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(StorePane::default(), StorePane::Results);
    assert_eq!(StorePane::Results.next(), StorePane::Categories);
    assert_eq!(StorePane::Categories.next(), StorePane::Marketplaces);
    assert_eq!(StorePane::Marketplaces.next(), StorePane::Results);

    assert_eq!(StorePane::Results.previous(), StorePane::Marketplaces);
    assert_eq!(StorePane::Marketplaces.previous(), StorePane::Categories);
    assert_eq!(StorePane::Categories.previous(), StorePane::Results);
}

#[test]
fn state_text_selection_normalized_orders_endpoints() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let reversed = TextSelection {
        pane: SelectionPane::Messages,
        anchor: (10, 5),
        endpoint: (3, 2),
    };
    assert_eq!(reversed.normalized(), ((3, 2), (10, 5)));

    let forward = TextSelection {
        pane: SelectionPane::Log,
        anchor: (1, 1),
        endpoint: (9, 4),
    };
    assert_eq!(forward.normalized(), ((1, 1), (9, 4)));

    // Same row: ordered by column.
    let same_row = TextSelection {
        pane: SelectionPane::Input,
        anchor: (8, 7),
        endpoint: (2, 7),
    };
    assert_eq!(same_row.normalized(), ((2, 7), (8, 7)));
}

#[test]
fn state_scroll_view_states() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut out = OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s".to_string(),
            label: "L".to_string(),
        },
        scroll_offset: 5,
        max_scroll: 20,
        line_cache: OutputViewLineCache::default(),
    };
    out.scroll_by(10);
    assert_eq!(out.scroll_offset, 15);
    out.scroll_by(100); // clamps at max_scroll
    assert_eq!(out.scroll_offset, 20);
    out.scroll_by(-5);
    assert_eq!(out.scroll_offset, 15);
    out.jump_start();
    assert_eq!(out.scroll_offset, 20);
    out.jump_end();
    assert_eq!(out.scroll_offset, 0);
    // A negative delta larger than the offset saturates at 0.
    out.scroll_by(-100);
    assert_eq!(out.scroll_offset, 0);

    let mut research = ResearchViewState {
        name: "r".to_string(),
        path: std::path::PathBuf::from("/tmp/RESEARCH.md"),
        base_dir: std::path::PathBuf::from("/tmp"),
        markdown: "# body".to_string(),
        scroll_offset: 0,
        max_scroll: 8,
        line_cache: OutputViewLineCache::default(),
    };
    research.scroll_by(3);
    assert_eq!(research.scroll_offset, 3);
    research.jump_start();
    assert_eq!(research.scroll_offset, 8);
    research.jump_end();
    assert_eq!(research.scroll_offset, 0);

    let mut memory = MemoryViewState {
        row: codemason_storage::storage::MemoryRow {
            id: 1,
            content: "note".to_string(),
            category: "fact".to_string(),
            source: "test".to_string(),
            confidence: 0.9,
            project: "p".to_string(),
            session_id: "s".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            access_count: 0,
            last_accessed: None,
        },
        scroll_offset: 2,
        max_scroll: 6,
        line_cache: OutputViewLineCache::default(),
    };
    memory.scroll_by(10);
    assert_eq!(memory.scroll_offset, 6);
    memory.scroll_by(-2);
    assert_eq!(memory.scroll_offset, 4);
    memory.jump_start();
    assert_eq!(memory.scroll_offset, 6);
    memory.jump_end();
    assert_eq!(memory.scroll_offset, 0);
}

#[test]
fn state_is_image_path_and_percent_decode() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    use codemason_app_core::app::{is_image_path, percent_decode_path};
    assert!(is_image_path(std::path::Path::new("a.PNG")));
    assert!(is_image_path(std::path::Path::new("a.jpeg")));
    assert!(is_image_path(std::path::Path::new("a.webp")));
    assert!(is_image_path(std::path::Path::new("a.tiff")));
    assert!(!is_image_path(std::path::Path::new("a.txt")));
    assert!(!is_image_path(std::path::Path::new("noext")));

    assert_eq!(
        percent_decode_path("a%20b"),
        std::path::PathBuf::from("a b")
    );
    assert_eq!(
        percent_decode_path("plain"),
        std::path::PathBuf::from("plain")
    );
    // A truncated percent escape is passed through untouched.
    assert_eq!(percent_decode_path("a%2"), std::path::PathBuf::from("a%2"));
    // A non-hex escape is passed through untouched.
    assert_eq!(
        percent_decode_path("a%ZZ"),
        std::path::PathBuf::from("a%ZZ")
    );
}

#[test]
fn state_atomic_config_update_reads_parses_and_reports_errors() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("codemason.json");
    std::fs::write(&path, r#"{"a":1}"#).expect("seed");

    codemason_app_core::app::atomic_config_update(&path, |json| {
        json["b"] = serde_json::json!(2);
        Ok(())
    })
    .expect("update");
    let raw = std::fs::read_to_string(&path).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse");
    assert_eq!(parsed["a"], 1);
    assert_eq!(parsed["b"], 2);

    // The updater error is surfaced.
    let err = codemason_app_core::app::atomic_config_update(&path, |_json| Err("boom".to_string()))
        .expect_err("updater error propagates");
    assert_eq!(err, "boom");

    // A missing file starts from an empty object; a malformed file errors.
    let missing = dir.path().join("new.json");
    codemason_app_core::app::atomic_config_update(&missing, |json| {
        json["c"] = serde_json::json!(true);
        Ok(())
    })
    .expect("create");
    assert!(std::fs::read_to_string(&missing).unwrap().contains("\"c\""));

    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, "{ not json").expect("seed bad");
    let err = codemason_app_core::app::atomic_config_update(&bad, |_json| Ok(()))
        .expect_err("parse error");
    assert!(err.contains("parse"), "error was: {err}");
}

#[test]
fn state_log_level_equality() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    // `LogLevel` is an ordinary derived enum, so asserting `Info == Info` and
    // `Warn != Error` only re-tests `#[derive(PartialEq)]`. The behaviour that
    // actually matters is that the four levels are distinct and exhaustive:
    // a log window that filtered on a collapsed level would hide or duplicate
    // entries, and a new variant must force a decision here rather than
    // silently compare equal to an existing one.
    let all = [
        LogLevel::Info,
        LogLevel::Tool,
        LogLevel::Warn,
        LogLevel::Error,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            assert_eq!(
                a == b,
                i == j,
                "levels {a:?} and {b:?} must be equal only when they are the same level"
            );
        }
    }
}

#[test]
fn state_plugin_browser_query_and_filter() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut browser = PluginStoreBrowser::new(codemason_plugins::StoreKind::Codex, "", false);
    assert_eq!(browser.kind, codemason_plugins::StoreKind::Codex);
    assert_eq!(browser.status, PluginStoreStatus::Loading);
    assert!(!browser.has_results());
    assert!(browser.selected().is_none());

    browser.set_entries(vec![entry("weather"), entry("calendar")]);
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert!(browser.has_results());
    assert_eq!(browser.selected().map(|e| e.id.as_str()), Some("calendar"));

    // Query narrows the set; backspace restores it.
    browser.set_query("weath".to_string());
    assert_eq!(browser.filtered.len(), 1);
    browser.push_char('x');
    assert_eq!(browser.filtered.len(), 0);
    assert!(browser.backspace());
    assert_eq!(browser.filtered.len(), 1);
    // Backspacing to empty re-matches everything.
    for _ in 0..6 {
        browser.backspace();
    }
    assert_eq!(browser.filtered.len(), 2);
    assert!(!browser.backspace());

    // Cursor move and visibility bookkeeping.
    browser.move_down();
    assert_eq!(browser.cursor, 1);
    browser.move_down(); // clamped
    assert_eq!(browser.cursor, 1);
    browser.move_up();
    assert_eq!(browser.cursor, 0);
    browser.ensure_visible(0);
    assert_eq!(browser.scroll, 0);
}

#[test]
fn state_plugin_browser_prefill_and_status_transitions() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let browser = PluginStoreBrowser::new(codemason_plugins::StoreKind::Claude, "cal", true);
    assert_eq!(browser.query, "cal");
    assert!(browser.refresh);

    let mut failed = PluginStoreBrowser::new(codemason_plugins::StoreKind::Claude, "", false);
    failed.set_failed("network down".to_string());
    assert_eq!(
        failed.status,
        PluginStoreStatus::Failed("network down".to_string())
    );

    // An empty index yields the Empty status.
    let mut empty = PluginStoreBrowser::new(codemason_plugins::StoreKind::Codex, "", false);
    empty.set_entries(Vec::new());
    assert_eq!(empty.status, PluginStoreStatus::Empty);
}

#[test]
fn state_plugin_browser_installed_set() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut browser = PluginStoreBrowser::new(codemason_plugins::StoreKind::Codex, "", false);
    let mut installed = std::collections::BTreeSet::new();
    installed.insert("weather".to_string());
    browser.set_installed(installed);
    assert!(browser.is_installed("weather"));
    assert!(!browser.is_installed("absent"));
}

#[test]
fn state_plugin_browser_manifest_enrichment() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut browser = PluginStoreBrowser::new(codemason_plugins::StoreKind::Codex, "", false);
    browser.set_entries(vec![entry("weather"), entry("filled")]);

    // Pre-fill one entry so it is not re-enriched.
    for e in &mut browser.all {
        if e.id == "filled" {
            e.version = "1.0".to_string();
            e.description = "already here".to_string();
        }
    }

    browser.apply_manifest_update(ManifestUpdate {
        id: "weather".to_string(),
        meta: ManifestMeta {
            version: Some("2.0".to_string()),
            description: Some("forecast".to_string()),
        },
    });
    let weather = browser.all.iter().find(|e| e.id == "weather").unwrap();
    assert_eq!(weather.version, "2.0");
    assert_eq!(weather.description, "forecast");

    // A no-change update leaves the entry untouched; an unknown id is a no-op.
    browser.apply_manifest_update(ManifestUpdate {
        id: "filled".to_string(),
        meta: ManifestMeta {
            version: Some("9.9".to_string()),
            description: Some("nope".to_string()),
        },
    });
    let filled = browser.all.iter().find(|e| e.id == "filled").unwrap();
    assert_eq!(filled.version, "1.0");
    assert_eq!(filled.description, "already here");

    browser.apply_manifest_update(ManifestUpdate {
        id: "ghost".to_string(),
        meta: ManifestMeta {
            version: Some("1".to_string()),
            description: None,
        },
    });

    browser.finish_enrichment();
    assert!(!browser.enriching);
}

#[test]
fn state_plugin_browser_category_navigator() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut browser = PluginStoreBrowser::new(codemason_plugins::StoreKind::Codex, "", false);
    let mut weather = entry("weather");
    weather.category = Some("Utilities".to_string());
    let mut cal = entry("calendar");
    cal.category = Some("Productivity".to_string());
    browser.set_entries(vec![weather, cal]);

    // Row 0 is ALL, row 1 is the Installed pseudo-category (pluginstores FR-054),
    // and the rest are the sorted categories.
    assert_eq!(browser.category_row_count(), 4);
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(browser.category_row_label(1), "Installed");
    assert_eq!(browser.category_row_display(0), "ALL (2)");
    assert_eq!(browser.category_row_display(1), "Installed (0)");
    assert_eq!(browser.active_category(), "ALL");
    assert_eq!(browser.category_cursor_row(), 0);

    assert!(browser.category_move_down());
    assert_eq!(browser.category_cursor_row(), 1);
    assert_eq!(browser.active_category(), "Installed");
    assert!(browser.category_move_down());
    assert!(browser.category_move_down());
    assert!(!browser.category_move_down()); // clamped at the last row
    assert!(browser.category_move_up());

    // Confirm applies the highlighted row (`Productivity`) as the active filter.
    assert!(browser.category_select_row(3));
    browser.category_confirm();
    assert_ne!(browser.active_category(), "ALL");
    assert_eq!(browser.filtered.len(), 1);

    // Clearing returns to ALL.
    browser.category_clear();
    assert_eq!(browser.active_category(), "ALL");
    assert_eq!(browser.filtered.len(), 2);

    // Wheel scroll and viewport clamping.
    browser.category_scroll_by(5);
    browser.ensure_category_visible(2);
    // A row select beyond the range is rejected.
    assert!(!browser.category_select_row(99));

    // The pending-category path validates once an index is loaded.
    browser.set_pending_category(CategoryFilter::of("Utilities"));
    browser.apply_pending_category();
    assert_eq!(browser.active_category(), "Utilities");
    assert_eq!(browser.filtered.len(), 1);

    // An unknown pending category is reported and leaves the filter in place.
    browser.set_pending_category(CategoryFilter::of("Nonexistent"));
    browser.apply_pending_category();
    assert_eq!(browser.active_category(), "Utilities");
    assert!(
        browser
            .last_install
            .as_deref()
            .is_some_and(|s| s.contains("unknown category")),
        "footer: {:?}",
        browser.last_install
    );

    // A pending `ALL` is a no-op (cleared, never reported).
    browser.set_pending_category(CategoryFilter::all());
    browser.apply_pending_category();
    assert!(browser.pending_category.is_none());
}

#[test]
fn state_plugin_browser_marketplace_navigator() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut browser = PluginStoreBrowser::new(codemason_plugins::StoreKind::Claude, "", false);
    let mut native = entry("a");
    native.marketplace = "Official".to_string();
    let mut third = entry("b");
    third.marketplace = "Community".to_string();
    browser.set_entries(vec![native, third]);

    assert_eq!(browser.marketplace_row_count(), 3);
    assert_eq!(browser.marketplace_row_label(0), "ALL");
    assert!(browser.marketplace_row_display(0).contains("ALL"));
    assert_eq!(browser.active_marketplace(), "ALL");
    assert_eq!(browser.marketplace_cursor_row(), 0);

    assert!(browser.marketplace_move_down());
    assert!(browser.marketplace_move_down());
    assert!(!browser.marketplace_move_down());
    assert!(browser.marketplace_move_up());

    assert!(browser.marketplace_select_row(1));
    browser.marketplace_confirm();
    assert_ne!(browser.active_marketplace(), "ALL");
    assert_eq!(browser.filtered.len(), 1);

    browser.marketplace_clear();
    assert_eq!(browser.active_marketplace(), "ALL");

    browser.marketplace_scroll_by(3);
    browser.ensure_marketplace_visible(2);
    assert!(!browser.marketplace_select_row(99));
    assert_eq!(browser.marketplace_entry_count("ALL"), 2);
    assert_eq!(browser.category_entry_count("ALL"), 2);
}

#[test]
fn state_plugins_list_state_builder() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let state = codemason_app_core::app::plugins_list_state(dir.path(), false, &BTreeMap::new());
    assert!(!state.verbose);
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.feedback.is_none());
}

#[test]
fn state_plugin_commands_for_surface_is_listable() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let cmds = codemason_app_core::app::plugin_commands_for_surface(dir.path());
    // With no plugins present this is an empty list, but the call must return.
    assert!(cmds.len() < 1000);
}

// ===========================================================================
// app/state.rs - App poll/housekeeping helpers
// ===========================================================================

#[test]
fn state_housekeeping_gate_and_interval() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    // Fresh app has jobs_last_poll at construction time; force it stale.
    app.jobs_last_poll = now()
        .checked_sub(Duration::from_millis(500))
        .expect("clock");
    assert!(app.run_housekeeping_if_due());
    let runs = app.housekeeping_runs;
    // Immediately after, the gate is closed.
    assert!(!app.run_housekeeping_if_due());
    assert_eq!(app.housekeeping_runs, runs);
    assert_eq!(
        app.housekeeping_due_at(),
        app.jobs_last_poll + codemason_app_core::app::HOUSEKEEPING_INTERVAL
    );
}

#[test]
fn state_periodic_redraw_and_animation_latches() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    app.model_loading_state = Some(ModelLoadingState {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        started_at: now(),
    });
    assert!(app.periodic_animation_active());
    assert!(app.needs_periodic_redraw());

    // A stale loading latch no longer counts as active.
    app.model_loading_state.as_mut().unwrap().started_at =
        now().checked_sub(Duration::from_secs(200)).expect("clock");
    assert!(!app.periodic_animation_active());

    // A fresh download latch keeps the animation active.
    app.model_loading_state = None;
    app.model_download_state = Some(codemason_app_core::app::ModelDownloadState {
        provider_id: "ollama".to_string(),
        model_id: "qwen".to_string(),
        percent: 10.0,
        started_at: now(),
    });
    assert!(app.periodic_animation_active());

    // Busy code-index flags also drive the animation.
    app.model_download_state = None;
    app.code_index_busy = true;
    assert!(app.periodic_animation_active());
    app.code_index_busy = false;
    app.code_index_graph_busy = true;
    assert!(app.periodic_animation_active());
    app.code_index_graph_busy = false;

    // Autopilot pending-continue also drives it.
    app.session.autopilot_enabled = true;
    app.session.autopilot_pending_continue = Some("go".to_string());
    assert!(app.periodic_animation_active());
}

#[test]
fn state_poll_stale_spinners_reaps_expired_latches() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    app.status = "busy".to_string();

    // A loading latch past its staleness cap is cleared.
    app.model_loading_state = Some(ModelLoadingState {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        started_at: now().checked_sub(Duration::from_secs(200)).expect("clock"),
    });
    app.poll_stale_spinners();
    assert!(app.model_loading_state.is_none());
    assert_eq!(app.status, "ready");

    // A fresh latch is left alone.
    app.status = "busy".to_string();
    app.model_loading_state = Some(ModelLoadingState {
        provider_id: "ollama".to_string(),
        provider_name: "Ollama".to_string(),
        started_at: now(),
    });
    app.poll_stale_spinners();
    assert!(app.model_loading_state.is_some());

    // A download latch past its (longer) cap is cleared too.
    app.model_loading_state = None;
    app.status = "busy".to_string();
    app.model_download_state = Some(codemason_app_core::app::ModelDownloadState {
        provider_id: "ollama".to_string(),
        model_id: "qwen".to_string(),
        percent: 1.0,
        started_at: now().checked_sub(Duration::from_secs(4000)).expect("clock"),
    });
    app.poll_stale_spinners();
    assert!(app.model_download_state.is_none());
    assert_eq!(app.status, "ready");
}

#[test]
fn state_poll_question_expiry_answers_past_deadline() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    app.session.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "s1".to_string(),
        question: "choose".to_string(),
        options: vec!["first".to_string(), "second".to_string()],
        default_index: Some(1),
        expire_at_ms: Some(1), // far in the past
        secret: false,
    });
    app.poll_question_expiry();
    assert!(app.session.question_queue.is_empty());
    assert_eq!(app.session.question_selected_index, 0);

    // A request without a deadline is left untouched.
    app.session.question_queue.push_back(QuestionRequest {
        id: "q2".to_string(),
        session_id: "s1".to_string(),
        question: "free text".to_string(),
        options: Vec::new(),
        default_index: None,
        expire_at_ms: None,
        secret: false,
    });
    app.poll_question_expiry();
    assert_eq!(app.session.question_queue.len(), 1);

    // A deadline in the future leaves the request queued.
    app.session.question_queue[0].expire_at_ms = Some(u64::MAX);
    app.poll_question_expiry();
    assert_eq!(app.session.question_queue.len(), 1);
}

#[test]
fn state_poll_run_cost_banner_expiry() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    app.session.run_cost_banner = Some("$1.23".to_string());
    app.session.run_cost_banner_at = Some(now());
    // Fresh banner is kept.
    app.poll_run_cost_banner_expiry();
    assert!(app.session.run_cost_banner.is_some());

    app.session.run_cost_banner_at =
        Some(now().checked_sub(Duration::from_mins(1)).expect("clock"));
    app.poll_run_cost_banner_expiry();
    assert!(app.session.run_cost_banner.is_none());
    assert!(app.session.run_cost_banner_at.is_none());
}

#[test]
fn state_arm_and_poll_status_expiry_branches() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();

    // [wait] statuses are never auto-armed.
    app.status = "[wait] doing".to_string();
    app.arm_status_expiry();
    assert!(app.status_set_at.is_none());

    // "ready" is the idle state - nothing to arm.
    app.status = "ready".to_string();
    app.arm_status_expiry();
    assert!(app.status_set_at.is_none());

    // A normal status is snapshot-armed, then expires to "ready".
    app.status = "custom".to_string();
    app.arm_status_expiry();
    assert!(app.status_set_at.is_some());
    app.status_set_at = Some(
        now()
            .checked_sub(Duration::from_millis(
                codemason_app_core::app::STATUS_EXPIRY_MS + 10,
            ))
            .expect("clock"),
    );
    app.poll_status_expiry();
    assert_eq!(app.status, "ready");

    // A status changed between arm and poll is not overwritten.
    app.status = "armed".to_string();
    app.arm_status_expiry();
    app.status = "changed".to_string();
    app.status_set_at = Some(
        now()
            .checked_sub(Duration::from_millis(
                codemason_app_core::app::STATUS_EXPIRY_MS + 10,
            ))
            .expect("clock"),
    );
    app.poll_status_expiry();
    assert_eq!(app.status, "changed");
}

#[test]
fn state_apply_registry_reconciliation_merges_both_ways() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    // Track a stale entry that the registry reports as completed.
    app.active_tasks
        .push(running_task("done", TaskStatus::Running));
    // A registry-only running entry must be added.
    let snapshot = vec![
        running_task("done", TaskStatus::Completed),
        running_task("fresh", TaskStatus::Running),
    ];
    app.apply_registry_reconciliation(snapshot);
    let ids: Vec<&str> = app.active_tasks.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"fresh"));
    assert!(!ids.contains(&"done"));
}

#[test]
fn state_poll_active_tasks_reconcile_without_runtime() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    // Force the periodic trigger to fire.
    app.active_tasks_reconcile_last = now().checked_sub(Duration::from_secs(5)).expect("clock");
    app.seen_event_lag = 0;
    app.tui_event_lag = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // No tokio runtime is active here and no AgentManager is set, so the poll
    // takes its guard-and-return paths without panicking.
    app.poll_active_tasks_reconcile();
    assert!(!app.active_tasks_reconcile_inflight);

    // A deposited snapshot is adopted on the next poll.
    *app.active_tasks_reconcile_result.lock().unwrap() =
        Some(vec![running_task("deposited", TaskStatus::Running)]);
    app.poll_active_tasks_reconcile();
    assert!(app.active_tasks.iter().any(|t| t.id == "deposited"));
}

#[test]
fn state_set_tui_event_lag_counter() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(7));
    app.set_tui_event_lag_counter(counter.clone());
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        app.tui_event_lag.load(std::sync::atomic::Ordering::Relaxed),
        8
    );
}

#[test]
fn state_mcp_helpers_without_client() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let app = support::make_app();
    // No shared MCP client is attached, so settling reports true.
    assert!(app.mcp_servers_settled(&app.session_processor));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_mcp_async_helpers_no_client() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    let processor = app.session_processor.clone();
    // Both are non-blocking no-ops with no shared client attached.
    app.adopt_mcp_client_state(&processor).await;
    app.report_mcp_startup().await;
    app.refresh_mcp_enabled_map();
    assert!(!app.mcp_client_adopted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_history_load_save_and_flush() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("history.txt");
    let mut app = support::make_app();
    app.set_history_file(path.clone());
    // The second entry carries a real newline and a literal backslash to test
    // both escape round-trips.
    app.input_history = vec!["one".to_string(), "two\nwith-newline\\slash".to_string()];
    app.save_history().expect("save");
    assert!(path.exists());

    // Reload decodes the escaped newline and backslash back.
    let mut reloaded = support::make_app();
    reloaded.set_history_file(path.clone());
    reloaded.load_history().expect("load");
    assert_eq!(reloaded.input_history.len(), 2);
    assert_eq!(reloaded.input_history[1], "two\nwith-newline\\slash");

    // The async flush writes when due and clears the latch.
    let flush_path = dir.path().join("flush.txt");
    let mut flushing = support::make_app();
    flushing.set_history_file(flush_path.clone());
    flushing.input_history = vec!["queued".to_string()];
    flushing.history_dirty = true;
    flushing.history_save_deadline = None;
    flushing.flush_history_if_due();
    assert!(!flushing.history_dirty);

    // With no history file the latch is cleared to a terminal state.
    let mut nofile = support::make_app();
    nofile.history_dirty = true;
    nofile.history_save_deadline = None;
    nofile.flush_history_if_due();
    assert!(!nofile.history_dirty);
    assert!(nofile.history_save_deadline.is_none());
}

// ===========================================================================
// app/cron.rs - scheduler handle lifecycle
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cron_scheduler_start_stop_and_drop() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let app = support::make_app();
    let workdir = tempfile::tempdir().expect("tempdir");
    let handle = codemason_app_core::app::cron::start_cron_scheduler(
        app.storage.clone(),
        app.session_processor.clone(),
        workdir.path().to_path_buf(),
    );
    // Give the loop a moment to run one tick, then stop and drop it.
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.stop();
    drop(handle);
}

// ===========================================================================
// app/models.rs - markdown rendering reachable through the public surface
// ===========================================================================

#[test]
fn models_render_markdown_keeps_mcp_status_glyphs_for_every_state() {
    // AG-06: `/mcp` labels servers `[green] connected`, `[yellow] needs auth`
    // and `[red] failed: ..`. The markdown pipeline dropped the unclosed
    // `[red]` tag, so failed rows lost their glyph while the others kept it.
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    let rendered = app.render_markdown_unconditionally(
        "From: /mcp\n\n- a: [green] connected\n- b: [yellow] needs auth\n- c: [red] failed: boom\n",
    );
    for glyph in [
        "[green] connected",
        "[yellow] needs auth",
        "[red] failed: boom",
    ] {
        assert!(rendered.contains(glyph), "missing {glyph:?} in: {rendered}");
    }
}

#[test]
fn models_render_markdown_unconditionally_renders_markdown() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    // Plain runtime text is sanitised and passed through with no prefix gate.
    let plain = app.render_markdown_unconditionally("just some text");
    assert!(plain.contains("just some text"));

    // A markdown table renders to an ASCII table without needing the `From: /`
    // prefix the gated entry point requires.
    let table = app.render_markdown_unconditionally("| A | B |\n|---|---|\n| 1 | 2 |");
    assert!(
        table.contains('1') && table.contains('2'),
        "rendered: {table}"
    );
    assert!(table.contains('+') || table.contains('|'));
}

#[test]
fn models_render_markdown_unconditionally_bypasses_research_progress() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    let text = "[research] Research Progress\n\nsome progress";
    let rendered = app.render_markdown_unconditionally(text);
    assert!(rendered.contains("Research Progress"));
}

// ===========================================================================
// app/reverse.rs - the `/reverse` text grammar through the public hooks
// ===========================================================================

#[test]
fn reverse_usage_and_invalid_repo_and_depth() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    // Empty and `help` both render the usage message.
    codemason_app_core::app::reverse_command_for_tests(&mut app, "");
    assert_eq!(app.status, "reverse: usage");
    codemason_app_core::app::reverse_command_for_tests(&mut app, "help");
    assert_eq!(app.status, "reverse: usage");

    // A repo with no recognisable format is rejected before any auth.
    codemason_app_core::app::reverse_command_for_tests(&mut app, "not-a-repo");
    assert_eq!(app.status, "reverse: invalid repo");

    // An unparsable --depth is rejected after the repo resolves.
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --depth abc");
    assert_eq!(app.status, "reverse: invalid depth");
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --depth 0");
    assert_eq!(app.status, "reverse: invalid depth");
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --depth 11");
    assert_eq!(app.status, "reverse: invalid depth");

    // A missing flag value makes the whole parse fail to usage.
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --depth");
    assert_eq!(app.status, "reverse: usage");
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --tech");
    assert_eq!(app.status, "reverse: usage");
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --create");
    assert_eq!(app.status, "reverse: usage");

    // A quoted-empty flag value (`""`) tokenizes as an empty token, which the
    // parser accepts as the repo identifier only when it is the positional.
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World --tech \"\"");
    // The empty tech value parses; the GitHub gate then reports the missing token.
    assert!(
        app.status == "reverse: no token" || app.status == "reverse: usage",
        "status: {}",
        app.status
    );

    // A repo identifier that is a quoted empty string yields no positional.
    codemason_app_core::app::reverse_command_for_tests(&mut app, "\"\"");
    assert_eq!(app.status, "reverse: usage");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_github_no_token_reports_and_stops() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    // `slash_support::enter_project` already takes this binary's crate-wide
    // env lock for the whole body (bound to `_guard`), so this test must NOT
    // take it again: `std::sync::Mutex` is not reentrant and a second
    // acquisition on the same thread self-deadlocks.
    // The no-token gate is reached synchronously, but the reverse path's
    // fetch stage calls `tokio::spawn` on its way out; drive the command from a
    // Tokio runtime context so that spawn has a reactor (see
    // `handle_reverse_parsed`).
    //
    // Env mutation is process-global; serialise against the sibling tests that
    // also touch `cwd`/`XDG_CONFIG_HOME`.
    //
    // The whole thing runs in an isolated project (tempdir cwd + isolated
    // `XDG_CONFIG_HOME`) so the GitHub credential chain never reads the
    // developer's real `~/.config/codemason/github_token`, which would otherwise
    // make the `octocat/Hello-World` case flaky on a machine that has logged in
    // with `/github login`.
    let (_guard, _env, _temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    // `CODEMASON_GITHUB_NO_GH_CLI` keeps `load_token` from spawning the `gh` CLI,
    // so the GitHub path deterministically reports the missing-token error
    // without touching the network or the developer's machine.
    unsafe { std::env::set_var(codemason_config::github::NO_GH_CLI_ENV, "1") };
    let prev_token = std::env::var_os("GITHUB_TOKEN");
    unsafe { std::env::remove_var("GITHUB_TOKEN") };

    let mut app = support::make_app();
    codemason_app_core::app::reverse_command_for_tests(&mut app, "octocat/Hello-World");
    assert_eq!(app.status, "reverse: no token");

    // A GitLab identifier with no token configured reports the GitLab error.
    let prev_gitlab = std::env::var_os("GITLAB_TOKEN");
    unsafe { std::env::remove_var("GITLAB_TOKEN") };
    let mut gitlab_app = support::make_app();
    codemason_app_core::app::reverse_command_for_tests(&mut gitlab_app, "gitlab:group/project");
    assert_eq!(gitlab_app.status, "reverse: no gitlab token");

    unsafe {
        match prev_token {
            Some(v) => std::env::set_var("GITHUB_TOKEN", v),
            None => std::env::remove_var("GITHUB_TOKEN"),
        }
        match prev_gitlab {
            Some(v) => std::env::set_var("GITLAB_TOKEN", v),
            None => std::env::remove_var("GITLAB_TOKEN"),
        }
    }
}

// ===========================================================================
// app/reverse.rs - `/spec reverse` scaffold refusal (isolated project)
// ===========================================================================

#[path = "slash_support/mod.rs"]
mod slash_support;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_spec_scaffold_refusal_reports_and_continues() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    // `slash_support::enter_project` already takes this binary's crate-wide
    // env lock for the whole body (bound to `_guard`), so this test must NOT
    // take it again: `std::sync::Mutex` is not reentrant and a second
    // acquisition on the same thread self-deadlocks.
    // Isolated cwd + XDG_CONFIG_HOME (the guard restores both on drop).
    let (_guard, _env, temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    // Keep the GitHub token resolution offline for the fetch stage that follows.
    let prev_no_gh = std::env::var_os(codemason_config::github::NO_GH_CLI_ENV);
    unsafe { std::env::set_var(codemason_config::github::NO_GH_CLI_ENV, "1") };
    let prev_token = std::env::var_os("GITHUB_TOKEN");
    unsafe { std::env::remove_var("GITHUB_TOKEN") };

    // A target folder that already contains a non-allowlisted file forces the
    // scaffold guard to refuse, exercising `render_scaffold_outcome`'s guard arm.
    let blocked = temp.path().join("occupied");
    std::fs::create_dir_all(&blocked).expect("mkdir");
    std::fs::write(blocked.join("blocker.txt"), "x").expect("write blocker");

    let mut app = slash_support::app_with_session();
    app.execute_slash_command(&format!(
        "/spec reverse octocat/Hello-World --language rust --type cmdline --folder {}",
        blocked.display()
    ))
    .await;

    let all = slash_support::all_messages(&app);
    assert!(
        all.contains("not empty") || all.contains("nothing was scaffolded"),
        "the scaffold refusal is reported: {all}"
    );
    // The run continues past the refusal to the GitHub auth gate.
    assert_eq!(app.status, "reverse: no token");

    unsafe {
        match prev_no_gh {
            Some(v) => std::env::set_var(codemason_config::github::NO_GH_CLI_ENV, v),
            None => std::env::remove_var(codemason_config::github::NO_GH_CLI_ENV),
        }
        match prev_token {
            Some(v) => std::env::set_var("GITHUB_TOKEN", v),
            None => std::env::remove_var("GITHUB_TOKEN"),
        }
    }
}

// A sanity check that `App` and the in-memory message type are wired as the
// other tests assume.
#[test]
fn state_message_append_is_readable() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let mut app = support::make_app();
    app.session.session_id = Some("s".to_string());
    app.session
        .messages
        .push(Message::user_text("s", "hello there".to_string()));
    assert!(
        app.session
            .messages
            .last()
            .unwrap()
            .text_content()
            .contains("hello")
    );
}

/// Restored from `theme_status_category_all_methods`: the app-core
/// `StatusCategory` methods. The terminal's `color()` / `style()` calls and its
/// "Working is bold" check were the ratatui `StatusStyle` trait, deleted with
/// the terminal UI.
#[test]
fn theme_status_category_all_methods() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    for category in [
        StatusCategory::Info,
        StatusCategory::Success,
        StatusCategory::Warning,
        StatusCategory::Error,
        StatusCategory::Working,
    ] {
        assert_ne!(category.icon(), "");
        assert_ne!(category.display_name(), "");
        let formatted = category.format("hello");
        assert!(formatted.contains("hello"));
    }
}
