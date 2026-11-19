//! Spacing, corner-radius and type scales.
//!
//! Every gap, corner and font size in the window comes from one of these
//! scales, so the layout keeps one rhythm: spacing in multiples of 4 points,
//! five corner radii and four text sizes. The radii and text sizes follow
//! the Material Design 3 shape and type scales.
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
    /// Menus, tooltips and snackbars (M3 "extra small").
    pub const XS: u8 = 4;
    /// Buttons, fields and other framed widgets (M3 "small").
    pub const SM: u8 = 8;
    /// Cards and chips (M3 "medium").
    pub const MD: u8 = 12;
    /// Dialogs and popups (M3 "large").
    pub const LG: u8 = 16;
    /// Material dialogs (M3 "extra large").
    pub const XL: u8 = 28;
}

/// The type scale, in points.
pub mod text_size {
    /// Captions and gutters (M3 Label Small).
    pub const SMALL: f32 = 11.0;
    /// Body text (M3 Body Medium).
    pub const BODY: f32 = 14.0;
    /// Subheadings (M3 Title Medium).
    pub const LARGE: f32 = 16.0;
    /// Headings (M3 Title Large).
    pub const TITLE: f32 = 22.0;
}
