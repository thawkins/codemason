//! Tests for the dockable pane layout model (`codemason_gui::dock`).
//!
//! Every file the tests write lives under `target/temp/` resolved from
//! `CARGO_MANIFEST_DIR`, so a run leaves the working tree untouched.

use std::path::PathBuf;

use craft_ui::docking::PanelState;

use codemason_gui::dock::{
    HiddenPanes, PaneId, default_layout, fit_prompt, load_layout, pin_left_tabs, save_layout,
    sync_visibility, toggle_pane,
};
use craft_ui::docking::Node;
use craft_ui::layout::SplitSize;

/// The size of the split whose second child is the prompt pane.
fn prompt_split_size(node: &Node<PaneId>) -> Option<SplitSize> {
    match node {
        Node::Split {
            size,
            first,
            second,
            ..
        } => match &**second {
            Node::Tabs { panels, .. } if panels == &[PaneId::Prompt] => Some(*size),
            _ => prompt_split_size(first).or_else(|| prompt_split_size(second)),
        },
        _ => None,
    }
}

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
fn test_dock_fit_prompt_sets_the_prompt_split_from_its_bottom() {
    let mut layout = default_layout();

    assert!(
        fit_prompt(&mut layout, 37.0),
        "a new height changes the split"
    );
    let root = layout.root.as_ref().expect("default layout has a root");
    assert_eq!(prompt_split_size(root), Some(SplitSize::FixedSecond(37.0)));
    assert!(
        !fit_prompt(&mut layout, 37.2),
        "a sub-point change leaves the split alone"
    );
    assert!(!fit_prompt(&mut layout, 0.0), "a zero height is ignored");
    assert!(
        layout.validate().is_ok(),
        "the fitted layout still validates"
    );
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
    assert_eq!(
        layout.panel_state(&PaneId::Log),
        PanelState::Front,
        "a pane shown again is its group's active tab"
    );
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

/// The panels of the tab group that holds `pane`, in tab order.
fn group_of(node: &Node<PaneId>, pane: PaneId) -> Option<Vec<PaneId>> {
    match node {
        Node::Tabs { panels, .. } if panels.contains(&pane) => Some(panels.clone()),
        Node::Split { first, second, .. } => {
            group_of(first, pane).or_else(|| group_of(second, pane))
        }
        _ => None,
    }
}

/// The default layout with the Context tab moved to the right column, as
/// layouts were saved before it was pinned.
fn old_layout() -> craft_ui::docking::Layout<PaneId> {
    let mut layout = default_layout();
    layout
        .apply(craft_ui::docking::Action::Move {
            panel: PaneId::Context,
            anchor: PaneId::ActiveAgents,
            placement: craft_ui::docking::Placement::Tab { before: None },
        })
        .expect("move Context to the right column");
    layout
        .apply(craft_ui::docking::Action::Activate {
            panel: PaneId::ActiveAgents,
        })
        .expect("front the right column's first tab");
    layout
}

#[test]
fn test_dock_context_tab_is_pinned_beside_sessions_by_default() {
    let layout = default_layout();
    let root = layout.root.as_ref().expect("root");
    assert_eq!(
        group_of(root, PaneId::Sessions),
        Some(vec![
            PaneId::Sessions,
            PaneId::Context,
            PaneId::Log,
            PaneId::Profile,
            PaneId::Tasks,
            PaneId::Memory,
        ])
    );
    for pane in PaneId::LEFT_TABS {
        let permissions = pane.permissions();
        assert!(
            !permissions.closable && !permissions.movable && !permissions.floatable,
            "{pane:?} is a fixed tab"
        );
        assert!(permissions.accepts_tabs, "{pane:?} takes tabs");
    }
}

#[test]
fn test_dock_fixed_tabs_are_left_tabs_only() {
    for pane in PaneId::ALL {
        assert_eq!(pane.is_left_tab(), PaneId::LEFT_TABS.contains(&pane));
    }
    assert!(!PaneId::Sessions.is_left_tab(), "the sidebar is not one");
}

#[test]
fn test_dock_log_and_tasks_leave_the_right_column() {
    let layout = default_layout();
    let root = layout.root.as_ref().expect("root");
    assert_eq!(
        group_of(root, PaneId::ActiveAgents),
        Some(vec![
            PaneId::ActiveAgents,
            PaneId::Teams,
            PaneId::Queue,
            PaneId::Diff,
        ])
    );
    for pane in [PaneId::Log, PaneId::Tasks] {
        assert_eq!(
            group_of(root, PaneId::Sessions).filter(|group| group.contains(&pane)),
            group_of(root, PaneId::Sessions),
            "{pane:?} is in the sidebar's group"
        );
        assert!(
            !group_of(root, PaneId::ActiveAgents)
                .expect("right group")
                .contains(&pane),
            "{pane:?} left the right column"
        );
    }
}

#[test]
fn test_dock_pin_left_tabs_moves_an_old_layouts_tabs_beside_sessions() {
    let mut layout = old_layout();
    let mut hidden = HiddenPanes::new();
    pin_left_tabs(&mut layout, &mut hidden).expect("pin");
    assert_eq!(layout, default_layout());
    assert_eq!(
        layout.panel_state(&PaneId::Sessions),
        PanelState::Front,
        "pinning does not cover the sidebar"
    );
    // Pinning pinned tabs changes nothing.
    assert!(!pin_left_tabs(&mut layout, &mut hidden).expect("pin again"));
    assert_eq!(layout, default_layout());
}

#[test]
fn test_dock_pin_left_tabs_opens_a_hidden_tab_beside_sessions() {
    let mut layout = old_layout();
    let mut hidden = HiddenPanes::new();
    toggle_pane(&mut layout, &mut hidden, PaneId::Context).expect("hide Context");
    assert!(hidden.contains_key(&PaneId::Context));
    pin_left_tabs(&mut layout, &mut hidden).expect("pin");
    assert!(
        !hidden.contains_key(&PaneId::Context),
        "old place forgotten"
    );
    assert_eq!(layout, default_layout());
}

#[test]
fn test_dock_pin_left_tabs_moves_a_stranded_log_and_tasks() {
    let mut layout = default_layout();
    let mut hidden = HiddenPanes::new();
    // A layout saved when the log and tasks panes lived in the right column.
    for pane in [PaneId::Log, PaneId::Tasks] {
        layout
            .apply(craft_ui::docking::Action::Move {
                panel: pane,
                anchor: PaneId::ActiveAgents,
                placement: craft_ui::docking::Placement::Tab { before: None },
            })
            .expect("move to the right column");
    }
    pin_left_tabs(&mut layout, &mut hidden).expect("pin");
    let root = layout.root.as_ref().expect("root");
    assert_eq!(
        group_of(root, PaneId::Sessions),
        Some(vec![
            PaneId::Sessions,
            PaneId::Context,
            PaneId::Log,
            PaneId::Profile,
            PaneId::Tasks,
            PaneId::Memory,
        ]),
        "both are pulled back left, in tab order"
    );
    assert_eq!(
        group_of(root, PaneId::ActiveAgents),
        Some(vec![
            PaneId::ActiveAgents,
            PaneId::Teams,
            PaneId::Queue,
            PaneId::Diff,
        ]),
        "the right column keeps only its own panes"
    );
    assert_eq!(
        layout.panel_state(&PaneId::Sessions),
        PanelState::Front,
        "the migration does not drop the user onto another pane"
    );
}
