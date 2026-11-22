//! Tests for `codemason_gui::material::segmented`: a click selects its
//! segment and marks the row changed, and every segment is 40 points high.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::segmented::{
    HEIGHT, SegmentMark, segmented_buttons, segmented_buttons_with,
};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_segmented_click_selects_and_marks_changed() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, state: &mut (usize, u32)| {
            if segmented_buttons(ui, &palette, &["Unified", "Split"], &mut state.0).changed() {
                state.1 += 1;
            }
        },
        (0, 0),
    );
    for label in ["Unified", "Split"] {
        let rect = harness.get_by_label(label).rect();
        assert!((rect.height() - HEIGHT).abs() < 0.5, "{label}");
    }
    assert_eq!(
        harness.get_by_label("Unified").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    harness.get_by_label("Split").click();
    harness.run();
    assert_eq!(*harness.state(), (1, 1));
    // Clicking the selected segment changes nothing.
    harness.get_by_label("Split").click();
    harness.run();
    assert_eq!(*harness.state(), (1, 1));
}

/// The width of segment `label` when the row is drawn with `mark`.
fn segment_width(mark: SegmentMark, label: &str) -> f32 {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui_state(
        move |ui, selected: &mut usize| {
            segmented_buttons_with(ui, &palette, &["Codex", "Claude"], selected, mark);
        },
        0,
    );
    harness.get_by_label(label).rect().width()
}

#[test]
fn test_material_segmented_highlight_mark_leaves_out_the_check() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, selected: &mut usize| {
            segmented_buttons_with(
                ui,
                &palette,
                &["Codex", "Claude"],
                selected,
                SegmentMark::Highlight,
            );
        },
        0,
    );
    harness.get_by_label("Claude").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
    assert_eq!(
        harness.get_by_label("Claude").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    // No room is kept for a check icon: 18 pt icon plus 8 pt gap.
    let check = segment_width(SegmentMark::Check, "Codex");
    let highlight = segment_width(SegmentMark::Highlight, "Codex");
    assert!(
        (check - highlight - 26.0).abs() < 0.5,
        "{check} vs {highlight}"
    );
}
