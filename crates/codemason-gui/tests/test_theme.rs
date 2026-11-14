//! Tests for `codemason_gui::theme`: every `ThemeColor` resolves in every
//! theme mode, the accent is readable on the panel background, each mode
//! has its own egui visuals, and the window switches visuals live when the
//! App's theme mode changes.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::styled::ThemeColor;
use codemason_app_core::theme_mode::ThemeMode;
use codemason_gui::Shell;
use codemason_gui::palette::palette_for;
use codemason_gui::theme::{
    AA_CONTRAST, AAA_CONTRAST, ThemeApplier, contrast_ratio, visuals_for, widget_palette,
};
use egui::Color32;
use egui_kittest::Harness;

/// Every theme mode the App can be in.
const MODES: [ThemeMode; 2] = [ThemeMode::Default, ThemeMode::HighContrast];

/// The standard xterm values of the 16 ANSI colours, in index order.
const XTERM_ANSI: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// The 16 ANSI variants, in xterm index order.
const ANSI: [ThemeColor; 16] = [
    ThemeColor::Black,
    ThemeColor::Red,
    ThemeColor::Green,
    ThemeColor::Yellow,
    ThemeColor::Blue,
    ThemeColor::Magenta,
    ThemeColor::Cyan,
    ThemeColor::Gray,
    ThemeColor::DarkGray,
    ThemeColor::LightRed,
    ThemeColor::LightGreen,
    ThemeColor::LightYellow,
    ThemeColor::LightBlue,
    ThemeColor::LightMagenta,
    ThemeColor::LightCyan,
    ThemeColor::White,
];

/// The semantic variants.
const SEMANTIC: [ThemeColor; 9] = [
    ThemeColor::Accent,
    ThemeColor::Text,
    ThemeColor::Reset,
    ThemeColor::Muted,
    ThemeColor::Error,
    ThemeColor::Warning,
    ThemeColor::Success,
    ThemeColor::Info,
    ThemeColor::Border,
];

/// Which group `color` belongs to. The match has no wildcard arm, so a new
/// `ThemeColor` variant fails to compile here until the test covers it.
fn group(color: ThemeColor) -> &'static str {
    match color {
        ThemeColor::Accent
        | ThemeColor::Text
        | ThemeColor::Reset
        | ThemeColor::Muted
        | ThemeColor::Error
        | ThemeColor::Warning
        | ThemeColor::Success
        | ThemeColor::Info
        | ThemeColor::Border => "semantic",
        ThemeColor::Black
        | ThemeColor::Red
        | ThemeColor::Green
        | ThemeColor::Yellow
        | ThemeColor::Blue
        | ThemeColor::Magenta
        | ThemeColor::Cyan
        | ThemeColor::Gray
        | ThemeColor::DarkGray
        | ThemeColor::LightRed
        | ThemeColor::LightGreen
        | ThemeColor::LightYellow
        | ThemeColor::LightBlue
        | ThemeColor::LightMagenta
        | ThemeColor::LightCyan
        | ThemeColor::White => "ansi",
        ThemeColor::Indexed(_) => "indexed",
        ThemeColor::Rgb(..) => "rgb",
    }
}

#[test]
fn test_theme_every_theme_color_resolves() {
    for mode in MODES {
        let palette = palette_for(mode);
        // Semantic colours: opaque, and the text colours (accent, text,
        // muted) readable on the panel background. The status colours keep
        // the terminal's values (ANSI red is 3.6:1 on black).
        for color in SEMANTIC {
            assert_eq!(group(color), "semantic");
            let resolved = palette.color(color);
            assert_eq!(resolved.a(), 255, "{mode:?} {color:?} is opaque");
            let text_color = matches!(
                color,
                ThemeColor::Accent | ThemeColor::Text | ThemeColor::Reset | ThemeColor::Muted
            );
            if text_color {
                let ratio = contrast_ratio(resolved, palette.background);
                assert!(
                    ratio >= AA_CONTRAST,
                    "{mode:?} {color:?} {resolved:?} contrast {ratio:.2} < {AA_CONTRAST}"
                );
            }
        }
        assert_eq!(palette.color(ThemeColor::Reset), palette.text);
        // The 16 ANSI colours are the xterm values.
        for (i, color) in ANSI.into_iter().enumerate() {
            assert_eq!(group(color), "ansi");
            let (r, g, b) = XTERM_ANSI[i];
            assert_eq!(
                palette.color(color),
                Color32::from_rgb(r, g, b),
                "{mode:?} {color:?}"
            );
        }
        // Every xterm-256 index resolves to an opaque colour; 0..16 are the
        // ANSI colours, then the 6x6x6 cube and the grey ramp.
        for index in 0..=255u8 {
            let color = ThemeColor::Indexed(index);
            assert_eq!(group(color), "indexed");
            let resolved = palette.color(color);
            assert_eq!(resolved.a(), 255, "{mode:?} Indexed({index})");
            if index < 16 {
                assert_eq!(resolved, palette.color(ANSI[usize::from(index)]));
            }
        }
        assert_eq!(
            palette.color(ThemeColor::Indexed(196)),
            Color32::from_rgb(255, 0, 0)
        );
        assert_eq!(
            palette.color(ThemeColor::Indexed(244)),
            Color32::from_rgb(128, 128, 128)
        );
        // Literal colours map as given.
        for (r, g, b) in [(0, 0, 0), (1, 2, 3), (255, 255, 255), (18, 52, 86)] {
            let color = ThemeColor::Rgb(r, g, b);
            assert_eq!(group(color), "rgb");
            assert_eq!(palette.color(color), Color32::from_rgb(r, g, b));
        }
    }
}

#[test]
fn test_theme_contrast_ratio_matches_wcag() {
    let white_on_black = contrast_ratio(Color32::WHITE, Color32::BLACK);
    assert!((white_on_black - 21.0).abs() < 0.01, "{white_on_black}");
    // The ratio is symmetric.
    let black_on_white = contrast_ratio(Color32::BLACK, Color32::WHITE);
    assert!((black_on_white - white_on_black).abs() < 0.001);
    let same = contrast_ratio(Color32::from_rgb(90, 90, 90), Color32::from_rgb(90, 90, 90));
    assert!((same - 1.0).abs() < 0.001, "{same}");
    // ANSI blue on black, the TUI's default accent, is below AA.
    let ansi_blue = contrast_ratio(Color32::from_rgb(0, 0, 238), Color32::BLACK);
    assert!(ansi_blue < AA_CONTRAST, "{ansi_blue}");
}

#[test]
fn test_theme_accent_is_readable_on_background() {
    let default = palette_for(ThemeMode::Default);
    assert_eq!(default.accent, Color32::from_rgb(95, 175, 255));
    let ratio = contrast_ratio(default.accent, default.background);
    assert!(ratio >= AAA_CONTRAST, "default accent contrast {ratio:.2}");

    let high = palette_for(ThemeMode::HighContrast);
    let ratio = contrast_ratio(high.accent, high.background);
    assert!(
        ratio >= AA_CONTRAST,
        "high-contrast accent contrast {ratio:.2}"
    );

    // Accent-coloured widget text (links, selected text) is readable too.
    for mode in MODES {
        let visuals = visuals_for(mode);
        let palette = palette_for(mode);
        assert_eq!(visuals.hyperlink_color, palette.accent);
        let selected = contrast_ratio(visuals.selection.stroke.color, visuals.selection.bg_fill);
        assert!(selected >= AA_CONTRAST, "{mode:?} selection {selected:.2}");
    }
}

#[test]
fn test_theme_visuals_follow_mode() {
    for mode in MODES {
        let visuals = visuals_for(mode);
        let palette = palette_for(mode);
        assert!(
            visuals.dark_mode,
            "{mode:?} draws light text on dark panels"
        );
        assert_eq!(visuals.panel_fill, palette.background);
        // Widget fills come from the mode's WidgetPalette, and widget text
        // is readable on every fill.
        let fills = widget_palette(mode);
        let widgets = &visuals.widgets;
        for (state, fill, widget) in [
            (
                "noninteractive",
                fills.noninteractive,
                &widgets.noninteractive,
            ),
            ("inactive", fills.inactive, &widgets.inactive),
            ("hovered", fills.hovered, &widgets.hovered),
            ("active", fills.active, &widgets.active),
            ("open", fills.open, &widgets.open),
        ] {
            assert_eq!(widget.bg_fill, fill, "{mode:?} {state}");
            assert_eq!(widget.weak_bg_fill, fill, "{mode:?} {state}");
            let ratio = contrast_ratio(widget.fg_stroke.color, fill);
            assert!(ratio >= AA_CONTRAST, "{mode:?} {state} text {ratio:.2}");
        }
    }
    assert_ne!(
        visuals_for(ThemeMode::Default),
        visuals_for(ThemeMode::HighContrast)
    );
    // High contrast outlines every widget and window in full white.
    let high = visuals_for(ThemeMode::HighContrast);
    assert_eq!(high.window_stroke.color, Color32::WHITE);
    assert_eq!(high.widgets.inactive.bg_stroke.color, Color32::WHITE);
}

#[test]
fn test_theme_applier_applies_only_on_change() {
    let ctx = egui::Context::default();
    let mut theme = ThemeApplier::default();
    assert!(theme.sync(&ctx, ThemeMode::Default));
    assert_eq!(ctx.global_style().visuals, visuals_for(ThemeMode::Default));
    assert!(!theme.sync(&ctx, ThemeMode::Default));
    assert!(theme.sync(&ctx, ThemeMode::HighContrast));
    assert_eq!(
        ctx.global_style().visuals,
        visuals_for(ThemeMode::HighContrast)
    );
    // The system's light/dark preference does not pick another style.
    ctx.set_theme(egui::Theme::Light);
    assert_eq!(
        ctx.global_style().visuals,
        visuals_for(ThemeMode::HighContrast)
    );
}

#[test]
fn test_theme_switches_live_with_app_mode() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    let shell = Shell::new(app);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(ThemeMode::Default)
    );

    harness.state_mut().app_mut().theme_mode = ThemeMode::HighContrast;
    harness.run();
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(ThemeMode::HighContrast)
    );

    harness.state_mut().app_mut().theme_mode = ThemeMode::Default;
    harness.run();
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(ThemeMode::Default)
    );
}
