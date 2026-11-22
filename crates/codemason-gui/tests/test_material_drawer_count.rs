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

/// The painted text shapes of the last frame, as `(text, rect)`.
fn painted_texts(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some((
                text.galley.job.text.clone(),
                text.galley.rect.translate(text.pos.to_vec2()),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn test_material_drawer_count_long_label_leaves_room_for_the_count() {
    let palette = support_material::palette();
    let label = "A very long marketplace name that runs past the rail";
    let mut harness = support_material::builder()
        .with_size(egui::vec2(180.0, 100.0))
        .build_ui(move |ui| {
            drawer_item_with_count(ui, &palette, label, 1234, false);
        });
    harness.run();
    let texts = painted_texts(harness.output());
    let count = texts
        .iter()
        .find(|(text, _)| text == "1234")
        .map(|(_, rect)| *rect)
        .expect("the count is painted");
    let painted = texts
        .iter()
        .find(|(text, _)| text.starts_with("A very"))
        .map(|(_, rect)| *rect)
        .expect("the label is painted");
    assert!(
        painted.right() <= count.left(),
        "label {painted:?} ends before count {count:?}"
    );
}
