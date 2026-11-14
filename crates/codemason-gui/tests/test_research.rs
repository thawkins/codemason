//! Tests for `codemason_gui::panels::research`: the research and deck
//! progress views drawn above the transcript while a `/research` run is
//! going. Each test seeds app-core's `research_progress` trackers the way
//! the event handler does, draws the whole [`Shell`] frame, and finds the
//! views by their accessible labels. Nothing sleeps or writes to disk.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::App;
use codemason_app_core::deck_progress::{
    DECK_PROGRESS_SENTINEL, DeckTracker, decode_deck_progress_event,
};
use codemason_app_core::research_progress::{ResearchProgress, SessionPhase, StepStatus};
use codemason_gui::Shell;
use codemason_gui::panels::research::{DECK_PROGRESS_LABEL, RESEARCH_PROGRESS_LABEL};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// A harness over a shell whose App `configure` prepared, after one run.
fn harness(configure: impl FnOnce(&mut App)) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    configure(&mut app);
    // A new session's transcript starts with the block-character logo; the
    // harness does not install the window's fonts.
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), Shell::new(app));
    harness.run();
    (runtime, harness)
}

/// Apply one deck stage report to `tracker`, encoded as the research crate
/// publishes it.
fn deck_report(tracker: &mut DeckTracker, stage: &str, status: &str, detail: &str) {
    let message = format!(
        "{DECK_PROGRESS_SENTINEL}{}",
        serde_json::json!({
            "name": "q3-review",
            "stage": stage,
            "status": status,
            "detail": detail,
            "completed": 2,
            "total": 5,
        })
    );
    let decoded = decode_deck_progress_event(&message).expect("deck report decodes");
    tracker.apply(&decoded.event());
}

/// Whether a node with `role` and `label` is drawn.
fn shown(harness: &Harness<'_, Shell>, role: Role, label: &str) -> bool {
    harness.query_by_role_and_label(role, label).is_some()
}

/// Whether some drawn node's label contains `text`.
fn has_text(harness: &Harness<'_, Shell>, text: &str) -> bool {
    harness.query_by_label_contains(text).is_some()
}

/// The text of the drawn label containing `needle` (a `Role::Label` keeps
/// its text in the AccessKit value).
fn row_text(harness: &Harness<'_, Shell>, needle: &str) -> String {
    let node = harness.get_by_label_contains(needle).accesskit_node();
    node.value().or_else(|| node.label()).unwrap_or_default()
}

#[test]
fn test_research_progress_shows_phase_label() {
    let (_rt, harness) = harness(|app| {
        let mut progress = ResearchProgress::new("rust-async", "async rust runtimes");
        progress.apply(SessionPhase::Setup, StepStatus::Done, "workspace ready");
        progress.apply(
            SessionPhase::Web,
            StepStatus::Started,
            "searching 12 queries",
        );
        app.research_progress.push(progress);
    });
    assert!(shown(&harness, Role::Group, RESEARCH_PROGRESS_LABEL));
    assert!(!shown(&harness, Role::Group, DECK_PROGRESS_LABEL));
    assert!(has_text(&harness, "Research progress - rust-async"));
    assert!(has_text(&harness, "Topic: async rust runtimes"));
    // Each step shows its phase label and status word.
    let web_text = row_text(&harness, "searching 12 queries");
    assert!(web_text.contains("web"), "{web_text}");
    assert!(web_text.contains("running"), "{web_text}");
    let setup_text = row_text(&harness, "workspace ready");
    assert!(setup_text.contains("setup"), "{setup_text}");
    assert!(setup_text.contains("done"), "{setup_text}");
}

#[test]
fn test_research_progress_shows_web_totals() {
    let (_rt, harness) = harness(|app| {
        let mut progress = ResearchProgress::new("rust-async", "async rust");
        progress.apply(SessionPhase::Web, StepStatus::Started, "searching");
        progress.apply(SessionPhase::Web, StepStatus::Done, "captured https://a");
        progress.apply(SessionPhase::Web, StepStatus::Done, "captured https://b");
        progress.apply(
            SessionPhase::Web,
            StepStatus::Error,
            "fetch failed for https://c",
        );
        app.research_progress.push(progress);
    });
    assert!(has_text(&harness, "Web fetch totals: 2 fetched, 1 failed"));
}

#[test]
fn test_deck_progress_shows_stage_rows() {
    let (_rt, harness) = harness(|app| {
        let mut progress = ResearchProgress::new("q3-review", "q3-review");
        let mut deck = DeckTracker::new();
        deck_report(&mut deck, "select", "done", "3 items");
        deck_report(&mut deck, "read", "started", "reading sources");
        progress.deck = Some(deck);
        app.research_progress.push(progress);
    });
    assert!(shown(&harness, Role::Group, DECK_PROGRESS_LABEL));
    assert!(!shown(&harness, Role::Group, RESEARCH_PROGRESS_LABEL));
    assert!(has_text(&harness, "Deck progress - q3-review"));
    let read_text = row_text(&harness, "reading sources");
    assert!(read_text.contains("read"), "{read_text}");
    assert!(read_text.contains("started"), "{read_text}");
    assert!(read_text.contains("[2/5]"), "{read_text}");
    assert!(has_text(&harness, "3 items"));
}

#[test]
fn test_research_views_hidden_once_runs_finish() {
    let (_rt, harness) = harness(|app| {
        let mut done = ResearchProgress::new("finished", "finished topic");
        done.apply(SessionPhase::Web, StepStatus::Started, "searching");
        done.finish(4, 0, 0, 0);
        app.research_progress.push(done);
        let mut deck_run = ResearchProgress::new("deck-done", "deck-done");
        let mut deck = DeckTracker::new();
        deck_report(&mut deck, "export", "done", "exported");
        deck.finish();
        deck_run.deck = Some(deck);
        app.research_progress.push(deck_run);
    });
    assert!(!shown(&harness, Role::Group, RESEARCH_PROGRESS_LABEL));
    assert!(!shown(&harness, Role::Group, DECK_PROGRESS_LABEL));
}
