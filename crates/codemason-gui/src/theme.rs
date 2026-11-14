//! egui visuals for the App's [`ThemeMode`], applied live.
//!
//! [`visuals_for`] builds the window's [`egui::Visuals`] for a theme mode
//! from its [`crate::palette::Palette`]: dark panels on the terminal theme's
//! black background, widget fills from [`widget_palette`] (applied with
//! craft-ui's [`WidgetPalette::apply_to`]), the accent as the link colour,
//! and readable selection colours. The high-contrast mode outlines every
//! widget and window in white. Both modes are dark: app-core has no light
//! theme mode.
//!
//! [`ThemeApplier`] installs the visuals of the App's current mode into the
//! egui context and re-installs them whenever the mode changes, so a change
//! of `App::theme_mode` restyles the window on the next frame.
//! [`contrast_ratio`] is the WCAG 2 contrast ratio the palette's text
//! colours are checked with ([`AA_CONTRAST`], [`AAA_CONTRAST`]).
//!
//! Dependencies: `egui`, `craft-ui` ([`WidgetPalette`]) and
//! `codemason-app-core` ([`ThemeMode`]).

use codemason_app_core::theme_mode::ThemeMode;
use craft_ui::theme::WidgetPalette;
use egui::{Color32, CornerRadius, Stroke};

use crate::palette::palette_for;

/// WCAG 2 level AA minimum contrast ratio for body text.
pub const AA_CONTRAST: f32 = 4.5;

/// WCAG 2 level AAA minimum contrast ratio for body text.
pub const AAA_CONTRAST: f32 = 7.0;

/// Corner radius of buttons and other framed widgets.
const WIDGET_RADIUS: CornerRadius = CornerRadius::same(2);

/// The WCAG 2 relative luminance of one sRGB channel value.
fn channel_luminance(value: u8) -> f32 {
    let c = f32::from(value) / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The WCAG 2 relative luminance of `color` (its alpha is ignored).
fn relative_luminance(color: Color32) -> f32 {
    let red = 0.2126 * channel_luminance(color.r());
    let green = 0.7152f32.mul_add(channel_luminance(color.g()), red);
    0.0722f32.mul_add(channel_luminance(color.b()), green)
}

/// The WCAG 2 contrast ratio between two opaque colours.
///
/// # Arguments
///
/// * `a` - one colour, e.g. the text colour.
/// * `b` - the other, e.g. the background it is drawn on.
///
/// # Returns
///
/// `(L1 + 0.05) / (L2 + 0.05)` for the lighter (`L1`) and darker (`L2`)
/// relative luminance: 1.0 for equal colours, 21.0 for white on black. The
/// ratio is symmetric.
#[must_use]
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// The background fills of egui's five widget states in `mode`.
///
/// # Arguments
///
/// * `mode` - the theme mode the App is in.
///
/// # Returns
///
/// Dark greys for the default mode; black resting fills, which the
/// high-contrast mode's white outlines frame, for high contrast. White text
/// is at least AAA-readable on every fill.
#[must_use]
pub fn widget_palette(mode: ThemeMode) -> WidgetPalette {
    match mode {
        ThemeMode::Default => WidgetPalette {
            noninteractive: Color32::from_rgb(24, 24, 24),
            inactive: Color32::from_rgb(56, 56, 56),
            hovered: Color32::from_rgb(76, 76, 76),
            active: Color32::from_rgb(0, 72, 140),
            open: Color32::from_rgb(40, 40, 40),
        },
        ThemeMode::HighContrast => WidgetPalette {
            noninteractive: Color32::BLACK,
            inactive: Color32::BLACK,
            hovered: Color32::from_rgb(48, 48, 48),
            active: Color32::from_rgb(0, 60, 120),
            open: Color32::from_rgb(32, 32, 32),
        },
    }
}

/// The egui visuals of `mode`.
///
/// # Arguments
///
/// * `mode` - the theme mode the App is in.
///
/// # Returns
///
/// egui's dark visuals with the mode's palette: panels on
/// [`crate::palette::Palette::background`], widget fills from
/// [`widget_palette`] with [`crate::palette::Palette::text`] as their
/// foreground, the accent as the link colour, the warning and error colours
/// for egui's own warnings and errors, and a dark-blue selection under
/// white text. High contrast also draws every widget and window outline in
/// white.
#[must_use]
pub fn visuals_for(mode: ThemeMode) -> egui::Visuals {
    let palette = palette_for(mode);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = palette.background;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warning;
    visuals.error_fg_color = palette.error;
    visuals.selection.stroke = Stroke::new(1.0, palette.text);
    widget_palette(mode).apply_to(
        &mut visuals.widgets,
        WIDGET_RADIUS,
        Stroke::new(1.0, palette.text),
    );
    match mode {
        ThemeMode::Default => {
            visuals.window_fill = Color32::from_rgb(24, 24, 24);
            visuals.window_stroke = Stroke::new(1.0, palette.border);
            visuals.extreme_bg_color = Color32::from_rgb(16, 16, 16);
            visuals.faint_bg_color = Color32::from_rgb(20, 20, 20);
            visuals.selection.bg_fill = Color32::from_rgb(0, 72, 140);
        }
        ThemeMode::HighContrast => {
            let outline = Stroke::new(1.0, Color32::WHITE);
            visuals.window_fill = palette.background;
            visuals.window_stroke = outline;
            visuals.extreme_bg_color = palette.background;
            visuals.faint_bg_color = Color32::from_rgb(16, 16, 16);
            visuals.selection.bg_fill = Color32::from_rgb(0, 60, 120);
            let widgets = &mut visuals.widgets;
            for widget in [
                &mut widgets.noninteractive,
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.open,
            ] {
                widget.bg_stroke = outline;
            }
        }
    }
    visuals
}

/// Keeps the egui context's visuals in step with the App's theme mode.
#[derive(Debug, Default)]
pub struct ThemeApplier {
    /// The mode whose visuals were last installed, if any.
    applied: Option<ThemeMode>,
}

impl ThemeApplier {
    /// Install the visuals of `mode` when they are not already installed.
    ///
    /// The visuals go into both of the context's styles and the context is
    /// pinned to its dark style, so the system's light/dark preference
    /// cannot swap in egui's own light visuals. A change requests a repaint,
    /// so the new visuals are drawn at once.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's egui context.
    /// * `mode` - the App's current theme mode.
    ///
    /// # Returns
    ///
    /// `true` when the visuals were (re)installed this call.
    pub fn sync(&mut self, ctx: &egui::Context, mode: ThemeMode) -> bool {
        if self.applied == Some(mode) {
            return false;
        }
        let visuals = visuals_for(mode);
        ctx.set_theme(egui::ThemePreference::Dark);
        ctx.all_styles_mut(|style| style.visuals = visuals.clone());
        self.applied = Some(mode);
        ctx.request_repaint();
        true
    }
}
