//! Tests for `codemason_gui::material::list`: an item's height for one and
//! two lines, its accessible name and selected state, and its clicks.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::list::{
    DENSE_HEIGHT, ONE_LINE_HEIGHT, TWO_LINE_HEIGHT, dense_item_option, list_item,
};
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
    // The painted second line and trailing text reach screen readers.
    assert_eq!(
        selected.accesskit_node().description().as_deref(),
        Some("gpt-4o")
    );
    let plain_item = harness.get_by_role_and_label(Role::ListItem, "glm-5.2");
    assert_eq!(
        plain_item.accesskit_node().description().as_deref(),
        Some("1M  Free")
    );
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

#[test]
fn test_material_dense_item_option_is_the_list_boxes_selected_row() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(320.0, 240.0))
        .build_ui_state(
            move |ui, chosen: &mut Option<usize>| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let rows = ["src/main.rs", "src/panel.rs", "src/prompt.rs"];
                let list = ui.vertical(|ui| {
                    for (i, row) in rows.iter().enumerate() {
                        if dense_item_option(ui, &palette, 0.0, row, i == 1).clicked() {
                            *chosen = Some(i);
                        }
                    }
                });
                list.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::ListBox, true, "Mention completions")
                });
            },
            None,
        );
    // The rows are the list box's options, one row high each.
    harness.get_by_role_and_label(Role::ListBox, "Mention completions");
    for row in ["src/main.rs", "src/panel.rs", "src/prompt.rs"] {
        let option = harness.get_by_role_and_label(Role::ListBoxOption, row);
        assert!((option.rect().height() - DENSE_HEIGHT).abs() < 0.5);
    }
    // The highlighted row is the selected option; the others are not.
    let selected = harness.get_by_role_and_label(Role::ListBoxOption, "src/panel.rs");
    assert_eq!(selected.accesskit_node().is_selected(), Some(true));
    let plain = harness.get_by_role_and_label(Role::ListBoxOption, "src/main.rs");
    assert_eq!(plain.accesskit_node().is_selected(), Some(false));
    // A click reports which option was chosen.
    harness
        .get_by_role_and_label(Role::ListBoxOption, "src/prompt.rs")
        .click();
    harness.run();
    assert_eq!(*harness.state(), Some(2));
}
