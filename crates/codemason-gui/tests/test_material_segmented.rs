//! Tests for `codemason_gui::material::segmented`: a click selects its
//! segment and marks the row changed, and every segment is 40 points high.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::segmented::{HEIGHT, segmented_buttons};
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
