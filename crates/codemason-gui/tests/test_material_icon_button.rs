//! Tests for `codemason_gui::material::icon_button`: size, click reporting
//! and the toggle state exposed to accessibility.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::icons::Icon;
use codemason_gui::material::icon_button::{
    DENSE_SIZE, IconButtonKind, SIZE, icon_button, icon_button_dense,
};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_material_icon_button_is_a_40_point_square() {
    let palette = support_material::palette();
    for kind in [IconButtonKind::Standard, IconButtonKind::Filled] {
        let harness = support_material::builder().build_ui(move |ui| {
            icon_button(ui, &palette, kind, Icon::Copy, "Copy", None);
        });
        let rect = harness.get_by_label("Copy").rect();
        assert!((rect.width() - SIZE).abs() < 0.5, "{kind:?}");
        assert!((rect.height() - SIZE).abs() < 0.5, "{kind:?}");
    }
}

#[test]
fn test_material_icon_button_toggle_flips_on_click_and_reports_state() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, on: &mut bool| {
            let response = icon_button(
                ui,
                &palette,
                IconButtonKind::Filled,
                Icon::Info,
                "Details",
                Some(*on),
            );
            if response.clicked() {
                *on = !*on;
            }
        },
        false,
    );
    assert_eq!(
        harness.get_by_label("Details").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    harness.get_by_label("Details").click();
    harness.run();
    assert!(*harness.state());
    assert_eq!(
        harness.get_by_label("Details").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}

#[test]
fn test_material_dense_icon_button_is_one_text_row() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        icon_button_dense(
            ui,
            &palette,
            IconButtonKind::Standard,
            Icon::UnfoldLess,
            "Collapse all",
            None,
        );
    });
    let rect = harness.get_by_label("Collapse all").rect();
    assert!((rect.width() - DENSE_SIZE).abs() < 0.5, "{}", rect.width());
    assert!(
        (rect.height() - DENSE_SIZE).abs() < 0.5,
        "{}",
        rect.height()
    );
    // Dense is smaller than the standard target, so a text row does not grow.
    let dense_is_smaller = DENSE_SIZE < SIZE;
    assert!(dense_is_smaller, "dense {DENSE_SIZE} vs standard {SIZE}");
}
