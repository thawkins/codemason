//! Tests for `codemason_gui::material::dialog`: the dialog surface, and the
//! action row's height and right-to-left order.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::button::{ButtonKind, HEIGHT, button};
use codemason_gui::material::dialog::{dialog_actions, dialog_body, dialog_frame, dialog_headline};
use codemason_gui::metrics::radius;
use codemason_gui::palette::rgb;
use egui_kittest::kittest::Queryable;

#[test]
fn test_material_dialog_frame_is_extra_large_and_raised() {
    let palette = support_material::palette();
    let frame = dialog_frame(&palette);
    assert_eq!(frame.fill, rgb(palette.m3.surface_container_high));
    assert_eq!(frame.corner_radius, egui::CornerRadius::same(radius::XL));
    assert_ne!(frame.shadow, egui::Shadow::NONE);
}

#[test]
fn test_material_dialog_actions_are_one_row_with_the_first_rightmost() {
    let palette = support_material::palette();
    let harness = support_material::builder()
        .with_size(egui::vec2(480.0, 400.0))
        .build_ui(move |ui| {
            dialog_frame(&palette).show(ui, |ui| {
                dialog_headline(ui, &palette, "Discard changes?");
                dialog_body(ui, &palette, "The edits will be lost.");
                let row = dialog_actions(ui, |ui| {
                    button(ui, &palette, ButtonKind::Text, None, "Discard");
                    button(ui, &palette, ButtonKind::Text, None, "Keep");
                });
                assert!((row.response.rect.height() - HEIGHT).abs() < 0.5);
            });
        });
    let discard = harness.get_by_label("Discard").rect();
    let keep = harness.get_by_label("Keep").rect();
    assert!(discard.left() > keep.right(), "{discard:?} {keep:?}");
}
