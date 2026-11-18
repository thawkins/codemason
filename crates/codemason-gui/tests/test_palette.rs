//! Tests for `codemason_gui::palette`: the GUI colours of each `ThemeColor`
//! come from the theme's tokens.

use codemason_app_core::styled::ThemeColor;
use codemason_app_core::theme_tokens::{Rgb, ThemeCatalog, ThemeSelection};
use codemason_gui::palette::{Palette, mix, rgb, to_rgb};
use egui::Color32;

/// The palette of the built-in theme `name`.
fn palette(name: &str) -> Palette {
    let themes = ThemeCatalog::builtin();
    let tokens = themes.get(name).expect("built-in theme");
    Palette::from_tokens(&tokens)
}

#[test]
fn test_palette_semantic_colors_are_the_theme_tokens() {
    for name in ThemeCatalog::builtin().names() {
        let tokens = ThemeCatalog::builtin().get(name).expect("listed");
        let p = Palette::from_tokens(&tokens);
        let expect = [
            (ThemeColor::Accent, tokens.accent),
            (ThemeColor::Text, tokens.text.base),
            (ThemeColor::Reset, tokens.text.base),
            (ThemeColor::Muted, tokens.text.muted),
            (ThemeColor::Subtle, tokens.text.subtle),
            (ThemeColor::Error, tokens.state.error),
            (ThemeColor::Warning, tokens.state.warning),
            (ThemeColor::Success, tokens.state.success),
            (ThemeColor::Info, tokens.state.info),
            (ThemeColor::Border, tokens.border.base),
            (ThemeColor::Raised, tokens.bg.raised1),
            (ThemeColor::DiffAdded, tokens.diff.added),
            (ThemeColor::DiffRemoved, tokens.diff.removed),
            (ThemeColor::Heading, tokens.markdown.heading),
            (ThemeColor::Link, tokens.markdown.link),
            (ThemeColor::Code, tokens.markdown.code),
            (ThemeColor::Blockquote, tokens.markdown.blockquote),
            (ThemeColor::Categorical(0), tokens.categorical[0]),
            (ThemeColor::Categorical(7), tokens.categorical[7]),
            // The index wraps.
            (ThemeColor::Categorical(9), tokens.categorical[1]),
        ];
        for (color, token) in expect {
            assert_eq!(p.color(color), rgb(token), "{name} {color:?}");
        }
        assert_eq!(p.background, rgb(tokens.bg.base), "{name}");
        assert_eq!(p.dark, tokens.dark, "{name}");
    }
}

#[test]
fn test_palette_differs_between_dark_and_light() {
    let (dark, light) = (palette("codemason-dark"), palette("codemason-light"));
    assert_eq!(dark.accent, Color32::from_rgb(95, 175, 255));
    assert_eq!(dark.background, Color32::from_rgb(15, 17, 21));
    assert_eq!(light.background, Color32::from_rgb(250, 250, 250));
    assert_ne!(dark.text, light.text);
    let system = ThemeCatalog::builtin();
    assert_eq!(
        Palette::from_tokens(&system.resolve(&ThemeSelection::System, false)),
        light
    );
}

#[test]
fn test_palette_ansi_names_follow_the_theme() {
    for name in ["codemason-dark", "codemason-light", "nord"] {
        let p = palette(name);
        let same = [
            (ThemeColor::Black, p.background),
            (ThemeColor::White, p.text),
            (ThemeColor::Gray, p.muted),
            (ThemeColor::DarkGray, p.subtle),
            (ThemeColor::Red, p.error),
            (ThemeColor::LightRed, p.error),
            (ThemeColor::Green, p.success),
            (ThemeColor::LightGreen, p.success),
            (ThemeColor::Yellow, p.warning),
            (ThemeColor::LightYellow, p.warning),
            (ThemeColor::Blue, p.accent),
            (ThemeColor::LightBlue, p.accent),
            (ThemeColor::Cyan, p.info),
            (ThemeColor::LightCyan, p.info),
            (ThemeColor::Magenta, p.categorical[4]),
            (ThemeColor::LightMagenta, p.categorical[4]),
        ];
        for (ansi, expected) in same {
            assert_eq!(p.color(ansi), expected, "{name} {ansi:?}");
        }
    }
}

#[test]
fn test_palette_literal_colors_use_xterm_table() {
    let palette = palette("codemason-dark");
    assert_eq!(
        palette.color(ThemeColor::Rgb(1, 2, 3)),
        Color32::from_rgb(1, 2, 3)
    );
    // 0..16 are the ANSI names themselves.
    assert_eq!(
        palette.color(ThemeColor::Indexed(1)),
        palette.color(ThemeColor::Red)
    );
    assert_eq!(
        palette.color(ThemeColor::Indexed(15)),
        palette.color(ThemeColor::White)
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
    for index in 0..=255u8 {
        assert_eq!(palette.color(ThemeColor::Indexed(index)).a(), 255);
    }
}

#[test]
fn test_palette_mix_and_rgb_round_trip() {
    let (black, white) = (Color32::BLACK, Color32::WHITE);
    assert_eq!(mix(black, white, 0.0), black);
    assert_eq!(mix(black, white, 1.0), white);
    assert_eq!(mix(black, white, 0.5), Color32::from_rgb(128, 128, 128));
    assert_eq!(to_rgb(rgb(Rgb(1, 2, 3))), Rgb(1, 2, 3));
    // The selection is the accent mixed into the background.
    let p = palette("codemason-dark");
    assert_eq!(p.selection, mix(p.background, p.accent, 0.35));
}
