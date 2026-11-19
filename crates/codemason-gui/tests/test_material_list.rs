//! Tests for `codemason_gui::material::list`: an item's height for one and
//! two lines, its accessible name and selected state, and its clicks.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::list::{ONE_LINE_HEIGHT, TWO_LINE_HEIGHT, list_item};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_list_item_heights_and_selected_state() {
    let palette = support_material::palette();
    let harness = support_material::builder()
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui(move |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let one = list_item(ui, &palette, "glm-5.2", None, Some("1M  Free"), false);
            let two = list_item(ui, &palette, "GPT-4o", Some("gpt-4o"), None, true);
            assert!((one.rect.height() - ONE_LINE_HEIGHT).abs() < 0.5);
            assert!((two.rect.height() - TWO_LINE_HEIGHT).abs() < 0.5);
        });
    let selected = harness.get_by_role_and_label(Role::ListItem, "GPT-4o");
    assert_eq!(selected.accesskit_node().is_selected(), Some(true));
    let plain = harness.get_by_role_and_label(Role::ListItem, "glm-5.2");
    assert_ne!(plain.accesskit_node().is_selected(), Some(true));
}

#[test]
fn test_material_list_item_reports_a_click() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(400.0, 100.0))
        .build_ui_state(
            move |ui, clicks: &mut u32| {
                if list_item(ui, &palette, "Kimi K2", None, None, false).clicked() {
                    *clicks += 1;
                }
            },
            0,
        );
    harness.get_by_label("Kimi K2").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
}
