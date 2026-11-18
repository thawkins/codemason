//! Spacing, corner-radius and type scales.
//!
//! Every gap, corner and font size in the window comes from one of these
//! scales, so the layout keeps one rhythm: spacing in multiples of 4 points,
//! three corner radii and four text sizes.
//!
//! No dependencies.

/// The spacing scale, in points.
pub mod space {
    /// Between tightly related items, e.g. a label and its value.
    pub const XS: f32 = 4.0;
    /// Between rows of one group.
    pub const SM: f32 = 8.0;
    /// Between groups.
    pub const MD: f32 = 12.0;
    /// Between sections.
    pub const LG: f32 = 16.0;
    /// Around a dialog's main content or a hero block.
    pub const XL: f32 = 24.0;
}

/// The corner-radius scale, in points.
pub mod radius {
    /// Buttons, fields and other framed widgets.
    pub const SM: u8 = 4;
    /// Cards and chips.
    pub const MD: u8 = 6;
    /// Dialogs and popups.
    pub const LG: u8 = 10;
}

/// The type scale, in points.
pub mod text_size {
    /// Captions and gutters.
    pub const SMALL: f32 = 12.0;
    /// Body text.
    pub const BODY: f32 = 14.0;
    /// Subheadings.
    pub const LARGE: f32 = 16.0;
    /// Headings.
    pub const TITLE: f32 = 20.0;
}
