//! Tests for `codemason_gui::material::sheet::drawer_item_with_count`.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::sheet::{ITEM_HEIGHT, drawer_item_with_count};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_drawer_count_label() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(300.0, 200.0))
        .build_ui_state(
            move |ui, picked: &mut bool| {
                if drawer_item_with_count(ui, &palette, "Testing", 19, *picked).clicked() {
                    *picked = true;
                }
            },
            false,
        );
    let rect = harness.get_by_label("Testing (19)").rect();
    assert!((rect.height() - ITEM_HEIGHT).abs() < 0.5);
    assert_eq!(
        harness
            .get_by_label("Testing (19)")
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    harness.get_by_label("Testing (19)").click();
    harness.run();
    assert_eq!(
        harness
            .get_by_label("Testing (19)")
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}
