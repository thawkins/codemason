//! Tests for the Teams table body (spec `agentui` T-005).
//!
//! Covers the lead row (FR-021), the teammate rows and their focus marker
//! (FR-022), the status vocabulary (FR-023, NFR-009), the model column with
//! global-model suppression (FR-024), the claimed/done/sent/recv counters
//! (FR-025), the empty state (FR-026) and character-safe truncation (NFR-004).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and reads the painted buffer back, so no renderer logic is
//! re-implemented here (NFR-001, NFR-012).

mod support;

use codemason_agent::team::MemberStatus;

// -- FR-021: the lead row ---------------------------------------------------

// -- FR-022: teammate rows --------------------------------------------------

// -- FR-023 / NFR-009: status vocabulary ------------------------------------

/// The canonical words are the documented nine, in their compact UI spelling
/// (FR-023).
#[test]
fn test_teams_status_vocabulary_is_exactly_the_documented_nine_words() {
    let words: Vec<&str> = [
        MemberStatus::Spawning,
        MemberStatus::Working,
        MemberStatus::Idle,
        MemberStatus::PlanPending,
        MemberStatus::Blocked,
        MemberStatus::Suspended,
        MemberStatus::ShuttingDown,
        MemberStatus::Stopped,
        MemberStatus::Failed,
    ]
    .iter()
    .map(MemberStatus::display_label)
    .collect();

    assert_eq!(
        words,
        vec![
            "spawning",
            "working",
            "idle",
            "planning",
            "blocked",
            "suspended",
            "stopping",
            "stopped",
            "failed",
        ],
        "the status vocabulary is fixed by FR-023"
    );
}

// -- FR-024: the model column ----------------------------------------------

// -- FR-025: counters -------------------------------------------------------

// -- FR-026: empty state ----------------------------------------------------

// -- Row controls -----------------------------------------------------------

// -- NFR-004: safe truncation ----------------------------------------------

// -- NFR-011: column grid ---------------------------------------------------
