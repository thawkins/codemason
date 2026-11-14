//! GUI colours for the frontend-neutral [`ThemeColor`]s.
//!
//! [`palette_for`] builds the [`Palette`] of a [`ThemeMode`] from the same
//! values the terminal theme uses (`codemason-tui`'s `theme::colors`,
//! `theme::status` and `theme::high_contrast`). ANSI colours, which a
//! terminal takes from its own palette, use the standard xterm values, and
//! [`ThemeColor::Indexed`] maps through the standard xterm-256 table.
//!
//! Dependencies: `egui` ([`Color32`]) and `codemason-app-core`
//! ([`ThemeColor`], [`ThemeMode`]).

use codemason_app_core::styled::ThemeColor;
use codemason_app_core::theme_mode::ThemeMode;
use egui::Color32;

/// The standard xterm values of the 16 ANSI colours, in index order
/// (black, red, green, yellow, blue, magenta, cyan, gray, then the bright
/// variants dark gray .. white).
const XTERM_ANSI: [Color32; 16] = [
    Color32::from_rgb(0, 0, 0),
    Color32::from_rgb(205, 0, 0),
    Color32::from_rgb(0, 205, 0),
    Color32::from_rgb(205, 205, 0),
    Color32::from_rgb(0, 0, 238),
    Color32::from_rgb(205, 0, 205),
    Color32::from_rgb(0, 205, 205),
    Color32::from_rgb(229, 229, 229),
    Color32::from_rgb(127, 127, 127),
    Color32::from_rgb(255, 0, 0),
    Color32::from_rgb(0, 255, 0),
    Color32::from_rgb(255, 255, 0),
    Color32::from_rgb(92, 92, 255),
    Color32::from_rgb(255, 0, 255),
    Color32::from_rgb(0, 255, 255),
    Color32::from_rgb(255, 255, 255),
];

/// The default mode's accent: xterm-256 colour 75 (`#5fafff`), 9.1:1 on
/// black (WCAG AAA). The terminal theme's accent is ANSI blue, 2.2:1 on
/// black, which the TUI only gets away with because it draws the accent as
/// uncoloured bold.
const DEFAULT_ACCENT: Color32 = Color32::from_rgb(95, 175, 255);

/// Channel levels of the xterm-256 6x6x6 colour cube (indices 16..232).
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// The resolved colours of one theme mode.
///
/// Semantic colours are fields; literal colours resolve through
/// [`Palette::color`] (ANSI from [`Palette::ansi`], `Indexed` through the
/// xterm-256 table, `Rgb` as given).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// [`ThemeColor::Accent`]: the theme's primary colour.
    pub accent: Color32,
    /// [`ThemeColor::Text`] and [`ThemeColor::Reset`]: default text.
    pub text: Color32,
    /// [`ThemeColor::Muted`]: secondary text.
    pub muted: Color32,
    /// [`ThemeColor::Error`].
    pub error: Color32,
    /// [`ThemeColor::Warning`].
    pub warning: Color32,
    /// [`ThemeColor::Success`].
    pub success: Color32,
    /// [`ThemeColor::Info`].
    pub info: Color32,
    /// [`ThemeColor::Border`]: inactive borders.
    pub border: Color32,
    /// The panel background the text is drawn on.
    pub background: Color32,
    /// The 16 ANSI colours, in xterm index order.
    pub ansi: [Color32; 16],
}

impl Palette {
    /// Resolve `color` to an egui colour.
    ///
    /// # Arguments
    ///
    /// * `color` - the semantic or literal colour to resolve.
    ///
    /// # Returns
    ///
    /// The colour this palette draws `color` with.
    #[must_use]
    pub fn color(&self, color: ThemeColor) -> Color32 {
        match color {
            ThemeColor::Accent => self.accent,
            ThemeColor::Text | ThemeColor::Reset => self.text,
            ThemeColor::Muted => self.muted,
            ThemeColor::Error => self.error,
            ThemeColor::Warning => self.warning,
            ThemeColor::Success => self.success,
            ThemeColor::Info => self.info,
            ThemeColor::Border => self.border,
            ThemeColor::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
            ThemeColor::Black => self.ansi[0],
            ThemeColor::Red => self.ansi[1],
            ThemeColor::Green => self.ansi[2],
            ThemeColor::Yellow => self.ansi[3],
            ThemeColor::Blue => self.ansi[4],
            ThemeColor::Magenta => self.ansi[5],
            ThemeColor::Cyan => self.ansi[6],
            ThemeColor::Gray => self.ansi[7],
            ThemeColor::DarkGray => self.ansi[8],
            ThemeColor::LightRed => self.ansi[9],
            ThemeColor::LightGreen => self.ansi[10],
            ThemeColor::LightYellow => self.ansi[11],
            ThemeColor::LightBlue => self.ansi[12],
            ThemeColor::LightMagenta => self.ansi[13],
            ThemeColor::LightCyan => self.ansi[14],
            ThemeColor::White => self.ansi[15],
            ThemeColor::Indexed(index) => self.indexed(index),
        }
    }

    /// Resolve an xterm-256 palette `index`.
    ///
    /// 0..16 are [`Palette::ansi`], 16..232 the 6x6x6 colour cube and
    /// 232..256 the 24-step grey ramp (8, 18, ..., 238).
    fn indexed(&self, index: u8) -> Color32 {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let i = usize::from(index - 16);
                Color32::from_rgb(
                    CUBE_LEVELS[i / 36],
                    CUBE_LEVELS[(i / 6) % 6],
                    CUBE_LEVELS[i % 6],
                )
            }
            232..=255 => {
                let v = 8 + 10 * (index - 232);
                Color32::from_rgb(v, v, v)
            }
        }
    }
}

/// Build the palette of `mode`.
///
/// # Arguments
///
/// * `mode` - the theme mode the app is in.
///
/// # Returns
///
/// The default palette mirrors the terminal theme's `colors`/`status`
/// constants, except the accent: `colors::PRIMARY` is ANSI blue, unreadable
/// on black, so the GUI uses `DEFAULT_ACCENT` (light blue). The high-contrast
/// palette mirrors `high_contrast` (accent `Rgb(0, 150, 255)`, error
/// `Rgb(255, 50, 50)`, ...). Both draw on black with the xterm ANSI colours.
#[must_use]
pub fn palette_for(mode: ThemeMode) -> Palette {
    let ansi = XTERM_ANSI;
    match mode {
        ThemeMode::Default => Palette {
            accent: DEFAULT_ACCENT,
            text: ansi[15],
            muted: Color32::from_rgb(170, 170, 170),
            error: ansi[1],
            warning: ansi[3],
            success: ansi[2],
            info: ansi[6],
            border: Color32::from_rgb(100, 100, 100),
            background: ansi[0],
            ansi,
        },
        ThemeMode::HighContrast => Palette {
            accent: Color32::from_rgb(0, 150, 255),
            text: ansi[15],
            muted: Color32::from_rgb(128, 128, 128),
            error: Color32::from_rgb(255, 50, 50),
            warning: Color32::from_rgb(255, 255, 0),
            success: Color32::from_rgb(50, 255, 50),
            info: ansi[6],
            border: Color32::from_rgb(128, 128, 128),
            background: ansi[0],
            ansi,
        },
    }
}
