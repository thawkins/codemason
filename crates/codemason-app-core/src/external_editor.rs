//! Editing the prompt in an external editor (`/editor`, leader `e`).
//!
//! [`edit_in_external_editor`] writes the prompt to a file in a scratch
//! directory, runs the user's editor on it and returns what was saved.
//! [`editor_from_env`] picks the editor: `$VISUAL`, then `$EDITOR`. The
//! editor text is run by the shell (`sh -c` on Unix, `cmd /C` on Windows) so
//! it may carry arguments, e.g. `code --wait`.
//!
//! The App side ([`App::begin_external_edit`] / [`App::finish_external_edit`])
//! locks the prompt while the editor runs and puts the result back; the
//! frontend runs the editor off its UI thread.
//!
//! Depends on `tempfile` and `std::process`.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::app::App;

/// The editor to run: `$VISUAL`, then `$EDITOR`; `None` when neither is set
/// to a non-blank value.
#[must_use]
pub fn editor_from_env() -> Option<String> {
    ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
}

/// Edit `text` in `editor` and return the saved contents.
///
/// # Arguments
///
/// * `text` - the starting contents.
/// * `editor` - the editor command, possibly with arguments; the file path is
///   appended as its last argument.
/// * `scratch_dir` - where the temporary file is created (and removed).
///
/// # Returns
///
/// The file's contents after the editor exits, without one trailing newline
/// (editors add one).
///
/// # Errors
///
/// When the file cannot be written or read back, the editor cannot start, or
/// it exits unsuccessfully.
pub fn edit_in_external_editor(text: &str, editor: &str, scratch_dir: &Path) -> Result<String> {
    std::fs::create_dir_all(scratch_dir)
        .with_context(|| format!("cannot create {}", scratch_dir.display()))?;
    let file = tempfile::Builder::new()
        .prefix("codemason-prompt-")
        .suffix(".md")
        .tempfile_in(scratch_dir)
        .context("cannot create the prompt file")?;
    std::fs::write(file.path(), text).context("cannot write the prompt file")?;
    let status = editor_command(editor, file.path())
        .status()
        .with_context(|| format!("cannot run editor '{editor}'"))?;
    if !status.success() {
        bail!("editor '{editor}' exited with {status}");
    }
    let mut edited =
        std::fs::read_to_string(file.path()).context("cannot read the edited prompt")?;
    if edited.ends_with('\n') {
        edited.pop();
        if edited.ends_with('\r') {
            edited.pop();
        }
    }
    Ok(edited)
}

/// The shell command that runs `editor` on `path`. The path is passed as a
/// positional argument, never spliced into the command text.
#[cfg(unix)]
fn editor_command(editor: &str, path: &Path) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path);
    command
}

/// The shell command that runs `editor` on `path`.
#[cfg(windows)]
fn editor_command(editor: &str, path: &Path) -> Command {
    let mut command = Command::new("cmd");
    command
        .arg("/C")
        .arg(format!("{editor} \"{}\"", path.display()));
    command
}

impl App {
    /// Start editing the prompt externally: lock the prompt and return its
    /// text for the editor.
    ///
    /// # Returns
    ///
    /// The prompt text, or `None` when an external edit is already running.
    pub fn begin_external_edit(&mut self) -> Option<String> {
        if self.external_editor_open {
            return None;
        }
        self.external_editor_open = true;
        self.status = "editor: waiting for the external editor".to_string();
        self.needs_redraw = true;
        Some(self.session.input.clone())
    }

    /// End an external edit: unlock the prompt and, on success, replace its
    /// text with the edited text (cursor at the end); on failure, keep the
    /// prompt and report the error in the status line.
    ///
    /// # Arguments
    ///
    /// * `result` - what [`edit_in_external_editor`] returned.
    pub fn finish_external_edit(&mut self, result: Result<String>) {
        self.external_editor_open = false;
        match result {
            Ok(text) => {
                self.session.input = text;
                self.set_cursor_char_index_clamped(usize::MAX);
                self.refresh_input_menus();
                self.status = "editor: prompt updated".to_string();
            }
            Err(e) => {
                self.status = format!("editor: {e:#}");
            }
        }
        self.needs_redraw = true;
    }
}
