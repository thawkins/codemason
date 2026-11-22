//! Tests for `codemason_gui::material::badge`: size, accessible label and
//! the contrast of every tone in every built-in theme.

#[path = "support_material.rs"]
mod support_material;

use codemason_app_core::theme_tokens::ThemeCatalog;
use codemason_gui::material::badge::{BadgeTone, HEIGHT, badge, badge_colours};
use codemason_gui::palette::Palette;
use codemason_gui::theme::{AA_CONTRAST, contrast_ratio};
use egui_kittest::kittest::Queryable;

const TONES: [BadgeTone; 6] = [
    BadgeTone::Success,
    BadgeTone::Primary,
    BadgeTone::Secondary,
    BadgeTone::Tertiary,
    BadgeTone::Error,
    BadgeTone::Neutral,
];

#[test]
fn test_material_badge_has_label_and_height() {
    let palette = support_material::palette();
    let harness = support_material::builder().build_ui(move |ui| {
        badge(ui, &palette, BadgeTone::Success, "Installed");
    });
    let rect = harness.get_by_label("Installed").rect();
    assert!((rect.height() - HEIGHT).abs() < 0.5, "{rect:?}");
}

#[test]
fn test_material_badge_has_label_and_contrast() {
    let catalog = ThemeCatalog::builtin();
    for name in catalog.names() {
        let palette = Palette::from_tokens(&catalog.get(name).expect("listed theme"));
        for tone in TONES {
            let (fill, text) = badge_colours(&palette, tone);
            let ratio = contrast_ratio(text, fill);
            assert!(ratio >= AA_CONTRAST, "{name} {tone:?} {ratio:.2}");
        }
    }
}
