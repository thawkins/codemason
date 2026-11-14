//! Theme mode selection (default vs high contrast).
//!
//! Frontend-neutral (no ratatui); each frontend maps the mode to its palette.
//!
//! No dependencies.

/// Theme mode enum for switching between default and high contrast themes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Default theme with standard colors
    #[default]
    Default,
    /// High contrast theme for accessibility (WCAG AAA)
    HighContrast,
}

impl ThemeMode {
    /// Parse a theme mode from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "default" => Some(Self::Default),
            "high-contrast" | "high_contrast" | "highcontrast" => Some(Self::HighContrast),
            _ => None,
        }
    }

    /// Get the display name for the theme mode
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::HighContrast => "high-contrast",
        }
    }
}
