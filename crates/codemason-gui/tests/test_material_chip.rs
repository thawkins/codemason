//! Tests for `codemason_gui::material::chip`: size, and a filter chip's
//! state and clicks.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::chip::{ChipKind, DENSE_HEIGHT, HEIGHT, chip, chip_dense};
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

#[test]
fn test_material_chip_dense_status_is_short_and_reports_state() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, selected: &mut bool| {
            let kind = ChipKind::Status {
                on: *selected,
                caution: true,
            };
            if chip_dense(ui, &palette, kind, None, "YOLO").clicked() {
                *selected = !*selected;
            }
        },
        true,
    );
    let rect = harness.get_by_label("YOLO").rect();
    assert!((rect.height() - DENSE_HEIGHT).abs() < 0.5);
    assert_eq!(
        harness.get_by_label("YOLO").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    harness.get_by_label("YOLO").click();
    harness.run();
    assert!(!*harness.state());
}

#[test]
fn test_material_chip_status_keeps_its_width_on_and_off() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, on: &mut bool| {
            let kind = ChipKind::Status {
                on: *on,
                caution: false,
            };
            if chip_dense(ui, &palette, kind, None, "GCF").clicked() {
                *on = !*on;
            }
        },
        false,
    );
    // The cross (off) and the check (on) take the same room.
    let off = harness.get_by_label("GCF").rect().width();
    harness.get_by_label("GCF").click();
    harness.run();
    assert!(*harness.state());
    let on = harness.get_by_label("GCF").rect().width();
    assert!((on - off).abs() < 0.5, "off {off} on {on}");
}
