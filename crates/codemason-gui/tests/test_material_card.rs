//! Tests for `codemason_gui::material::card`: each kind's fill, border and
//! shadow, and the shared corner radius.

#[path = "support_material.rs"]
mod support_material;

use codemason_gui::material::card::{CardKind, card_frame};
use codemason_gui::metrics::radius;
use codemason_gui::palette::rgb;

#[test]
fn test_material_card_kinds_have_their_fill_border_and_shadow() {
    let palette = support_material::palette();
    let m3 = palette.m3;
    let elevated = card_frame(CardKind::Elevated, &palette);
    assert_eq!(elevated.fill, rgb(m3.surface_container_low));
    assert_ne!(elevated.shadow, egui::Shadow::NONE);
    let filled = card_frame(CardKind::Filled, &palette);
    assert_eq!(filled.fill, rgb(m3.surface_container_highest));
    assert_eq!(filled.shadow, egui::Shadow::NONE);
    let outlined = card_frame(CardKind::Outlined, &palette);
    assert_eq!(outlined.fill, rgb(m3.surface));
    assert_eq!(outlined.stroke.color, rgb(m3.outline_variant));
    for kind in [CardKind::Elevated, CardKind::Filled, CardKind::Outlined] {
        assert_eq!(
            card_frame(kind, &palette).corner_radius,
            egui::CornerRadius::same(radius::MD),
            "{kind:?}"
        );
    }
}
