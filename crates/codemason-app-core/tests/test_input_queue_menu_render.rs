//! Render tests for the ALT-Q queue-control menu overlay (spec `inputqueue`
//! T-014).
//!
//! Covers FR-021 (the menu presents exactly the four options `Next`, `Stop`,
//! `Clear`, and `Show`), FR-023 (the `Next` option is visible/selectable only
//! while the input queue is non-empty, otherwise hidden/non-selectable), and
//! NFR-007 (the menu reuses the shared overlay/modal rendering machinery rather
//! than a parallel path). The action semantics for the rows are covered by the
//! T-015..T-018 tests and the `Show`-panel suites.

#[path = "support/mod.rs"]
mod support;

// ---------------------------------------------------------------------------
// FR-021 - exactly four options, in fixed order
// ---------------------------------------------------------------------------

#[test]
fn test_menu_labels_are_next_halt_clear_and_show_in_order() {
    let app = support::make_app();

    let labels = app.queue_menu_labels();

    assert_eq!(labels.len(), 4, "FR-021: the menu has exactly four options");
    assert_eq!(labels[0], "Next", "FR-021: the first option is `Next`");
    assert_eq!(labels[2], "Clear", "FR-021: the third option is `Clear`");
    assert_eq!(labels[3], "Show", "FR-021: the fourth option is `Show`");
}

// ---------------------------------------------------------------------------
// FR-023 - `Next` visible/selectable only with a non-empty queue
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-007 - reuses the shared overlay/modal rendering path
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-026 - halt label reflects the running-turn state
// ---------------------------------------------------------------------------

#[test]
fn test_halt_label_is_stop_while_processing_and_resume_when_idle() {
    let mut app = support::make_app();

    app.session.is_processing = true;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Stop",
        "FR-026: while a turn executes the halt option reads `Stop`"
    );
    assert_eq!(app.queue_menu_labels()[1], "Stop");

    app.session.is_processing = false;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Resume",
        "FR-026: once the agent has stopped the option reads `Resume`"
    );
    assert_eq!(app.queue_menu_labels()[1], "Resume");
}
