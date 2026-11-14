//! Tests for the dockable pane layout model (`codemason_gui::dock`).
//!
//! Every file the tests write lives under `target/temp/` resolved from
//! `CARGO_MANIFEST_DIR`, so a run leaves the working tree untouched.

use std::path::PathBuf;

use craft_ui::docking::PanelState;

use codemason_gui::dock::{
    HiddenPanes, PaneId, default_layout, load_layout, save_layout, sync_visibility, toggle_pane,
};

/// A fresh per-test scratch directory under the workspace `target/temp/`.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/codemason-gui-test-dock")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn test_dock_default_layout_contains_messages_and_prompt() {
    let layout = default_layout();
    assert!(layout.validate().is_ok(), "default layout must validate");
    assert!(layout.contains(&PaneId::Messages));
    assert!(layout.contains(&PaneId::Prompt));
}

#[test]
fn test_dock_toggle_log_twice_restores_layout() {
    let mut layout = default_layout();
    let mut hidden = HiddenPanes::new();
    let before = layout.location(&PaneId::Log).expect("log is docked");

    toggle_pane(&mut layout, &mut hidden, PaneId::Log).expect("hide log");
    assert!(!layout.contains(&PaneId::Log), "first toggle hides the log");

    toggle_pane(&mut layout, &mut hidden, PaneId::Log).expect("show log");
    let after = layout.location(&PaneId::Log).expect("log is docked again");
    assert_eq!(before, after);
    assert_eq!(layout, default_layout());
}

#[test]
fn test_dock_layout_roundtrips_json() {
    let path = scratch_dir("roundtrip").join("gui-layout.json");
    let layout = default_layout();
    save_layout(&path, &layout).expect("save layout");
    assert_eq!(load_layout(&path), layout);
}

#[test]
fn test_dock_load_layout_invalid_file_falls_back_to_default() {
    let dir = scratch_dir("invalid");
    let path = dir.join("gui-layout.json");
    std::fs::write(&path, "{ not json").expect("write invalid file");
    assert_eq!(load_layout(&path), default_layout());
    assert_eq!(load_layout(&dir.join("missing.json")), default_layout());
}

#[test]
fn test_dock_sync_visibility_hides_and_restores_in_place() {
    let mut layout = default_layout();
    let mut hidden = HiddenPanes::new();
    let before = layout.location(&PaneId::Teams).expect("teams is docked");

    let changed = sync_visibility(&mut layout, &mut hidden, |pane| pane != PaneId::Teams)
        .expect("hide teams");
    assert!(changed);
    assert!(!layout.contains(&PaneId::Teams));
    let unchanged = sync_visibility(&mut layout, &mut hidden, |pane| pane != PaneId::Teams)
        .expect("nothing to do");
    assert!(!unchanged, "a synced layout is left alone");

    sync_visibility(&mut layout, &mut hidden, |_| true).expect("show teams");
    assert_eq!(
        layout.location(&PaneId::Teams).expect("docked again"),
        before
    );
    assert_eq!(
        layout.panel_state(&PaneId::Teams),
        PanelState::Front,
        "a pane shown again is its group's active tab"
    );
}
