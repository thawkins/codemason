//! The eframe application that draws the codemason window.
//!
//! The shell renders the dock ([`crate::dock`], craft-ui's `DockArea`)
//! above the status bar ([`status_bar_ui`]), and over them the App's active
//! modal dialog ([`dialogs::show`]). Each dock pane maps to its panel: the
//! message transcript ([`MessagesView`]) under the running research and
//! deck progress views ([`research_ui`]), the input queue ([`queue_ui`])
//! above the prompt editor ([`prompt_ui`]), which edits the App's own input
//! buffer, the sessions sidebar ([`SessionsView`]), the log panel
//! ([`log_ui`]), the profiler table ([`profile_ui`]), the task list
//! ([`tasks_ui`]), the memories ([`memory_ui`]) and the Agents and Teams
//! window tables ([`agents_ui`], [`teams_ui`]). The queue pane, whose list
//! is drawn above the prompt, draws a labelled placeholder.
//!
//! # Dock visibility and persistence
//!
//! The App's panel flags are the single source of truth for which panes
//! are shown. Before drawing, each frame runs [`dock::sync_visibility`]
//! with [`dock::pane_wanted`], so `Alt+L`, `Alt+S` and every other path
//! that flips a flag (slash commands, app-core's own rules such as "the log
//! dismisses the other side panels") shows or hides the matching pane in
//! the same frame. A pane closed from the dock's chrome is hidden through
//! app-core ([`dock::close_in_app`]: app-core's own hide action, or the
//! pane's toggle chord sent through `App::handle_key` as the shortcut is)
//! instead of changing the layout, and the next sync hides it. A pane
//! app-core starts sending the keyboard to ([`dock::pane_takes_keys`]) is
//! brought to the front of its tab group, and the sessions sidebar is kept
//! in front for as long as it has the keyboard, so keys never go to a
//! covered pane. A pane toggled on in the focused sidebar's tab group
//! (`Alt+P`, `Alt+M`) takes the front instead, and the sidebar gives up the
//! keyboard (`sessions_sidebar_state::unfocus`, as a press outside it).
//! The other actions the dock returns (moves, resizes, tab switches,
//! floats) are applied to the layout. With a layout file
//! ([`Shell::with_layout_file`]) the layout and the hidden panes' places
//! are loaded at start ([`dock::load_dock`]), and every
//! change is saved once no pointer button is held (so a drag is saved once,
//! when it ends) and when the window is asked to close.
//! [`Shell::logic`], which eframe also runs while the window is hidden,
//! drains the agent-event queue and the tracing log channel into the `App`
//! and runs app-core's background housekeeping pass (`App::housekeeping`)
//! and the App's timers (`App::poll_timers`). Each shown frame then
//! prepares the prompt's input
//! ([`PromptInput::prepare`]) and the active dialog's ([`dialogs::prepare`]),
//! then routes the frame's key presses to `App::handle_key` through
//! [`KeyRouter`] before any widget reads them. After drawing it runs the key
//! presses the dialogs' buttons sent ([`dialogs::take_keys`]).

use std::collections::BTreeSet;
use std::path::PathBuf;

use codemason_agent::event::Event;
use codemason_app_core::app::{App, LogLevel, SelectionPane};
use codemason_app_core::input_event::KeyInput;
use codemason_app_core::log_channel::TuiLogReceiver;
use codemason_app_core::sessions_sidebar_state;
use craft_ui::docking::{Action, DockArea, DockStyle, Layout, PanelState};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::dialogs;
use crate::dock::{self, HiddenPanes, PaneId};
use crate::keys::{Focus, KeyRouter, default_keymap};
use crate::palette::{Palette, palette_for};
use crate::panels::agents::agents_ui;
use crate::panels::log::log_ui;
use crate::panels::memory::memory_ui;
use crate::panels::messages::MessagesView;
use crate::panels::profile::profile_ui;
use crate::panels::prompt::{PromptInput, prompt_ui};
use crate::panels::queue::queue_ui;
use crate::panels::research::research_ui;
use crate::panels::sessions::SessionsView;
use crate::panels::status_bar::status_bar_ui;
use crate::panels::tasks::tasks_ui;
use crate::panels::teams::teams_ui;
use crate::platform::clipboard;
use crate::platform::files::{self, FileDialogs, file_menu};
use crate::platform::window::CloseGuard;
use crate::theme::ThemeApplier;

/// Most log records moved into the app per frame, so a burst cannot stall
/// a frame (the terminal frontend uses the same limit).
const LOG_DRAIN_LIMIT: usize = 50;

pub use crate::panels::prompt::PROMPT_LABEL;

/// Event and log sources a running window drains each frame.
struct Sources {
    events: UnboundedReceiver<Event>,
    logs: TuiLogReceiver,
    runtime: Handle,
}

/// The codemason window: an [`eframe::App`] over the frontend-neutral
/// [`App`].
pub struct Shell {
    app: App,
    prompt: PromptInput,
    sources: Option<Sources>,
    keys: KeyRouter,
    messages: MessagesView,
    sessions: SessionsView,
    /// The dock layout drawn each frame.
    layout: Layout<PaneId>,
    /// Where each hidden pane was, so showing it again puts it back.
    hidden: HiddenPanes,
    /// The file the layout is loaded from and saved to, if any.
    layout_file: Option<PathBuf>,
    /// Whether the layout changed since it was last saved.
    layout_dirty: bool,
    /// The panes app-core sent the keyboard to last frame.
    key_panes: BTreeSet<PaneId>,
    /// Toggle chords that close panes closed from the dock's chrome, run
    /// through `App::handle_key` after drawing.
    close_keys: Vec<KeyInput>,
    /// Installs the visuals of the App's theme mode, again when it changes.
    theme: ThemeApplier,
    /// The native file dialogs the File menu queues.
    files: FileDialogs,
    /// What a close request does: quit, or ask while a turn runs.
    close: CloseGuard,
    /// app-core's exit steps have run (once, whichever path quit).
    quit_finished: bool,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("input", &self.app.session.input)
            .field("has_sources", &self.sources.is_some())
            .finish_non_exhaustive()
    }
}

impl Shell {
    /// Create a shell over `app` with no event or log sources attached.
    ///
    /// # Arguments
    ///
    /// * `app` - the application state to render.
    ///
    /// # Returns
    ///
    /// A shell over the App's prompt and the default keymap of the platform
    /// this build runs on.
    #[must_use]
    pub fn new(mut app: App) -> Self {
        // The GUI draws the keyboard shortcuts panel
        // (`dialogs::shortcuts`), so `?` on an empty prompt opens it, as in
        // the TUI.
        app.shortcuts_panel_enabled = true;
        Self {
            app,
            prompt: PromptInput::default(),
            sources: None,
            keys: KeyRouter::new(default_keymap(craft_keymap::Platform::current())),
            messages: MessagesView::default(),
            sessions: SessionsView::default(),
            layout: dock::default_layout(),
            hidden: HiddenPanes::new(),
            layout_file: None,
            layout_dirty: false,
            key_panes: BTreeSet::new(),
            close_keys: Vec::new(),
            theme: ThemeApplier::default(),
            files: FileDialogs::new(),
            close: CloseGuard::default(),
            quit_finished: false,
        }
    }

    /// Load the dock layout from `path` and save it back there when it
    /// changes ([`dock::load_layout`], [`dock::save_layout`]).
    ///
    /// # Arguments
    ///
    /// * `path` - the JSON layout file, e.g. [`dock::layout_path`]; a
    ///   missing or invalid file starts from [`dock::default_layout`].
    ///
    /// # Returns
    ///
    /// The shell drawing the stored layout.
    #[must_use]
    pub fn with_layout_file(mut self, path: PathBuf) -> Self {
        (self.layout, self.hidden) = dock::load_dock(&path);
        self.layout_file = Some(path);
        self
    }

    /// The dock layout as last drawn.
    ///
    /// # Returns
    ///
    /// The layout, after this frame's visibility sync and dock actions.
    #[doc(hidden)]
    #[must_use]
    pub fn layout(&self) -> &Layout<PaneId> {
        &self.layout
    }

    /// Replace the keyboard shortcuts.
    ///
    /// # Arguments
    ///
    /// * `keymap` - the bindings to use, e.g. from [`crate::keys::load_keymap`].
    ///
    /// # Returns
    ///
    /// The shell routing keys through `keymap`.
    #[must_use]
    pub fn with_keymap(mut self, keymap: craft_keymap::Keymap) -> Self {
        self.keys = KeyRouter::new(keymap);
        self
    }

    /// Attach the queues the shell drains each frame.
    ///
    /// # Arguments
    ///
    /// * `events` - agent events from [`crate::bridge::spawn_event_bridge`].
    /// * `logs` - tracing records for the log panel.
    /// * `runtime` - the tokio runtime that drives `App::handle_event`.
    ///
    /// # Returns
    ///
    /// The shell with its sources attached.
    #[must_use]
    pub fn with_sources(
        mut self,
        events: UnboundedReceiver<Event>,
        logs: TuiLogReceiver,
        runtime: Handle,
    ) -> Self {
        self.sources = Some(Sources {
            events,
            logs,
            runtime,
        });
        self
    }

    /// Draw the window contents into `ui`.
    ///
    /// # Arguments
    ///
    /// * `ui` - the root ui of the window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // A changed theme mode restyles the window (a repaint is requested,
        // so the next pass draws with the new visuals).
        self.theme.sync(ui.ctx(), self.app.theme_mode);
        self.sync_dock(ui.ctx());
        let palette = palette_for(self.app.theme_mode);
        let menus_enabled = !dialogs::locks_prompt(&self.app);
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                file_menu(ui, &mut self.files, &self.app.cwd_path, menus_enabled);
            });
        });
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            status_bar_ui(ui, &mut self.app, &palette);
        });
        let mut sessions_rect = None;
        egui::CentralPanel::default().show(ui, |ui| {
            sessions_rect = self.show_dock(ui, &palette);
        });
        dialogs::show(ui.ctx(), &mut self.app);
        self.close.show(ui.ctx(), &mut self.app);
        self.unfocus_sidebar_on_press_outside(ui.ctx(), sessions_rect);
        self.run_close_keys(ui.ctx());
        self.save_layout_when_due(ui.ctx());
    }

    /// Send the toggle chords of panes closed from the dock's chrome
    /// through `App::handle_key`, as the keyboard shortcut does. Without
    /// attached sources there is no runtime, so they are dropped.
    fn run_close_keys(&mut self, ctx: &egui::Context) {
        let keys = std::mem::take(&mut self.close_keys);
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        if keys.is_empty() {
            return;
        }
        for key in keys {
            sources.runtime.block_on(self.app.handle_key(key));
        }
        ctx.request_repaint();
    }

    /// Hide or show the dock's panes to match the App's flags.
    fn sync_dock(&mut self, ctx: &egui::Context) {
        let app = &self.app;
        let sidebar_was_front = self.layout.panel_state(&PaneId::Sessions) == PanelState::Front;
        match dock::sync_visibility(&mut self.layout, &mut self.hidden, |pane| {
            dock::pane_wanted(app, pane)
        }) {
            Ok(true) => {
                self.layout_dirty = true;
                ctx.request_repaint();
                // A pane toggled on in the focused sidebar's tab group (Alt+P,
                // Alt+M) is now in front of it. The TUI shows both side by
                // side; here the new pane stays in front and the sidebar gives
                // up the keyboard, as a press outside it does, so keys never
                // go to a covered pane.
                if self.app.sidebar.focused
                    && sidebar_was_front
                    && self.layout.panel_state(&PaneId::Sessions) == PanelState::Open
                {
                    sessions_sidebar_state::unfocus(&mut self.app);
                }
            }
            Ok(false) => {}
            Err(err) => tracing::warn!(%err, "dock visibility sync failed"),
        }
        self.front_key_panes(ctx);
    }

    /// Bring a pane app-core has just started sending the keyboard to to
    /// the front of its tab group, and keep the sessions sidebar in front
    /// while it has the keyboard (it shares a group with Profile and
    /// Memory; the TUI never covers it).
    fn front_key_panes(&mut self, ctx: &egui::Context) {
        let takes: BTreeSet<PaneId> = PaneId::ALL
            .into_iter()
            .filter(|pane| dock::pane_takes_keys(&self.app, *pane))
            .collect();
        let mut front: Vec<PaneId> = takes
            .iter()
            .copied()
            .filter(|pane| !self.key_panes.contains(pane) && *pane != PaneId::Sessions)
            .collect();
        // The sidebar goes last, so it wins a group it shares.
        if takes.contains(&PaneId::Sessions) {
            front.push(PaneId::Sessions);
        }
        for pane in front {
            match dock::bring_to_front(&mut self.layout, pane) {
                Ok(true) => {
                    self.layout_dirty = true;
                    ctx.request_repaint();
                }
                Ok(false) => {}
                Err(err) => tracing::debug!(?pane, %err, "pane not brought to front"),
            }
        }
        self.key_panes = takes;
    }

    /// Draw the dock and apply what the user did to it; returns the
    /// sessions sidebar's rectangle when it was drawn.
    fn show_dock(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<egui::Rect> {
        let style = DockStyle::from_ui(ui);
        let mut sessions_rect = None;
        let Self {
            app,
            messages,
            sessions,
            layout,
            ..
        } = self;
        let output = DockArea::new(egui::Id::unique("codemason_dock")).show(
            ui,
            layout,
            &style,
            |pane: &PaneId| pane.label().to_string(),
            |pane: &PaneId| pane.permissions(),
            |ui, pane| match pane {
                PaneId::Messages => messages_body(ui, app, messages, palette),
                PaneId::Prompt => {
                    queue_ui(ui, app);
                    prompt_ui(ui, app);
                }
                PaneId::Sessions => {
                    sessions.show(ui, app, palette);
                    sessions_rect = Some(ui.max_rect());
                }
                PaneId::Log => log_ui(ui, app, palette),
                PaneId::Profile => profile_ui(ui, app, palette),
                PaneId::Tasks => tasks_ui(ui, app, palette),
                PaneId::Memory => memory_ui(ui, app, palette),
                PaneId::ActiveAgents => agents_ui(ui, app, palette),
                PaneId::Teams => teams_ui(ui, app, palette),
                PaneId::Queue => placeholder(ui, *pane),
            },
        );
        if let Some(err) = output.error {
            tracing::warn!(%err, "dock layout not drawn");
        }
        for action in output.actions {
            match action {
                // The App's flag owns visibility: hide the pane through
                // app-core, and the next sync hides it where it is.
                Action::Close { panel } => {
                    if let Some(key) = dock::close_in_app(&mut self.app, panel) {
                        self.close_keys.push(key);
                    }
                    ui.ctx().request_repaint();
                }
                other => match self.layout.apply(other) {
                    Ok(_) => self.layout_dirty = true,
                    Err(err) => tracing::debug!(%err, "dock action rejected"),
                },
            }
        }
        sessions_rect
    }

    /// A press anywhere outside the sessions sidebar takes the keyboard
    /// from it, as a click outside it does in the TUI.
    fn unfocus_sidebar_on_press_outside(
        &mut self,
        ctx: &egui::Context,
        sessions_rect: Option<egui::Rect>,
    ) {
        if !self.app.sidebar.focused {
            return;
        }
        let press = ctx.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.press_origin())
                .flatten()
        });
        if let Some(at) = press
            && !sessions_rect.is_some_and(|rect| rect.contains(at))
            && ctx.memory(|m| m.top_modal_layer().is_none())
        {
            sessions_sidebar_state::unfocus(&mut self.app);
        }
    }

    /// Save a changed layout to the layout file once no pointer button is
    /// held, or at once when the window is asked to close.
    fn save_layout_when_due(&mut self, ctx: &egui::Context) {
        if !self.layout_dirty {
            return;
        }
        if self.layout_file.is_none() {
            return;
        }
        let closing = ctx.input(|i| i.viewport().close_requested()) || !self.app.is_running;
        if !closing && ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        self.save_layout();
    }

    /// Save the layout to the layout file, if there is one.
    fn save_layout(&mut self) {
        let Some(path) = self.layout_file.as_ref() else {
            return;
        };
        if let Err(err) = dock::save_dock(path, &self.layout, &self.hidden) {
            tracing::warn!(path = %path.display(), %err, "cannot save GUI layout");
        }
        self.layout_dirty = false;
    }

    /// Run one whole frame: [`Self::logic`], then route the keys and draw,
    /// then show a file dialog the frame queued (with no parent window).
    ///
    /// eframe calls the two halves separately (`eframe::App::logic` and
    /// `eframe::App::ui`); harnesses that drive only a `Ui` call this.
    ///
    /// # Arguments
    ///
    /// * `ui` - the root ui of the window.
    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.logic(ui.ctx());
        self.draw_frame(ui);
        self.files.show_pending(None);
    }

    /// The work that must go on whether or not the window is shown: drain
    /// the agent events and log records into the App, run app-core's
    /// housekeeping pass and the App's timers, and keep a wake scheduled for
    /// the next pass.
    ///
    /// eframe calls `eframe::App::logic` before every frame and, while the
    /// window is minimised or hidden (when it runs no frame at all), on
    /// every requested repaint, at most every 100 ms for an invisible
    /// window. The pass always requests the next wake (at most
    /// `HOUSEKEEPING_INTERVAL` away), so swarm unblocking and completion,
    /// autopilot, timed-question defaults and the history flush keep going
    /// while the window is hidden, as in the TUI. It reads no input and
    /// draws nothing.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's context.
    pub fn logic(&mut self, ctx: &egui::Context) {
        // App actions run on this thread; their clipboard writes go to the
        // window's clipboard (`forward_copies`).
        clipboard::route_copies_to_window();
        self.drain_sources();
        self.apply_file_answers(ctx);
        self.housekeeping(ctx);
        self.poll_timers(ctx);
        clipboard::forward_copies(ctx);
        // A close request quits, or asks first while a turn runs.
        self.close.logic(ctx, &mut self.app);
        // `/quit` (or any other path that stops the app) runs the exit steps
        // and closes the window.
        if !self.app.is_running {
            self.finish_quit();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Last, so it sees the guard's `CancelClose`: on macOS an accepted
        // close becomes `-[NSApplication terminate:]`.
        craft_quit::on_close_request(ctx);
    }

    /// Run app-core's exit steps once, whichever path quit: the same steps,
    /// in the same order, the terminal frontend runs after its loop
    /// (`App::save_on_quit`, then `App::stop_services_on_quit`), and save a
    /// changed layout (on macOS the process ends before another frame).
    /// Without attached sources there is no runtime, so only the first step
    /// runs.
    fn finish_quit(&mut self) {
        if self.quit_finished {
            return;
        }
        self.quit_finished = true;
        self.app.save_on_quit();
        if let Some(sources) = self.sources.as_ref() {
            let processor = std::sync::Arc::clone(&self.app.session_processor);
            sources
                .runtime
                .block_on(self.app.stop_services_on_quit(&processor));
        }
        if self.layout_dirty {
            self.save_layout();
        }
    }

    /// Route this frame's keys and draw it (the shown half of a frame).
    fn draw_frame(&mut self, ui: &mut egui::Ui) {
        // A context-menu item chosen last frame acts on the prompt editor
        // before the prompt reads this frame's input.
        dialogs::context_menu::apply_pending(ui.ctx());
        // An image pasted with Ctrl+V/Cmd+V is staged as Alt+V stages one.
        clipboard::attach_pasted_images(ui.ctx(), &mut self.app);
        let focus = self.prompt.prepare(ui.ctx(), &mut self.app);
        dialogs::prepare(ui.ctx(), &mut self.app);
        // While the quit confirmation asks, keys stay with it (Esc cancels
        // the dialog, not the turn).
        if !self.close.is_asking() {
            self.route_keys(ui.ctx(), focus);
        }
        // `/quit` (or any other path that stops the app) closes the window,
        // which ends `run_gui` the same way closing the window does.
        if !self.app.is_running {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.show(ui);
        self.run_dialog_keys(ui.ctx());
        clipboard::forward_copies(ui.ctx());
    }

    /// Answer every file dialog with `answer_with` instead of a native
    /// dialog (tests and automation); `None` shows native dialogs again.
    ///
    /// # Arguments
    ///
    /// * `answer_with` - the stand-in for the user's choice.
    #[doc(hidden)]
    pub fn set_file_dialog_override(&mut self, answer_with: Option<craft_dialogs::Override>) {
        self.files.set_override(answer_with);
    }

    /// The application state the shell renders.
    ///
    /// # Returns
    ///
    /// The App, including the input buffer the prompt edits.
    #[must_use]
    pub fn app(&self) -> &App {
        &self.app
    }

    /// The application state, for tests that change it between frames
    /// (e.g. to move app-core's test clock).
    ///
    /// # Returns
    ///
    /// The App the shell renders.
    #[doc(hidden)]
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Run app-core's background housekeeping pass (`App::housekeeping`):
    /// the polls that apply what background jobs deposited (compaction,
    /// rollback, `/swarm`, `/bench`, sub-agent launches, background
    /// sessions, store fetches, ...), gated by `HOUSEKEEPING_INTERVAL`, then
    /// the history flush. It runs on every [`Self::logic`] call, as the
    /// terminal frontend runs it every loop wake, and asks for a wake when
    /// the next pass is due: background jobs fill shared slots without
    /// waking anything, so that wake (the TUI's loop keeps the same one) is
    /// what lands their results while the user is idle or the window is
    /// hidden.
    ///
    /// It runs on the UI thread through `Handle::block_on`, like
    /// `App::handle_event`, the same pass the TUI runs on its render loop.
    /// Its awaits are short (lock `try_read`s that skip when held, small
    /// task-store writes): the one step that can run team hooks, a
    /// finalising swarm's task completions with their `TaskCompleted` hooks
    /// (30 s timeout each), runs in a job app-core spawns on the runtime,
    /// and a later pass applies its result. Without attached sources there
    /// is no runtime, so nothing runs.
    fn housekeeping(&mut self, ctx: &egui::Context) {
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        let processor = std::sync::Arc::clone(&self.app.session_processor);
        sources.runtime.block_on(self.app.housekeeping(&processor));
        ctx.request_repaint_after(
            self.app
                .housekeeping_due_at()
                .saturating_duration_since(std::time::Instant::now()),
        );
    }

    /// Run the App's wall-clock timers (`App::poll_timers`: status expiry,
    /// run-cost banner, timed question) every frame, whatever is drawn, and
    /// ask for a frame when the next one is due, so they fire while the
    /// window is idle too. A timed question behind another modal still
    /// answers its default, as in the TUI.
    fn poll_timers(&mut self, ctx: &egui::Context) {
        self.app.poll_timers();
        if let Some(due) = self.app.next_timer_due() {
            ctx.request_repaint_after(due);
        }
    }

    /// Route this frame's key presses to `App::handle_key`.
    ///
    /// Runs before any widget reads input, so a press that runs a keymap
    /// command is gone before the prompt sees it. While the prompt has the
    /// keyboard (`focus`), text it types stays with it (see
    /// [`KeyRouter::route`]). Without attached sources there is no runtime
    /// to drive the async handler, so the presses are left to the widgets.
    fn route_keys(&mut self, ctx: &egui::Context, focus: Focus) {
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        let keys = ctx.input_mut(|input| self.keys.route(input, focus));
        // egui chose where Tab or an arrow moves its focus when the frame
        // began, before the router took the press for the App; with the
        // press gone, its focus move is cancelled too (Tab that accepts a
        // completion stays in the prompt, a key the focused sidebar takes
        // moves no widget focus).
        let moves_focus = ctx.input(|input| input.events.iter().any(is_focus_move));
        if !moves_focus {
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        for key in keys {
            sources.runtime.block_on(self.app.handle_key(key));
        }
    }

    /// Run the key presses the dialogs' buttons sent this frame through
    /// `App::handle_key`, as typed keys, and repaint to show the result.
    /// Without attached sources there is no runtime, so they are dropped.
    fn run_dialog_keys(&mut self, ctx: &egui::Context) {
        let keys = dialogs::take_keys(ctx);
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        if keys.is_empty() {
            return;
        }
        for key in keys {
            sources.runtime.block_on(self.app.handle_key(key));
        }
        ctx.request_repaint();
    }

    /// Run what the answered file dialogs ask for (`files::apply_answer`):
    /// stage the chosen images, or run `/project add` / `/session new` with
    /// the chosen folder. The dialogs wake the window through `ctx` when
    /// they answer. Without attached sources there is no runtime, so the
    /// answers wait.
    fn apply_file_answers(&mut self, ctx: &egui::Context) {
        self.files.bind(ctx);
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        let answers = self.files.take_answers();
        if answers.is_empty() {
            return;
        }
        for (intent, answer) in answers {
            sources
                .runtime
                .block_on(files::apply_answer(&mut self.app, intent, answer));
        }
        ctx.request_repaint();
    }

    /// Move pending agent events and log records into the [`App`].
    fn drain_sources(&mut self) {
        let Some(sources) = self.sources.as_mut() else {
            return;
        };
        while let Ok(event) = sources.events.try_recv() {
            sources.runtime.block_on(self.app.handle_event(event));
        }
        for record in sources.logs.try_iter().take(LOG_DRAIN_LIMIT) {
            if record.message.is_empty() {
                continue;
            }
            let level = match record.level {
                tracing::Level::ERROR => LogLevel::Error,
                tracing::Level::WARN => LogLevel::Warn,
                _ => LogLevel::Info,
            };
            // Redact before display, as the terminal frontend does.
            let message = codemason_types::sanitize::redact_secrets(&record.message);
            self.app.push_log(level, message, None);
        }
    }
}

/// Whether `event` is a press egui moves its keyboard focus on: Tab,
/// Shift+Tab or an arrow key.
fn is_focus_move(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Key {
            key: egui::Key::Tab
                | egui::Key::ArrowUp
                | egui::Key::ArrowDown
                | egui::Key::ArrowLeft
                | egui::Key::ArrowRight,
            pressed: true,
            ..
        }
    )
}

/// The Messages pane: the transcript, whose right-click opens the context
/// menu.
fn messages_body(ui: &mut egui::Ui, app: &mut App, messages: &mut MessagesView, palette: &Palette) {
    research_ui(ui, app, palette);
    let transcript = messages.show(ui, app, palette);
    let right_click = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    if right_click && transcript.hovered() && !dialogs::locks_prompt(app) {
        // The transcript has no text selection in the GUI, so its menu has
        // nothing enabled, as the TUI's without one.
        dialogs::context_menu::open(ui.ctx(), app, SelectionPane::Messages, false);
    }
}

/// A pane with no panel of its own (the queue, drawn above the prompt): its
/// name, so the pane is labelled.
fn placeholder(ui: &mut egui::Ui, pane: PaneId) {
    ui.weak(format!("{} panel", pane.label()));
}

impl Drop for Shell {
    fn drop(&mut self) {
        // The window is gone: app-core writes the clipboard itself again.
        clipboard::stop_routing_copies();
    }
}

impl eframe::App for Shell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The inherent `Shell::logic` (inherent methods take precedence).
        Self::logic(self, ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // eframe has just called `logic` for this frame.
        self.draw_frame(ui);
        // Show a dialog the File menu queued, with the window as parent.
        self.files.show_pending(Some(frame));
    }

    fn on_exit(&mut self) {
        // Normally the close path has already run the exit steps. An exit
        // that skipped it (macOS `applicationWillTerminate:` at logout)
        // still saves what `/quit` saves.
        self.app.quit();
        self.finish_quit();
    }
}
