//! Material Design 3 cards: elevated, filled and outlined.
//!
//! [`card_frame`] is the frame a card's content is drawn in: medium (12
//! point) corners and 16 points of padding, with the fill, border and
//! shadow of its kind.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{CornerRadius, Frame, Margin, Stroke};

use super::elevation_shadow;
use crate::metrics::radius;
use crate::palette::{Palette, rgb};

/// Padding inside a card, in points.
const PADDING: i8 = 16;

/// The style of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// `surface_container_low` with the elevation-1 shadow.
    Elevated,
    /// `surface_container_highest`, no shadow.
    Filled,
    /// `surface` with an `outline_variant` border.
    Outlined,
}

/// The frame of a card.
///
/// # Arguments
///
/// * `kind` - the card's style.
/// * `palette` - the colours.
///
/// # Returns
///
/// A frame with medium corners, 16 points of padding, and the fill, border
/// and shadow of `kind`.
#[must_use]
pub fn card_frame(kind: CardKind, palette: &Palette) -> Frame {
    let m3 = &palette.m3;
    let frame = Frame::new()
        .corner_radius(CornerRadius::same(radius::MD))
        .inner_margin(Margin::same(PADDING));
    match kind {
        CardKind::Elevated => frame
            .fill(rgb(m3.surface_container_low))
            .shadow(elevation_shadow(1, palette)),
        CardKind::Filled => frame.fill(rgb(m3.surface_container_highest)),
        CardKind::Outlined => frame
            .fill(rgb(m3.surface))
            .stroke(Stroke::new(1.0, rgb(m3.outline_variant))),
    }
}
