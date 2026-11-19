//! Tests for `codemason_gui::material::sheet`: drawer items' size, state and
//! clicks, and the side sheet header's close button.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::icons::Icon;
use codemason_gui::material::sheet::{ITEM_HEIGHT, drawer_item, sheet_frame, sheet_header};
use codemason_gui::palette::rgb;
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_sheet_drawer_items_select_on_click() {
    let palette = support_material::palette();
    let mut harness = support_material::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            move |ui, current: &mut usize| {
                for (index, label) in ["Current", "Other"].into_iter().enumerate() {
                    let selected = *current == index;
                    if drawer_item(ui, &palette, Some(Icon::Read), label, selected).clicked() {
                        *current = index;
                    }
                }
            },
            0,
        );
    let rect = harness.get_by_label("Current").rect();
    assert!((rect.height() - ITEM_HEIGHT).abs() < 0.5);
    assert_eq!(
        harness.get_by_label("Current").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    harness.get_by_label("Other").click();
    harness.run();
    assert_eq!(*harness.state(), 1);
}

#[test]
fn test_material_sheet_header_reports_close() {
    let palette = support_material::palette();
    assert_eq!(
        sheet_frame(&palette).fill,
        rgb(palette.m3.surface_container_low)
    );
    let mut harness = support_material::builder()
        .with_size(egui::vec2(300.0, 120.0))
        .build_ui_state(
            move |ui, closed: &mut bool| {
                if sheet_header(ui, &palette, "Tasks") {
                    *closed = true;
                }
            },
            false,
        );
    harness.get_by_label("Close").click();
    harness.run();
    assert!(*harness.state());
}
