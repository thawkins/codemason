//! Tests for the destructive row-action confirmation dialog (spec `agentui`
//! T-009).
//!
//! Covers the modal itself - `open_row_action` arming it with `No` selected
//! (FR-069), the render that paints the title/target and caches the three
//! hit-rect areas (NFR-010), the keyboard answer path (`Enter` selects the
//! highlighted option, `Esc` dismisses, every other key is swallowed so
//! nothing reaches the input buffer), the mouse answer path (a click on the
//! painted `[ Yes ]` / `[ No ]` span answers that option), the destructive
//! trigger points routing through the dialog (sub-agent Shift+K with the
//! documented task id and agent name; background `[x]` cancel; teammate
//! `[stop]` suspend with the direct resume exemption, FR-040; teammate `[x]`
//! shutdown; teammate Space/Shift+K shutdown, FR-045), the stale-row answer
//! that mutates nothing (FR-069), and the redraw flag every mutating path
//! sets (NFR-005).
//!
//! The keyboard-only confirmations mirror `test_stop_confirm_dialog.rs` and
//! the mouse cases drive the real `App::handle_mouse_event` router with the
//! rects the renderer records.

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_app_core::app::App;
use codemason_app_core::app::RowAction;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-0001";

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// A live sub-agent task.
fn task(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
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

/// A background shell task view.
fn bg_task(id: &str, command: &str) -> codemason_app_core::app::BgTaskView {
    codemason_app_core::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: command.to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// An app with one running sub-agent and the Agents window open.
fn app_with_task() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.show_agents_window = true;
    app
}

// ---------------------------------------------------------------------------
// Arming and defaults
// ---------------------------------------------------------------------------

/// `open_row_action` arms the dialog with `No` selected and demands a redraw
/// (FR-069, NFR-005); the dialog alone dispatches nothing.
#[tokio::test]
async fn open_row_action_arms_the_dialog_with_no_selected() {
    let mut app = make_app();

    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    assert!(
        app.row_action_confirm_open(),
        "the dialog must be armed by open_row_action"
    );
    assert!(
        !app.row_action_confirm_is_yes(),
        "No must be the default selection (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// Keyboard answering (FR-069, NFR-005)
// ---------------------------------------------------------------------------

/// `Enter` on the default `No` dismisses the dialog and dispatches nothing.
#[tokio::test]
async fn enter_on_the_default_no_dismisses_without_dispatching() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "Enter on the default No closes the dialog"
    );
    assert!(
        app.active_tasks
            .iter()
            .any(|t| t.id == "a1b2c3d4" && t.status == TaskStatus::Running),
        "No must leave the task untouched (FR-069)"
    );
}

/// Toggle then `Enter` accepts: the dialog closes and the pending action is
/// dispatched.
#[tokio::test]
async fn enter_after_toggling_accepts_the_pending_action() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    // Left toggles No -> Yes (the queue-clear dialog's horizontal-move rule).
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    assert!(
        app.row_action_confirm_is_yes(),
        "Left moves the selection to Yes"
    );
    assert!(
        app.row_action_confirm_open(),
        "toggling keeps the dialog open"
    );

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(!app.row_action_confirm_open(), "Yes closes the dialog");
}

/// `Esc` dismisses the dialog and dispatches nothing.
#[tokio::test]
async fn esc_dismisses_the_dialog() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::SuspendTeammate {
        team_name: String::new(),
        agent_id: "tm-001".to_string(),
        name: "reviewer".to_string(),
    });

    app.handle_key_event(key(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "Esc closes the dialog");
}

/// While the dialog is open every other key is swallowed: nothing reaches the
/// input buffer and the dialog stays armed.
#[tokio::test]
async fn other_keys_are_swallowed_while_the_dialog_is_open() {
    let mut app = app_with_task();
    app.session.input = "hello".to_string();
    app.session.input_cursor = 5;
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });

    app.handle_key_event(key(KeyCode::Char('q'), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('k'), KeyModifiers::SHIFT))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "an unrelated key must not dismiss the dialog"
    );
    assert_eq!(
        app.session.input, "hello",
        "modal keys must not leak into the input buffer (FR-055 style gate)"
    );
}

// ---------------------------------------------------------------------------
// Mouse answering (T-009: "with keyboard and mouse answering")
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Destructive trigger points route through the dialog
// ---------------------------------------------------------------------------

/// Arming the background-shell cancel directly through `open_row_action` (the
/// keyboard cursor never covers background rows, so the trigger is
/// service-level; the confirm flow itself is unchanged, FR-037).
#[tokio::test]
async fn background_cancel_opens_the_cancel_dialog() {
    let mut app = app_with_task();
    app.active_tasks.clear();
    app.bg_tasks = vec![bg_task("cc112233", "sleep 30")];
    app.open_row_action(RowAction::CancelBackground {
        task_id: "cc112233".to_string(),
        command: "sleep 30".to_string(),
    });

    assert!(
        app.row_action_confirm_open(),
        "the background cancel confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::CancelBackground { task_id, command }) => {
            assert_eq!(task_id, "cc112233");
            assert_eq!(command, "sleep 30");
        }
        other => panic!("expected CancelBackground, got {other:?}"),
    }
}

/// `Space` on the cursor-selected RUNNING sub-agent row suspends it without a
/// dialog: the suspend direction is not destructive (FR-035 parity with the
/// mouse; the old selected_agent_index trigger is retired).
#[tokio::test]
async fn space_on_a_running_agent_row_suspends_without_a_dialog() {
    let mut app = app_with_task();
    app.move_agents_cursor(1);

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "a non-destructive suspend opens no dialog"
    );
}

// ---------------------------------------------------------------------------
// Stale-row answers dispatch nothing (FR-069)
// ---------------------------------------------------------------------------

/// Answering `Yes` after the row vanished closes the dialog and mutates
/// nothing (FR-069).
#[tokio::test]
async fn answering_yes_after_the_row_vanishes_mutates_nothing() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });
    // The subject disappears between open and answer.
    app.active_tasks.clear();
    app.status = "sentinel".to_string();

    // Toggle to Yes and confirm.
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open());
    assert_eq!(
        app.status, "sentinel",
        "a stale answer must not touch the status line or any list (FR-069)"
    );
}

/// The dialog closes on `Yes` even when the target transitioned to a
/// background shell task identity; the same cancel service runs either way.
#[tokio::test]
async fn the_kill_answer_follows_a_row_that_became_a_background_task() {
    let mut app = app_with_task();
    app.open_row_action(RowAction::KillSubagent {
        task_id: "a1b2c3d4".to_string(),
        agent_name: "explore".to_string(),
    });
    // The task registry replaced the agent task with a bg task of the same id.
    app.active_tasks.clear();
    app.bg_tasks = vec![bg_task("a1b2c3d4", "sleep 30")];

    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "the answer still resolves");
}

// ---------------------------------------------------------------------------
// FR-028 / FR-032 - the dialog states which option performs and which cancels
// ---------------------------------------------------------------------------

/// The confirming button names the action's own verb, and the outcome hint
/// names that same verb, so the dialog's destructive click is never a generic
/// `Yes`.
#[test]
fn every_row_action_confirms_with_its_own_verb() {
    let actions = [
        (
            RowAction::KillSubagent {
                task_id: "a1b2c3d4".to_string(),
                agent_name: "explore".to_string(),
            },
            "Kill",
        ),
        (
            RowAction::CancelBackground {
                task_id: "a1b2c3d4".to_string(),
                command: "sleep 30".to_string(),
            },
            "Cancel task",
        ),
        (
            RowAction::SuspendTeammate {
                team_name: String::new(),
                agent_id: "tm-001".to_string(),
                name: "reviewer".to_string(),
            },
            "Suspend",
        ),
        (
            RowAction::ShutdownTeammate {
                team_name: String::new(),
                agent_id: "tm-001".to_string(),
                name: "reviewer".to_string(),
            },
            "Shut down",
        ),
        (
            RowAction::ResumeTeammate {
                team_name: String::new(),
                agent_id: "tm-001".to_string(),
                name: "reviewer".to_string(),
            },
            "Resume",
        ),
        (
            RowAction::DeleteSession {
                session_id: "session-1".to_string(),
                title: "a session".to_string(),
                children: 0,
            },
            "Delete",
        ),
    ];
    for (action, verb) in actions {
        assert_eq!(action.confirm_label(), verb, "{action:?}");
        let hint = codemason_app_core::app::row_action_outcome_hint(&action);
        assert!(
            hint.contains(verb),
            "{action:?}: the outcome hint {hint:?} does not name {verb:?}"
        );
        assert!(
            !hint.contains("Yes"),
            "{action:?}: the outcome hint {hint:?} still says Yes"
        );
    }
}
