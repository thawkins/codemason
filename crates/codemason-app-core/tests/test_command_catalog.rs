//! Drift guard for the static slash-command catalog used by `commands_info`.
//!
//! `codemason-agent` cannot depend on `codemason-tui`, so the `commands_info` tool
//! serves a static mirror of the TUI's `SLASH_COMMANDS` table. This test
//! compares the two so a command added to (or removed from) the TUI registry
//! fails loudly until the mirror in
//! `crates/codemason-agent/src/tool/command_catalog.rs` is updated.

use std::collections::{BTreeMap, BTreeSet};

#[test]
fn test_command_catalog_covers_every_slash_command() {
    let tui_commands: BTreeSet<&str> = codemason_app_core::app::SLASH_COMMANDS
        .iter()
        .map(|c| c.trigger)
        .collect();
    let catalog: BTreeSet<&str> = codemason_agent::tool::command_catalog::COMMAND_CATALOG
        .iter()
        .map(|c| c.trigger)
        .collect();

    let missing: Vec<&&str> = tui_commands.difference(&catalog).collect();
    let extra: Vec<&&str> = catalog.difference(&tui_commands).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "command_catalog drift: missing from catalog {missing:?}; extra in catalog {extra:?}"
    );
}

#[test]
fn test_command_catalog_descriptions_match_slash_commands() {
    let tui_descriptions: BTreeMap<&str, &str> = codemason_app_core::app::SLASH_COMMANDS
        .iter()
        .map(|c| (c.trigger, c.description))
        .collect();
    for entry in codemason_agent::tool::command_catalog::COMMAND_CATALOG {
        let Some(expected) = tui_descriptions.get(entry.trigger) else {
            // Covered by the drift test above.
            continue;
        };
        assert_eq!(
            entry.description, *expected,
            "description drift for /{}: catalog={:?} tui={:?}",
            entry.trigger, entry.description, *expected
        );
    }
}
