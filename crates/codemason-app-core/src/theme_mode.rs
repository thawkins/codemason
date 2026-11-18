//! Theme mode selection (default vs high contrast).
//!
//! Frontend-neutral (no terminal-UI types); each frontend maps the mode to its palette.
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

/// The theme the launch configuration asks for, set once by the binary.
static STARTUP: std::sync::OnceLock<ThemeMode> = std::sync::OnceLock::new();

/// Records the theme named by the launch configuration's `theme` key, so a
/// new [`App`](crate::app::App) starts in it.
///
/// The binary calls this with the configuration it actually loaded (the
/// `--config` file when one was given), which the layered config chain an App
/// could read for itself does not always match. The first call wins.
pub fn set_startup(mode: ThemeMode) {
    // INTENTIONAL: a second call keeps the first theme.
    let _ = STARTUP.set(mode);
}

/// The theme a new App starts in: the one given to [`set_startup`], else the
/// default.
#[must_use]
pub fn startup() -> ThemeMode {
    STARTUP.get().copied().unwrap_or_default()
}
