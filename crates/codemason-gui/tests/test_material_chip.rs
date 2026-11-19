//! Tests for `codemason_gui::material::chip`: size, and a filter chip's
//! state and clicks.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::chip::{ChipKind, HEIGHT, chip};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_chip_filter_toggles_and_reports_state() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, selected: &mut bool| {
            let kind = ChipKind::Filter {
                selected: *selected,
            };
            if chip(ui, &palette, kind, None, "Errors only").clicked() {
                *selected = !*selected;
            }
            chip(ui, &palette, ChipKind::Assist, None, "Search");
        },
        false,
    );
    for label in ["Errors only", "Search"] {
        let rect = harness.get_by_label(label).rect();
        assert!((rect.height() - HEIGHT).abs() < 0.5, "{label}");
    }
    harness.get_by_label("Errors only").click();
    harness.run();
    assert!(*harness.state());
    assert_eq!(
        harness
            .get_by_label("Errors only")
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}
