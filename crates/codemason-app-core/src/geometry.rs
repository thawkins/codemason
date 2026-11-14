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

impl Area {
    /// Build an area, clamping `width`/`height` so the right and bottom edges
    /// stay within `u16` (the same rule as the terminal renderer's rectangle constructor).
    ///
    /// # Arguments
    /// * `x`, `y` - top-left cell.
    /// * `width`, `height` - size in cells (clamped).
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width: x.saturating_add(width) - x,
            height: y.saturating_add(height) - y,
        }
    }

    /// Number of cells covered (`width * height`).
    pub const fn area(self) -> u32 {
        (self.width as u32) * (self.height as u32)
    }

    /// Whether the area covers no cells.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Left edge column (`x`).
    pub const fn left(self) -> u16 {
        self.x
    }

    /// First column right of the area (saturating at `u16::MAX`).
    pub const fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// Top edge row (`y`).
    pub const fn top(self) -> u16 {
        self.y
    }

    /// First row below the area (saturating at `u16::MAX`).
    pub const fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    /// Whether the cell at `(column, row)` lies inside the area.
    ///
    /// # Arguments
    /// * `pos` - the `(column, row)` cell to test.
    pub const fn contains(self, pos: (u16, u16)) -> bool {
        pos.0 >= self.x && pos.0 < self.right() && pos.1 >= self.y && pos.1 < self.bottom()
    }
}
