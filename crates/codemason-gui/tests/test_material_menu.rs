//! Tests for `codemason_gui::material::menu`: item size, clicks, and the
//! menu surface's colours.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::icons::Icon;
use codemason_gui::material::menu::{ITEM_HEIGHT, menu_frame, menu_item};
use codemason_gui::palette::rgb;
use egui_kittest::kittest::Queryable;

#[test]
fn test_material_menu_item_is_48_points_high_and_clickable() {
    let palette = support_material::palette();
    let mut harness = support_material::builder().build_ui_state(
        move |ui, chosen: &mut Option<&'static str>| {
            menu_frame(&palette).show(ui, |ui| {
                if menu_item(ui, &palette, Some(Icon::Copy), "Copy", Some("Ctrl+C")).clicked() {
                    *chosen = Some("Copy");
                }
                if menu_item(ui, &palette, None, "Find", None).clicked() {
                    *chosen = Some("Find");
                }
            });
        },
        None,
    );
    let rect = harness.get_by_label("Copy").rect();
    assert!((rect.height() - ITEM_HEIGHT).abs() < 0.5);
    assert!(rect.width() >= 112.0);
    harness.get_by_label("Find").click();
    harness.run();
    assert_eq!(*harness.state(), Some("Find"));
}

#[test]
fn test_material_menu_frame_is_a_surface_container_with_a_shadow() {
    let palette = support_material::palette();
    let frame = menu_frame(&palette);
    assert_eq!(frame.fill, rgb(palette.m3.surface_container));
    assert_ne!(frame.shadow, egui::Shadow::NONE);
}
