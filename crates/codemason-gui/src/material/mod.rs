//! Material Design 3 components drawn with egui.
//!
//! Each component takes the window's [`Palette`] and draws with its
//! Material colour roles ([`Palette::m3`]), so it follows the theme like the
//! rest of the window. Components keep no state of their own beyond what
//! egui keeps by widget id; the caller owns the value a component edits.
//!
//! The modules, one per component family:
//!
//! - [`button`]: filled, tonal, outlined and text buttons;
//! - [`icon_button`]: standard, filled and toggle icon buttons;
//! - [`segmented`]: segmented buttons;
//! - [`tabs`]: primary tabs;
//! - [`list`]: dense one- and two-line list items;
//! - [`menu`]: the menu surface and its items;
//! - [`card`]: elevated, filled and outlined cards;
//! - [`text_field`]: the outlined text field;
//! - [`dialog`]: the basic dialog surface, headline, body and actions;
//! - [`sheet`]: the navigation drawer, its items, and side sheets;
//! - [`chip`]: assist and filter chips;
//! - [`badge`]: small state-label pills;
//! - [`thumbnail`]: a removable image thumbnail;
//! - [`feedback`]: slider, linear progress, plain tooltip and snackbar.
//!
//! The helpers here are the parts every component shares: the state layer
//! drawn over a hovered, focused or pressed component, the focus ring,
//! elevation shadows, and motion ([`motion_time`], [`standard_easing`]).
//!
//! Dependencies: `egui`, and this crate's palette, icons, fonts and metrics.

pub mod badge;
pub mod button;
pub mod card;
pub mod chip;
pub mod dialog;
pub mod feedback;
pub mod icon_button;
pub mod list;
pub mod menu;
pub mod segmented;
pub mod sheet;
pub mod tabs;
pub mod text_field;
pub mod thumbnail;

use egui::{Color32, CornerRadius, Response, Stroke, StrokeKind};

use codemason_app_core::theme_tokens::{M3Roles, Rgb};

use crate::icons::{Icon, icon_label};
use crate::palette::{Palette, rgb};

/// Opacity of the state layer over a hovered component.
pub const HOVER_OPACITY: f32 = 0.08;

/// Opacity of the state layer over a focused or pressed component.
pub const PRESS_OPACITY: f32 = 0.10;

/// Opacity of a disabled component's content.
pub const DISABLED_CONTENT_OPACITY: f32 = 0.38;

/// Opacity of a disabled component's container.
pub const DISABLED_CONTAINER_OPACITY: f32 = 0.12;

/// Width of the focus ring, in points.
const FOCUS_RING_WIDTH: f32 = 3.0;

/// Gap between a component and its focus ring, in points.
const FOCUS_RING_GAP: f32 = 2.0;

/// The opacity of the state layer for `response`'s interaction state.
///
/// # Returns
///
/// [`PRESS_OPACITY`] while pressed or focused, [`HOVER_OPACITY`] while
/// hovered, else 0.0.
#[must_use]
pub fn state_opacity(response: &Response) -> f32 {
    if response.is_pointer_button_down_on() || response.has_focus() {
        PRESS_OPACITY
    } else if response.hovered() {
        HOVER_OPACITY
    } else {
        0.0
    }
}

/// Paint the state layer: `content` at the state opacity over `rect`.
///
/// # Arguments
///
/// * `ui` - the ui to paint in.
/// * `response` - the component's response, for its state.
/// * `radius` - the component's corner radius.
/// * `content` - the component's content colour, which tints the layer.
pub fn paint_state_layer(
    ui: &egui::Ui,
    response: &Response,
    radius: CornerRadius,
    content: Color32,
) {
    let opacity = state_opacity(response);
    if opacity > 0.0 && ui.is_enabled() {
        ui.painter()
            .rect_filled(response.rect, radius, content.gamma_multiply(opacity));
    }
}

/// Paint the focus ring around a component with keyboard focus.
///
/// # Arguments
///
/// * `ui` - the ui to paint in.
/// * `response` - the component's response; nothing is drawn unless it has
///   focus.
/// * `radius` - the component's corner radius; the ring follows it.
/// * `palette` - the colours; the ring is `secondary`.
pub fn paint_focus_ring(
    ui: &egui::Ui,
    response: &Response,
    radius: CornerRadius,
    palette: &Palette,
) {
    if response.has_focus() {
        let grow = FOCUS_RING_GAP + FOCUS_RING_WIDTH / 2.0;
        let ring_radius = radius + CornerRadius::same(grow as u8);
        ui.painter().rect_stroke(
            response.rect.expand(grow),
            ring_radius,
            Stroke::new(FOCUS_RING_WIDTH, rgb(palette.m3.secondary)),
            StrokeKind::Middle,
        );
    }
}

/// The shadow under a surface at `level` (0 to 3) of Material elevation.
///
/// # Returns
///
/// No shadow at level 0; otherwise a soft shadow in the `shadow` role whose
/// offset and blur grow with the level.
#[must_use]
pub fn elevation_shadow(level: u8, palette: &Palette) -> egui::Shadow {
    let (offset, blur, alpha) = match level {
        0 => return egui::Shadow::NONE,
        1 => (1, 3, 0.20),
        2 => (2, 6, 0.22),
        _ => (4, 10, 0.25),
    };
    egui::Shadow {
        offset: [0, offset],
        blur,
        spread: 0,
        color: rgb(palette.m3.shadow).gamma_multiply(alpha),
    }
}

/// The colour role of a component's less-prominent text on its own fill.
///
/// `on_surface_variant` is the Material role for supporting text, but it is
/// only checked against the surfaces ([`crate::material`]'s components also
/// paint it on containers): on a selected component's `secondary_container`
/// fill it falls below 4.5:1 in several themes. A selected component
/// therefore takes its headline colour, which is checked against that fill.
///
/// # Arguments
///
/// * `m3` - the theme's Material roles.
/// * `selected` - whether the component is drawn on `secondary_container`.
#[must_use]
pub fn supporting_role(m3: &M3Roles, selected: bool) -> Rgb {
    if selected {
        m3.on_secondary_container
    } else {
        m3.on_surface_variant
    }
}

/// A content colour as drawn in `ui`: unchanged when enabled, at
/// [`DISABLED_CONTENT_OPACITY`] of `on_surface` when disabled.
#[must_use]
pub fn content_colour(ui: &egui::Ui, palette: &Palette, enabled: Color32) -> Color32 {
    if ui.is_enabled() {
        enabled
    } else {
        rgb(palette.m3.on_surface).gamma_multiply(DISABLED_CONTENT_OPACITY)
    }
}

/// `icon` laid out at `size` in `colour`, ready to paint.
pub(crate) fn icon_galley(
    ui: &egui::Ui,
    icon: Icon,
    size: f32,
    colour: Color32,
) -> std::sync::Arc<egui::Galley> {
    egui::WidgetText::from(icon_label(ui.ctx(), icon, size, colour)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    )
}

/// `text` laid out in `font` and `colour`, on one line.
pub(crate) fn text_galley(
    ui: &egui::Ui,
    text: &str,
    font: egui::FontId,
    colour: Color32,
) -> std::sync::Arc<egui::Galley> {
    egui::WidgetText::from(egui::RichText::new(text).font(font).color(colour)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    )
}

/// `text` laid out in `font` and `colour` on one line, cut with an ellipsis
/// to `max_width` points; the galley's `elided` flag says whether it was.
pub(crate) fn elided_galley(
    ui: &egui::Ui,
    text: &str,
    font: egui::FontId,
    colour: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    egui::WidgetText::from(egui::RichText::new(text).font(font).color(colour)).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        max_width.max(0.0),
        egui::TextStyle::Body,
    )
}

/// The M3 "label large" font: 14 pt medium, used on buttons, tabs and chips.
///
/// # Arguments
///
/// * `ctx` - the window's context, for its installed fonts.
///
/// # Returns
///
/// The bold UI family ([`crate::fonts::BOLD_FAMILY`]) when the window's
/// font stack is installed, else egui's proportional family, so a context
/// without the stack (a test harness) still draws the label.
#[must_use]
pub fn label_large(ctx: &egui::Context) -> egui::FontId {
    let size = crate::metrics::text_size::BODY;
    let bold = egui::FontFamily::Name(crate::fonts::BOLD_FAMILY.into());
    let installed = ctx.fonts_mut(|fonts| fonts.definitions().families.contains_key(&bold));
    if installed {
        crate::fonts::bold_font(size, false)
    } else {
        egui::FontId::proportional(size)
    }
}

/// The Material "standard" motion duration, in seconds.
pub const MOTION_DURATION: f32 = 0.2;

/// How long a component's motion takes in `ctx`.
///
/// # Returns
///
/// [`MOTION_DURATION`], or 0.0 (no motion) when the app's style turns
/// egui's animations off (`animation_time` of zero).
#[must_use]
pub fn motion_time(ctx: &egui::Context) -> f32 {
    if ctx.global_style().animation_time > 0.0 {
        MOTION_DURATION
    } else {
        0.0
    }
}

/// The Material standard easing, decelerating: fast out, slow in.
#[must_use]
pub fn standard_easing(t: f32) -> f32 {
    egui::emath::easing::cubic_out(t)
}

/// The opacity of something that started appearing `elapsed` seconds ago,
/// fading in over `duration` seconds with [`standard_easing`].
///
/// # Returns
///
/// The opacity while the fade runs, or `None` once it is over (or when
/// `duration` is zero: no motion).
#[must_use]
pub fn fade_in_opacity(elapsed: f64, duration: f32) -> Option<f32> {
    if duration <= 0.0 {
        return None;
    }
    let progress = (elapsed as f32 / duration).max(0.0);
    (progress < 1.0).then(|| standard_easing(progress))
}
