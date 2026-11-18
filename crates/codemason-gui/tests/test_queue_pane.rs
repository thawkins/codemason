//! The Queue dock pane lists the queued prompts, or says the queue is empty.
//!
//! GUI assertions use egui_kittest accessibility queries only.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::{App, QueuedInput};
use codemason_gui::panels::queue::{QUEUE_PANE_EMPTY, QUEUE_PANE_LABEL, queue_pane_ui};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn pane(app: App) -> Harness<'static, App> {
    let mut harness = Harness::new_ui_state(|ui, app: &mut App| queue_pane_ui(ui, app), app);
    harness.run();
    harness
}

#[tokio::test]
async fn test_queue_pane_says_when_the_queue_is_empty() {
    let harness = pane(support::make_app());

    harness.get_by_label(QUEUE_PANE_EMPTY);
    assert!(harness.query_by_label(QUEUE_PANE_LABEL).is_none());
    assert!(
        harness.query_by_label("Queue panel").is_none(),
        "the placeholder text is gone"
    );
}

#[tokio::test]
async fn test_queue_pane_lists_queued_prompts_in_order() {
    let mut app = support::make_app();
    for text in ["first queued", "second queued"] {
        app.session.input_queue.push_back(QueuedInput {
            text: text.to_string(),
            image_paths: Vec::new(),
        });
    }

    let harness = pane(app);

    harness.get_by_role_and_label(Role::List, QUEUE_PANE_LABEL);
    harness.get_by_label("2 queued");
    harness.get_by_label("1. first queued");
    harness.get_by_label("2. second queued");
    assert!(harness.query_by_label(QUEUE_PANE_EMPTY).is_none());
}
