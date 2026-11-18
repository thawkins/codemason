//! Formatter-after-edit configuration.
//!
//! These types define the `formatter` section of codemason's configuration
//! file. When enabled, a code formatter chosen by file extension runs on each
//! file a file-writing tool has just written.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Top-level formatter configuration.
///
/// Corresponds to the `formatter` key in `codemason.json`. The feature
/// defaults to **off**.
///
/// ```json
/// {
///   "formatter": {
///     "enabled": true,
///     "formatters": {
///       "prettier": { "disabled": true },
///       "shfmt": { "command": ["shfmt", "-w", "$FILE"], "extensions": [".sh"] }
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatterConfig {
    /// Whether formatters run after file-writing tools.
    #[serde(default)]
    pub enabled: bool,
    /// Per-formatter settings, keyed by formatter name. A key that names a
    /// built-in formatter adjusts it; any other key defines a custom one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub formatters: BTreeMap<String, FormatterEntry>,
}

impl FormatterConfig {
    /// `true` when the section is at its default state, so serialisation can
    /// omit the `formatter` key.
    #[must_use]
    pub fn is_default(&self) -> bool {
        !self.enabled && self.formatters.is_empty()
    }
}

/// Settings for one formatter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatterEntry {
    /// Turns this formatter off.
    #[serde(default)]
    pub disabled: bool,
    /// The program and its arguments; `$FILE` is replaced by the file path.
    /// Empty keeps a built-in formatter's own command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    /// File extensions the formatter handles, with the leading dot
    /// (`".rs"`). Empty keeps a built-in formatter's own list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
}
