//! Tests for `codemason_gui::palette`: the GUI colours of each `ThemeColor`
//! in both theme modes, matching the terminal theme.

use codemason_app_core::styled::ThemeColor;
use codemason_app_core::theme_mode::ThemeMode;
use codemason_gui::palette::palette_for;
use egui::Color32;

#[test]
fn test_palette_for_maps_accent_text_error_in_both_modes() {
    let default = palette_for(ThemeMode::Default);
    assert_eq!(
        default.color(ThemeColor::Accent),
        Color32::from_rgb(0, 0, 238)
    );
    assert_eq!(default.color(ThemeColor::Text), Color32::WHITE);
    assert_eq!(
        default.color(ThemeColor::Error),
        Color32::from_rgb(205, 0, 0)
    );

    let high = palette_for(ThemeMode::HighContrast);
    assert_eq!(
        high.color(ThemeColor::Accent),
        Color32::from_rgb(0, 150, 255)
    );
    assert_eq!(high.color(ThemeColor::Text), Color32::WHITE);
    assert_eq!(
        high.color(ThemeColor::Error),
        Color32::from_rgb(255, 50, 50)
    );
}

#[test]
fn test_palette_literal_colors_use_xterm_table() {
    let palette = palette_for(ThemeMode::Default);
    assert_eq!(
        palette.color(ThemeColor::Rgb(1, 2, 3)),
        Color32::from_rgb(1, 2, 3)
    );
    assert_eq!(
        palette.color(ThemeColor::DarkGray),
        Color32::from_rgb(127, 127, 127)
    );
    // 0..16 are the ANSI colours themselves.
    assert_eq!(
        palette.color(ThemeColor::Indexed(1)),
        palette.color(ThemeColor::Red)
    );
    // 16..232 is the 6x6x6 cube with levels 0, 95, 135, 175, 215, 255.
    assert_eq!(
        palette.color(ThemeColor::Indexed(16)),
        Color32::from_rgb(0, 0, 0)
    );
    assert_eq!(
        palette.color(ThemeColor::Indexed(67)),
        Color32::from_rgb(95, 135, 175)
    );
    assert_eq!(
        palette.color(ThemeColor::Indexed(231)),
        Color32::from_rgb(255, 255, 255)
    );
    // 232..256 is the grey ramp 8, 18, ..., 238.
    assert_eq!(
        palette.color(ThemeColor::Indexed(232)),
        Color32::from_rgb(8, 8, 8)
    );
    assert_eq!(
        palette.color(ThemeColor::Indexed(255)),
        Color32::from_rgb(238, 238, 238)
    );
}
