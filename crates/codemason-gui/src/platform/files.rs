//! Native file dialogs for the window, built on craft-dialogs.
//!
//! The GUI asks for a path in the three places the terminal frontend has
//! the user type one:
//!
//! | [`FileIntent`] | Dialog | Typed TUI path it stands for |
//! |---|---|---|
//! | [`FileIntent::Attach`] | Open, several images | an image path staged for the next message (`Alt+V` with a path on the clipboard) |
//! | [`FileIntent::OpenProject`] | Folder | `/project add <path>` |
//! | [`FileIntent::NewProjectDir`] | Folder | `/session new <path>`, the directory `/new` then scaffolds into |
//!
//! [`apply_answer`] hands a chosen path to the same app-core action the
//! typed path reaches (`App::attach_image_path`, or the slash command run
//! through `App::execute_slash_command`), so the two frontends cannot
//! drift. A cancelled dialog changes nothing; one that could not be shown
//! is logged as a warning.
//!
//! [`FileDialogs`] never blocks the event loop: [`FileDialogs::ask`] queues
//! a request, [`FileDialogs::show_pending`] shows it at the end of a frame
//! with the window as parent, and the answer is taken on a later frame
//! ([`FileDialogs::take_answers`]). The dialog wakes the window when it
//! answers, through the context bound with [`FileDialogs::bind`].
//!
//! Dependencies: `craft-dialogs` (queued native dialogs over `rfd`),
//! `egui` (the File menu and the repaint wake), `eframe` (the parent
//! window), `codemason-app-core` (the
//! actions the answers run) and `tracing`.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use codemason_app_core::app::{App, LogLevel};
use craft_dialogs::{Answer, Dialogs, FileDialog, Override};

/// The extensions the attach dialog offers: the ones app-core accepts as
/// images (`is_image_path`).
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif"];

/// The File menu's label.
pub const FILE_MENU_LABEL: &str = "File";
/// The File menu item that attaches images.
pub const ATTACH_LABEL: &str = "Attach Images...";
/// The File menu item that registers a project folder.
pub const OPEN_PROJECT_LABEL: &str = "Open Project...";
/// The File menu item that starts a session in a new project's folder.
pub const NEW_PROJECT_DIR_LABEL: &str = "New Project Folder...";

/// What a file dialog's answer is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileIntent {
    /// Stage the chosen images as attachments for the next message.
    Attach,
    /// Register the chosen folder as a project (`/project add`).
    OpenProject,
    /// Start a session in the chosen folder (`/session new`), where `/new`
    /// scaffolds the new project.
    NewProjectDir,
}

impl FileIntent {
    /// The native dialog this intent shows.
    ///
    /// # Arguments
    ///
    /// * `start` - the folder the dialog opens in, when there is one.
    ///
    /// # Returns
    ///
    /// The craft-dialogs request: several images for [`Self::Attach`], one
    /// folder for the others.
    #[must_use]
    pub fn dialog(self, start: Option<&Path>) -> FileDialog {
        let dialog = match self {
            Self::Attach => FileDialog::open_many()
                .title("Attach images")
                .filter("Images", IMAGE_EXTENSIONS),
            Self::OpenProject => FileDialog::folder().title("Open project"),
            Self::NewProjectDir => {
                FileDialog::folder().title("Choose the folder for the new project")
            }
        };
        match start {
            Some(dir) => dialog.directory(dir),
            None => dialog,
        }
    }
}

/// The window's file dialogs: the queued requests, the one on screen and
/// the answers not yet taken, plus the context their answers wake.
pub struct FileDialogs {
    dialogs: Dialogs<FileIntent>,
    ctx: Arc<OnceLock<egui::Context>>,
}

impl std::fmt::Debug for FileDialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileDialogs")
            .field("open", &self.dialogs.is_open())
            .field("queued", &self.dialogs.queued())
            .finish_non_exhaustive()
    }
}

impl Default for FileDialogs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileDialogs {
    /// Create the dialogs, not yet bound to a window.
    ///
    /// # Returns
    ///
    /// Dialogs whose answers wake the context given to [`Self::bind`]
    /// (`ctx.request_repaint()`); before that, nothing is woken.
    #[must_use]
    pub fn new() -> Self {
        let ctx: Arc<OnceLock<egui::Context>> = Arc::new(OnceLock::new());
        let wake_ctx = Arc::clone(&ctx);
        let dialogs = Dialogs::new(Arc::new(move || {
            if let Some(ctx) = wake_ctx.get() {
                ctx.request_repaint();
            }
        }));
        Self { dialogs, ctx }
    }

    /// Wake `ctx` when a dialog answers. Only the first context is kept.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's context.
    pub fn bind(&self, ctx: &egui::Context) {
        // A second call keeps the context already bound (one window).
        let _ = self.ctx.set(ctx.clone()); // INTENTIONAL: already bound to this window
    }

    /// Queue the dialog of `intent`; it shows at the next
    /// [`Self::show_pending`] once earlier ones have answered.
    ///
    /// # Arguments
    ///
    /// * `intent` - what the chosen path is for.
    /// * `start` - the folder the dialog opens in, when there is one.
    pub fn ask(&mut self, intent: FileIntent, start: Option<&Path>) {
        self.dialogs.ask(intent.dialog(start), intent);
    }

    /// Show the next queued dialog unless one is open. Called at the end of
    /// every frame on the main thread, with the window as `parent`.
    ///
    /// # Arguments
    ///
    /// * `parent` - the window the dialog belongs to, or `None` (a harness
    ///   with no native window).
    pub fn show_pending(&mut self, parent: Option<&eframe::Frame>) {
        self.dialogs.show_pending(parent);
    }

    /// The answers that came in since the last call, in the order asked.
    ///
    /// # Returns
    ///
    /// Each answer with the intent it was asked for.
    pub fn take_answers(&mut self) -> Vec<(FileIntent, Answer)> {
        self.dialogs.take_answers()
    }

    /// Whether a dialog is on screen or its answer is not yet taken.
    ///
    /// # Returns
    ///
    /// `true` while a dialog is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.dialogs.is_open()
    }

    /// Answer every request with `answer_with` instead of a native dialog
    /// (tests and automation); `None` shows native dialogs again.
    ///
    /// # Arguments
    ///
    /// * `answer_with` - the stand-in for the user's choice.
    pub fn set_override(&mut self, answer_with: Option<Override>) {
        self.dialogs.set_override(answer_with);
    }
}

/// Draw the File menu, whose items queue the dialogs.
///
/// # Arguments
///
/// * `ui` - the menu bar.
/// * `files` - the dialogs the items queue.
/// * `start` - the folder the dialogs open in (the focused session's).
/// * `enabled` - whether the items can be used (not while an app-core modal
///   owns the window).
pub fn file_menu(ui: &mut egui::Ui, files: &mut FileDialogs, start: &Path, enabled: bool) {
    let start = (!start.as_os_str().is_empty()).then_some(start);
    ui.add_enabled_ui(enabled, |ui| {
        ui.menu_button(FILE_MENU_LABEL, |ui| {
            for (label, intent) in [
                (ATTACH_LABEL, FileIntent::Attach),
                (OPEN_PROJECT_LABEL, FileIntent::OpenProject),
                (NEW_PROJECT_DIR_LABEL, FileIntent::NewProjectDir),
            ] {
                if ui.button(label).clicked() {
                    files.ask(intent, start);
                    ui.close();
                }
            }
        });
    });
}

/// Run what a dialog's answer asks for.
///
/// - [`Answer::Paths`]: [`FileIntent::Attach`] stages each image with
///   `App::attach_image_path`; [`FileIntent::OpenProject`] runs
///   `/project add <path>` and [`FileIntent::NewProjectDir`] runs
///   `/session new <path>` through `App::execute_slash_command`, exactly as
///   if the user had typed them.
/// - [`Answer::Cancelled`]: nothing changes.
/// - [`Answer::Unavailable`]: no dialog could be shown; logged as a warning.
///
/// # Arguments
///
/// * `app` - the application the answer acts on.
/// * `intent` - what the dialog was asked for.
/// * `answer` - how it ended.
pub async fn apply_answer(app: &mut App, intent: FileIntent, answer: Answer) {
    let paths = match answer {
        Answer::Paths(paths) => paths,
        // A file dialog never answers with a message-box button.
        Answer::Cancelled | Answer::Message(_) => return,
        Answer::Unavailable(why) => {
            app.push_log(
                LogLevel::Warn,
                format!("[files] the file dialog could not be shown: {why}"),
                None,
            );
            return;
        }
    };
    let command = match intent {
        FileIntent::Attach => {
            for path in &paths {
                app.attach_image_path(path);
            }
            return;
        }
        FileIntent::OpenProject => "/project add",
        FileIntent::NewProjectDir => "/session new",
    };
    let Some(path) = paths.first() else {
        return;
    };
    match path.to_str() {
        Some(path) => {
            app.execute_slash_command(&format!("{command} {path}"))
                .await
        }
        None => app.push_log(
            LogLevel::Warn,
            format!(
                "[files] {} is not valid UTF-8 and cannot be used",
                path.display()
            ),
            None,
        ),
    }
}
