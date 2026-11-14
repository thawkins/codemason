//! Frontend-neutral rectangle type in character cells.
//!
//! No dependencies.

/// A rectangular area in cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Area {
    /// Left edge column.
    pub x: u16,
    /// Top edge row.
    pub y: u16,
    /// Width in cells.
    pub width: u16,
    /// Height in cells.
    pub height: u16,
}
