//! egui visuals for the App's theme, applied live.
//!
//! [`active_tokens`] resolves the App's theme selection (`system` follows the
//! operating system's dark/light setting, as egui reports it) to its
//! [`ThemeTokens`]. [`visuals_for`] builds the window's [`egui::Visuals`]
//! from those tokens alone: panels on `bg.base`, input fields on
//! `bg.raised1`, buttons on `bg.raised2`, floating windows on `bg.overlay`,
//! outlines in the border colours, and a selection that is the accent mixed
//! into the background.
//!
//! [`ThemeApplier`] installs the visuals of the current theme into the egui
//! context and re-installs them whenever the theme changes, so `/theme` or a
//! change of the system setting restyles the window on the next frame.
//! [`contrast_ratio`] is the WCAG 2 contrast ratio the themes' text colours
//! are checked with ([`AA_CONTRAST`], [`AAA_CONTRAST`]).
//!
//! Dependencies: `egui`, `craft-ui` ([`WidgetPalette`]) and
//! `codemason-app-core` (the theme tokens).

use std::sync::Arc;

use codemason_app_core::app::App;
use codemason_app_core::theme_tokens::ThemeTokens;
use craft_ui::theme::WidgetPalette;
use egui::{Color32, CornerRadius, Stroke};

use crate::palette::{Palette, mix, to_rgb};

pub use codemason_app_core::theme_tokens::AA_CONTRAST;

/// WCAG 2 level AAA minimum contrast ratio for body text.
pub const AAA_CONTRAST: f32 = 7.0;

/// How much of the body text colour is mixed into a button's fill while the
/// pointer is over it.
const HOVER_MIX: f32 = 0.12;

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
    codemason_app_core::theme_tokens::contrast_ratio(to_rgb(a), to_rgb(b))
}

/// The tokens of the App's current theme, or of the theme being edited
/// while the theme editor is open (its live preview).
///
/// # Arguments
///
/// * `app` - the App, for its theme selection and catalog.
/// * `ctx` - the window's egui context. Its system theme decides between the
///   dark and light built-in themes when the selection is `system`; a system
///   that reports no preference counts as dark.
#[must_use]
pub fn active_tokens(app: &App, ctx: &egui::Context) -> Arc<ThemeTokens> {
    let system_dark = ctx.system_theme() != Some(egui::Theme::Light);
    app.active_theme_tokens(system_dark)
}

/// The background fills of egui's five widget states in `palette`.
///
/// # Returns
///
/// Resting frames on `raised1`, buttons on `raised2`, a hovered button a
/// little towards the text colour, and a pressed one on the selection
/// colour.
#[must_use]
pub fn widget_palette(palette: &Palette) -> WidgetPalette {
    WidgetPalette {
        noninteractive: palette.raised1,
        inactive: palette.raised2,
        hovered: mix(palette.raised2, palette.text, HOVER_MIX),
        active: palette.selection,
        open: palette.raised2,
    }
}

/// The egui visuals of `tokens`.
///
/// # Returns
///
/// egui's dark or light visuals (by the theme's `dark` flag) with every
/// colour replaced from the tokens: panels on `bg.base`, text fields on
/// `bg.raised1`, widget fills from [`widget_palette`] under the body text
/// colour, floating windows on `bg.overlay`, resting outlines in
/// `border.base` and hovered or pressed ones in `border.focus`, the link,
/// warning and error colours, and the selection colour under body text.
/// Corners use the radius scale: small for widgets, medium for menus, large
/// for windows. A theme with its own Material roles (an `m3` block) is then
/// re-coloured from them by `apply_m3`.
#[must_use]
pub fn visuals_for(tokens: &ThemeTokens) -> egui::Visuals {
    let palette = Palette::from_tokens(tokens);
    let mut visuals = if tokens.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.overlay;
    visuals.window_stroke = Stroke::new(1.0, palette.border);
    visuals.window_corner_radius = CornerRadius::same(crate::metrics::radius::LG);
    visuals.menu_corner_radius = CornerRadius::same(crate::metrics::radius::MD);
    visuals.extreme_bg_color = palette.raised1;
    visuals.faint_bg_color = palette.raised1;
    visuals.code_bg_color = palette.raised1;
    visuals.hyperlink_color = palette.link;
    visuals.warn_fg_color = palette.warning;
    visuals.error_fg_color = palette.error;
    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(1.0, palette.text);
    widget_palette(&palette).apply_to(
        &mut visuals.widgets,
        CornerRadius::same(crate::metrics::radius::SM),
        Stroke::new(1.0, palette.text),
    );
    let widgets = &mut visuals.widgets;
    for (widget, outline) in [
        (&mut widgets.noninteractive, palette.border),
        (&mut widgets.inactive, palette.border),
        (&mut widgets.hovered, palette.border_focus),
        (&mut widgets.active, palette.border_focus),
        (&mut widgets.open, palette.border),
    ] {
        widget.bg_stroke = Stroke::new(1.0, outline);
    }
    if tokens.m3.is_some() {
        apply_m3(&mut visuals, &palette);
    }
    visuals
}

/// Re-colour `visuals` from the Material roles of `palette`, for a theme
/// that carries its own (an `m3` block).
///
/// Panels go on `surface`, windows on `surface_container_high` with
/// extra-large corners and the elevation-3 shadow, menus get extra-small
/// corners and the elevation-2 shadow, text fields and code on
/// `surface_container_highest`, the selection on `primary_container`, links
/// in `primary`, and egui's own buttons become tonal buttons
/// (`secondary_container`, with the Material state layers when hovered and
/// pressed). Labels stay in `on_surface`.
fn apply_m3(visuals: &mut egui::Visuals, palette: &Palette) {
    use crate::material::{HOVER_OPACITY, PRESS_OPACITY, elevation_shadow};
    use crate::metrics::radius;
    use crate::palette::rgb;

    let m3 = &palette.m3;
    visuals.panel_fill = rgb(m3.surface);
    visuals.window_fill = rgb(m3.surface_container_high);
    visuals.window_stroke = Stroke::new(1.0, rgb(m3.outline_variant));
    visuals.window_corner_radius = CornerRadius::same(radius::XL);
    visuals.window_shadow = elevation_shadow(3, palette);
    visuals.popup_shadow = elevation_shadow(2, palette);
    visuals.menu_corner_radius = CornerRadius::same(radius::XS);
    visuals.extreme_bg_color = rgb(m3.surface_container_highest);
    visuals.faint_bg_color = rgb(m3.surface_container);
    visuals.code_bg_color = rgb(m3.surface_container_highest);
    visuals.hyperlink_color = rgb(m3.primary);
    visuals.error_fg_color = rgb(m3.error);
    visuals.selection.bg_fill = rgb(m3.primary_container);
    visuals.selection.stroke = Stroke::new(1.0, rgb(m3.on_primary_container));
    let tonal = rgb(m3.secondary_container);
    let on_tonal = rgb(m3.on_secondary_container);
    WidgetPalette {
        noninteractive: rgb(m3.surface_container),
        inactive: tonal,
        hovered: mix(tonal, on_tonal, HOVER_OPACITY),
        active: mix(tonal, on_tonal, PRESS_OPACITY),
        open: tonal,
    }
    .apply_to(
        &mut visuals.widgets,
        CornerRadius::same(radius::SM),
        Stroke::new(1.0, on_tonal),
    );
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, rgb(m3.on_surface));
    for (widget, outline) in [
        (&mut widgets.noninteractive, m3.outline_variant),
        (&mut widgets.inactive, m3.outline_variant),
        (&mut widgets.hovered, m3.primary),
        (&mut widgets.active, m3.primary),
        (&mut widgets.open, m3.outline_variant),
    ] {
        widget.bg_stroke = Stroke::new(1.0, rgb(outline));
    }
}

/// Keeps the egui context's visuals in step with the App's theme.
#[derive(Debug, Default)]
pub struct ThemeApplier {
    /// The theme whose visuals were last installed, if any.
    applied: Option<Arc<ThemeTokens>>,
}

impl ThemeApplier {
    /// Install the visuals of `tokens` when they are not already installed.
    ///
    /// The visuals go into both of the context's styles and the context is
    /// set to the theme's own dark or light style, so egui's built-in
    /// visuals never show. A change requests a repaint, so the new visuals
    /// are drawn at once.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's egui context.
    /// * `tokens` - the App's current theme, from [`active_tokens`].
    ///
    /// # Returns
    ///
    /// `true` when the visuals were (re)installed this call.
    pub fn sync(&mut self, ctx: &egui::Context, tokens: &Arc<ThemeTokens>) -> bool {
        if self
            .applied
            .as_ref()
            .is_some_and(|applied| applied == tokens)
        {
            return false;
        }
        let visuals = visuals_for(tokens);
        ctx.set_theme(if tokens.dark {
            egui::ThemePreference::Dark
        } else {
            egui::ThemePreference::Light
        });
        ctx.all_styles_mut(|style| style.visuals = visuals.clone());
        self.applied = Some(Arc::clone(tokens));
        ctx.request_repaint();
        true
    }
}
