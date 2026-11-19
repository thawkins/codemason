//! Running `/editor` (action `prompt.editor`) for the window.
//!
//! The editor runs on a worker thread so the window keeps drawing; app-core
//! locks the prompt meanwhile (`App::begin_external_edit`) and takes the
//! result back (`App::finish_external_edit`), which [`ExternalEdit::poll`]
//! hands over once the editor exits.
//!
//! Dependencies: `codemason-app-core` (`external_editor`) and `egui`.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use codemason_app_core::app::App;
use codemason_app_core::external_editor::{edit_in_external_editor, editor_from_env};

/// How often the window checks for the editor's result while it runs.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The window's external-editor runs.
#[derive(Debug, Default)]
pub struct ExternalEdit {
    /// The editor command, instead of `$VISUAL` / `$EDITOR`.
    editor: Option<String>,
    /// Where the prompt file is written, instead of the system temp dir.
    scratch_dir: Option<PathBuf>,
    /// The running editor's result, when one runs.
    job: Option<Receiver<anyhow::Result<String>>>,
}

impl ExternalEdit {
    /// Use `editor` and `scratch_dir` instead of the environment's editor and
    /// the system temp directory.
    #[must_use]
    pub fn with_editor(editor: String, scratch_dir: PathBuf) -> Self {
        Self {
            editor: Some(editor),
            scratch_dir: Some(scratch_dir),
            job: None,
        }
    }

    /// Open the prompt in the editor. Without an editor the status line
    /// says to set one; while one already runs nothing happens.
    pub fn start(&mut self, ctx: &egui::Context, app: &mut App) {
        let Some(editor) = self.editor.clone().or_else(editor_from_env) else {
            app.status = "editor: set $VISUAL or $EDITOR to use /editor".to_string();
            app.needs_redraw = true;
            return;
        };
        let Some(text) = app.begin_external_edit() else {
            return;
        };
        let scratch = self.scratch_dir.clone().unwrap_or_else(std::env::temp_dir);
        let (tx, rx) = std::sync::mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let result = edit_in_external_editor(&text, &editor, &scratch);
            // INTENTIONAL: the window may have closed; nothing waits for the result then.
            let _ = tx.send(result);
            repaint.request_repaint();
        });
        self.job = Some(rx);
        ctx.request_repaint_after(POLL_INTERVAL);
    }

    /// Hand a finished editor's result to the App; keep polling while it
    /// runs.
    pub fn poll(&mut self, ctx: &egui::Context, app: &mut App) {
        let Some(job) = self.job.as_ref() else {
            return;
        };
        match job.try_recv() {
            Ok(result) => {
                self.job = None;
                app.finish_external_edit(result);
                ctx.request_repaint();
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(POLL_INTERVAL),
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                app.finish_external_edit(Err(anyhow::anyhow!("the editor thread stopped")));
            }
        }
    }
}
