//! Tests for `codemason_gui::theme`: every built-in theme has readable egui
//! visuals built from its tokens alone, and the window switches visuals live
//! when the App's theme or the system's dark/light setting changes. The theme
//! picker dialog is tested in `test_dialogs_more.rs`.

#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;

use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection, ThemeTokens};
use codemason_gui::Shell;
use codemason_gui::palette::Palette;
use codemason_gui::theme::{
    AA_CONTRAST, AAA_CONTRAST, ThemeApplier, active_tokens, contrast_ratio, visuals_for,
    widget_palette,
};
use egui::Color32;
use egui_kittest::Harness;

/// Every built-in theme.
fn themes() -> Vec<Arc<ThemeTokens>> {
    let catalog = ThemeCatalog::builtin();
    catalog
        .names()
        .map(|name| catalog.get(name).expect("listed theme"))
        .collect()
}

/// The built-in theme `name`.
fn theme(name: &str) -> Arc<ThemeTokens> {
    ThemeCatalog::builtin().get(name).expect("built-in theme")
}

fn named(name: &str) -> ThemeSelection {
    ThemeSelection::Named(name.to_string())
}

/// A window on an App with the built-in catalog and the `system` theme.
fn shell_harness<'a>() -> (tokio::runtime::Runtime, Harness<'a, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::System;
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), Shell::new(app));
    (runtime, harness)
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
    // ANSI blue on black, the terminal's default accent, is below AA.
    let ansi_blue = contrast_ratio(Color32::from_rgb(0, 0, 238), Color32::BLACK);
    assert!(ansi_blue < AA_CONTRAST, "{ansi_blue}");
}

#[test]
fn test_theme_text_is_readable_in_every_builtin_theme() {
    for tokens in themes() {
        let name = &tokens.name;
        let palette = Palette::from_tokens(&tokens);
        let visuals = visuals_for(&tokens);
        // Body and secondary text on the panel, and on every raised surface.
        for surface in [
            palette.background,
            palette.raised1,
            palette.raised2,
            palette.overlay,
        ] {
            for text in [palette.text, palette.muted] {
                let ratio = contrast_ratio(text, surface);
                assert!(ratio >= AA_CONTRAST, "{name} text on surface {ratio:.2}");
            }
        }
        // Selected text.
        let selected = contrast_ratio(visuals.selection.stroke.color, visuals.selection.bg_fill);
        assert!(selected >= AA_CONTRAST, "{name} selection {selected:.2}");
        // Labels on the panel.
        let label = contrast_ratio(
            visuals.widgets.noninteractive.fg_stroke.color,
            visuals.panel_fill,
        );
        assert!(label >= AA_CONTRAST, "{name} label {label:.2}");
        if tokens.m3.is_none() {
            assert_eq!(visuals.selection.bg_fill, palette.selection, "{name}");
            assert_eq!(visuals.hyperlink_color, palette.link, "{name}");
        }
    }
    let dark = Palette::from_tokens(&theme("codemason-dark"));
    assert!(contrast_ratio(dark.accent, dark.background) >= AAA_CONTRAST);
}

#[test]
fn test_theme_visuals_come_from_the_tokens() {
    // Themes with their own Material roles are covered by
    // `test_theme_m3_visuals_come_from_the_roles`.
    for tokens in themes().into_iter().filter(|t| t.m3.is_none()) {
        let name = &tokens.name;
        let visuals = visuals_for(&tokens);
        let palette = Palette::from_tokens(&tokens);
        assert_eq!(visuals.dark_mode, tokens.dark, "{name}");
        assert_eq!(visuals.panel_fill, palette.background, "{name}");
        assert_eq!(visuals.window_fill, palette.overlay, "{name}");
        assert_eq!(visuals.extreme_bg_color, palette.raised1, "{name}");
        assert_eq!(visuals.window_stroke.color, palette.border, "{name}");
        assert_eq!(visuals.warn_fg_color, palette.warning, "{name}");
        assert_eq!(visuals.error_fg_color, palette.error, "{name}");
        // Widget fills come from the theme's WidgetPalette, and widget text
        // is readable on every fill.
        let fills = widget_palette(&palette);
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
            assert_eq!(widget.bg_fill, fill, "{name} {state}");
            assert_eq!(widget.weak_bg_fill, fill, "{name} {state}");
            let ratio = contrast_ratio(widget.fg_stroke.color, fill);
            assert!(ratio >= AA_CONTRAST, "{name} {state} text {ratio:.2}");
        }
        assert_eq!(widgets.inactive.bg_stroke.color, palette.border, "{name}");
        assert_eq!(
            widgets.hovered.bg_stroke.color, palette.border_focus,
            "{name}"
        );
    }
    assert_ne!(
        visuals_for(&theme("codemason-dark")),
        visuals_for(&theme("codemason-light"))
    );
    // High contrast outlines every widget and window in full white.
    let high = visuals_for(&theme("high-contrast"));
    assert_eq!(high.window_stroke.color, Color32::WHITE);
    assert_eq!(high.widgets.inactive.bg_stroke.color, Color32::WHITE);
    assert_eq!(high.panel_fill, Color32::BLACK);
}

#[test]
fn test_theme_applier_applies_only_on_change() {
    let ctx = egui::Context::default();
    let (dark, light) = (theme("codemason-dark"), theme("codemason-light"));
    let mut applier = ThemeApplier::default();
    assert!(applier.sync(&ctx, &dark));
    assert_eq!(ctx.global_style().visuals, visuals_for(&dark));
    assert!(!applier.sync(&ctx, &dark));
    // An equal theme behind another Arc is not a change.
    assert!(!applier.sync(&ctx, &Arc::new((*dark).clone())));
    assert!(applier.sync(&ctx, &light));
    assert_eq!(ctx.global_style().visuals, visuals_for(&light));
    assert!(!ctx.global_style().visuals.dark_mode);
    // egui's own dark visuals never show, whichever style it selects.
    ctx.set_theme(egui::Theme::Dark);
    assert_eq!(ctx.global_style().visuals, visuals_for(&light));
}

#[test]
fn test_theme_switches_live_with_app_theme() {
    let (_runtime, mut harness) = shell_harness();
    harness.run();
    // The test window reports no system preference, which counts as dark.
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(&theme("codemason-dark"))
    );

    for name in ["high-contrast", "codemason-light", "nord"] {
        harness.state_mut().app_mut().theme = named(name);
        harness.run();
        assert_eq!(
            harness.ctx.global_style().visuals,
            visuals_for(&theme(name)),
            "{name}"
        );
    }

    harness.state_mut().app_mut().theme = ThemeSelection::System;
    harness.run();
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(&theme("codemason-dark"))
    );
}

#[test]
fn test_theme_system_follows_the_systems_light_setting() {
    let (_runtime, mut harness) = shell_harness();
    harness.run();
    harness.input_mut().system_theme = Some(egui::Theme::Light);
    harness.run();
    assert_eq!(
        active_tokens(harness.state().app(), &harness.ctx).name,
        "codemason-light"
    );
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(&theme("codemason-light"))
    );

    // A named theme ignores the system setting.
    harness.state_mut().app_mut().theme = named("nord");
    harness.input_mut().system_theme = Some(egui::Theme::Light);
    harness.run();
    assert_eq!(
        harness.ctx.global_style().visuals,
        visuals_for(&theme("nord"))
    );
}

/// `/theme high-contrast` changes the visuals the window is drawn with.
#[tokio::test]
async fn test_theme_command_changes_the_palette() {
    support::scratch_config_target();
    let mut app = support::make_app();
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::System;
    let before = visuals_for(&app.themes.resolve(&app.theme, true));

    app.execute_slash_command("/theme high-contrast").await;

    assert_eq!(app.theme, named("high-contrast"));
    let after = visuals_for(&app.themes.resolve(&app.theme, true));
    assert_ne!(
        before.window_stroke, after.window_stroke,
        "high contrast draws different outlines"
    );
}

#[test]
fn test_theme_m3_visuals_come_from_the_roles() {
    use codemason_gui::palette::rgb;

    for name in ["document-light", "document-dark"] {
        let tokens = theme(name);
        let m3 = tokens.roles();
        let visuals = visuals_for(&tokens);
        assert_eq!(visuals.dark_mode, tokens.dark, "{name}");
        assert_eq!(visuals.panel_fill, rgb(m3.surface), "{name}");
        assert_eq!(
            visuals.window_fill,
            rgb(m3.surface_container_high),
            "{name}"
        );
        assert_eq!(
            visuals.extreme_bg_color,
            rgb(m3.surface_container_highest),
            "{name}"
        );
        assert_eq!(
            visuals.selection.bg_fill,
            rgb(m3.primary_container),
            "{name}"
        );
        assert_eq!(visuals.hyperlink_color, rgb(m3.primary), "{name}");
        assert_eq!(
            visuals.window_corner_radius,
            egui::CornerRadius::same(codemason_gui::metrics::radius::XL)
        );
        let widgets = &visuals.widgets;
        assert_eq!(
            widgets.inactive.bg_fill,
            rgb(m3.secondary_container),
            "{name}"
        );
        assert_eq!(
            widgets.noninteractive.fg_stroke.color,
            rgb(m3.on_surface),
            "{name}"
        );
        for (state, widget) in [
            ("inactive", &widgets.inactive),
            ("hovered", &widgets.hovered),
            ("active", &widgets.active),
        ] {
            let ratio = contrast_ratio(widget.fg_stroke.color, widget.bg_fill);
            assert!(ratio >= AA_CONTRAST, "{name} {state} text {ratio:.2}");
        }
    }
}
