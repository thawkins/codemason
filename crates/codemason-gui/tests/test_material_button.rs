//! Tests for `codemason_gui::material::button`: size, click reporting,
//! the accessible name, and that a disabled button reports no clicks.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::button::{ButtonKind, HEIGHT, button};
use egui_kittest::kittest::Queryable;

#[test]
fn test_material_button_every_kind_is_40_points_high_and_clickable() {
    for kind in [
        ButtonKind::Filled,
        ButtonKind::Tonal,
        ButtonKind::Outlined,
        ButtonKind::Text,
    ] {
        let palette = support_material::palette();
        let mut harness = support_material::builder().build_ui_state(
            move |ui, clicks: &mut u32| {
                if button(ui, &palette, kind, None, "Send").clicked() {
                    *clicks += 1;
                }
            },
            0,
        );
        let node = harness.get_by_label("Send");
        assert!((node.rect().height() - HEIGHT).abs() < 0.5, "{kind:?}");
        node.click();
        harness.run();
        assert_eq!(*harness.state(), 1, "{kind:?}");
    }
}

#[test]
fn test_material_button_icon_widens_the_button() {
    let palette = support_material::palette();
    let mut widths = Vec::new();
    for icon in [None, Some(codemason_gui::icons::Icon::Check)] {
        let harness = support_material::builder().build_ui(move |ui| {
            button(ui, &palette, ButtonKind::Filled, icon, "Send");
        });
        widths.push(harness.get_by_label("Send").rect().width());
    }
    assert!(widths[1] > widths[0], "{widths:?}");
}

#[test]
fn test_material_button_disabled_reports_no_click() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, clicks: &mut u32| {
            ui.add_enabled_ui(false, |ui| {
                if button(ui, &palette, ButtonKind::Filled, None, "Send").clicked() {
                    *clicks += 1;
                }
            });
        },
        0,
    );
    harness.get_by_label("Send").click();
    harness.run();
    assert_eq!(*harness.state(), 0);
}
