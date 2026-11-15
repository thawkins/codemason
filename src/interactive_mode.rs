//! Which frontend an interactive `cdmn` run (no sub-command, or
//! `session resume`) opens.
//!
//! The desktop GUI is the only interactive UI, so it is the default;
//! `--no-tui` keeps the plain stdin/stdout mode and `--gui` is still accepted
//! for existing scripts but changes nothing. The flags live in
//! [`InteractiveArgs`], flattened into the binary's top-level clap parser, so
//! a test can parse the same arguments without the rest of the binary.
//!
//! Depends on `clap`.

use clap::Args;

/// The interactive frontend a run opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The desktop GUI window (the default).
    Gui,
    /// Plain line-by-line stdin/stdout, with no UI.
    Plain,
}

/// The top-level flags that choose the interactive frontend.
#[derive(Debug, Clone, Copy, Args)]
pub struct InteractiveArgs {
    /// Disable the GUI, use plain stdin/stdout
    #[arg(long, global = true)]
    pub no_tui: bool,

    /// Open the desktop GUI (the default; accepted for compatibility)
    #[arg(long, global = true, conflicts_with = "no_tui")]
    pub gui: bool,
}

impl InteractiveArgs {
    /// The frontend these flags select.
    ///
    /// # Returns
    ///
    /// [`Mode::Plain`] for `--no-tui`, otherwise [`Mode::Gui`] (with or
    /// without `--gui`).
    #[must_use]
    pub fn mode(&self) -> Mode {
        if self.no_tui { Mode::Plain } else { Mode::Gui }
    }
}
