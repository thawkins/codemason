//! Transcript display preferences: how much detail the transcript shows
//! ([`Verbosity`]) and how thinking text is presented ([`ThinkingDisplay`]).
//!
//! Depends on `codemason-config` for the startup keys. Holds state only; the
//! frontends read it when rendering.

use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

/// How much detail the transcript shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Quiet transcript (the default).
    #[default]
    Low,
    /// Moderate detail.
    Medium,
    /// Full detail.
    High,
}

/// How thinking text is presented in the transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingDisplay {
    /// Shown folded, expandable on demand (the default).
    #[default]
    Collapsed,
    /// Always shown in full.
    Expanded,
    /// Not shown.
    Hidden,
}

/// The transcript display preferences held on `App`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TranscriptPrefs {
    /// Transcript detail level.
    pub verbosity: Verbosity,
    /// Thinking text presentation.
    pub thinking: ThinkingDisplay,
}

impl TranscriptPrefs {
    /// Builds the startup preferences from the `verbosity` and
    /// `thinking_display` config keys; unset or unrecognised values fall back
    /// to the defaults.
    #[must_use]
    pub fn from_config(config: &codemason_config::Config) -> Self {
        Self {
            verbosity: config
                .verbosity
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or_default(),
            thinking: config
                .thinking_display
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or_default(),
        }
    }
}

impl FromStr for Verbosity {
    type Err = String;

    /// Parses `low`, `medium` or `high` (case-insensitive).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            other => Err(format!(
                "unknown verbosity `{other}`, use low, medium or high"
            )),
        }
    }
}

impl fmt::Display for Verbosity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        })
    }
}

impl FromStr for ThinkingDisplay {
    type Err = String;

    /// Parses `collapsed`, `expanded` or `hidden` (case-insensitive).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "collapsed" => Ok(Self::Collapsed),
            "expanded" => Ok(Self::Expanded),
            "hidden" => Ok(Self::Hidden),
            other => Err(format!(
                "unknown thinking display `{other}`, use collapsed, expanded or hidden"
            )),
        }
    }
}

impl fmt::Display for ThinkingDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Collapsed => "collapsed",
            Self::Expanded => "expanded",
            Self::Hidden => "hidden",
        })
    }
}

/// The preferences the launch configuration asks for, set once by the binary.
static STARTUP: OnceLock<TranscriptPrefs> = OnceLock::new();

/// Records the launch configuration's preferences, so a new
/// [`App`](crate::app::App) starts with them.
///
/// The binary calls this with the configuration it actually loaded (the
/// `--config` file when one was given). The first call wins.
pub fn set_startup(prefs: TranscriptPrefs) {
    // INTENTIONAL: a second call keeps the first preferences.
    let _ = STARTUP.set(prefs);
}

/// The preferences a new App starts with: the ones given to [`set_startup`],
/// else the defaults.
#[must_use]
pub fn startup() -> TranscriptPrefs {
    STARTUP.get().copied().unwrap_or_default()
}
