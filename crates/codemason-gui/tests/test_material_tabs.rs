//! Tests for `codemason_gui::material::tabs`: tabs share the width, and a
//! click activates its tab.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::tabs::{HEIGHT, primary_tabs};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_tabs_share_the_width_and_switch_on_click() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(400.0, 100.0))
        .build_ui_state(
            move |ui, selected: &mut usize| {
                primary_tabs(ui, &palette, &["Home", "Insert"], selected);
            },
            0,
        );
    let home = harness.get_by_label("Home").rect();
    let insert = harness.get_by_label("Insert").rect();
    assert!((home.width() - insert.width()).abs() < 0.5);
    assert!((home.height() - HEIGHT).abs() < 0.5);
    harness.get_by_label("Insert").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
    assert_eq!(
        harness.get_by_label("Insert").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}
