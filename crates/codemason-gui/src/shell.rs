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
//! ([`tasks_ui`]), the memories ([`memory_ui`]), the context-window
//! breakdown ([`context_ui`]) and the Agents and Teams window tables
//! ([`agents_ui`], [`teams_ui`]). The queue pane, whose list
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

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use codemason_agent::event::Event;
use codemason_app_core::app::{App, LogLevel};
use codemason_app_core::input_event::KeyInput;
use codemason_app_core::log_channel::TuiLogReceiver;
use codemason_app_core::sessions_sidebar_state;
use craft_ui::docking::{Action, DockArea, DockStyle, Layout, PanelState};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::dialogs;
use crate::dock::{self, HiddenPanes, PaneId};
use crate::keys::{Focus, KeyRouter, default_keymap};
use crate::page::{PageStyle, page_ui};
use crate::palette::Palette;
use crate::panels::agents::agents_ui;
use crate::panels::context::context_ui;
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
use crate::platform::files::{self, FileDialogs};
use crate::platform::menu::{self, MenuAction};
use crate::platform::window::{CloseGuard, WindowGeometry};
use crate::ribbon::{RibbonAction, RibbonState, ribbon_ui, title_strip_ui};
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
    /// Installs the visuals of the App's theme, again when it changes.
    theme: ThemeApplier,
    /// The native file dialogs the ribbon queues.
    files: FileDialogs,
    /// What a close request does: quit, or ask while a turn runs.
    close: CloseGuard,
    /// app-core's exit steps have run (once, whichever path quit).
    quit_finished: bool,
    /// Arm app-core's exit safety net when quitting (the real window only;
    /// a test process must not be ended by it).
    force_exit: bool,
    /// The longest the exit steps wait for the MCP servers to stop.
    quit_mcp_timeout: std::time::Duration,
    /// The native menu bar (macOS) and the layout it shows.
    native_menu: Option<(craft_menu::NativeMenu, craft_menu::Layout)>,
    /// The saved window geometry, kept up to date while the window runs.
    geometry: Option<WindowGeometry>,
    /// Set by the Ctrl+C / SIGTERM task ([`crate::signal`]): quit.
    shutdown_signal: Option<Arc<AtomicBool>>,
    /// The ribbon's open tab and collapsed state.
    ribbon: RibbonState,
    /// Where the ribbon's collapsed state is saved, when it is.
    ribbon_file: Option<PathBuf>,
    /// When (egui time) each pane was last shown, while it fades in.
    opened: BTreeMap<PaneId, f64>,
    /// Who draws the window frame ([`crate::title_bar`]); the system unless
    /// [`Shell::with_title_bar`] says otherwise.
    frame: craft_window::policy::Frame,
    /// The `movable` flag last given to the macOS window.
    #[cfg(target_os = "macos")]
    movable_applied: Option<bool>,
    /// `/editor`: the prompt out in an external editor.
    external_edit: crate::external_edit::ExternalEdit,
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
            force_exit: false,
            quit_mcp_timeout: codemason_app_core::app::quit::MCP_SHUTDOWN_TIMEOUT,
            native_menu: None,
            geometry: None,
            shutdown_signal: None,
            ribbon: RibbonState::default(),
            ribbon_file: None,
            opened: BTreeMap::new(),
            frame: craft_window::policy::Frame::Native,
            #[cfg(target_os = "macos")]
            movable_applied: None,
            external_edit: crate::external_edit::ExternalEdit::default(),
        }
    }

    /// Draw the window frame as `frame` says: with an app-drawn frame the
    /// title strip is the window's title bar on every theme, with caption
    /// buttons where the desktop puts them, and the window edges resize it.
    ///
    /// # Arguments
    ///
    /// * `frame` - the frame the window was opened with
    ///   ([`crate::title_bar::decide_frame`]).
    ///
    /// # Returns
    ///
    /// The shell drawing that frame.
    #[must_use]
    pub fn with_title_bar(mut self, frame: craft_window::policy::Frame) -> Self {
        self.frame = frame;
        self
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

    /// Load the ribbon's collapsed state from `path` and save it there
    /// whenever it changes ([`crate::ribbon::load_ribbon`],
    /// [`crate::ribbon::save_ribbon`]).
    ///
    /// # Arguments
    ///
    /// * `path` - the JSON state file, e.g. [`crate::ribbon::ribbon_path`].
    ///
    /// # Returns
    ///
    /// The shell with the saved ribbon state.
    #[must_use]
    pub fn with_ribbon_file(mut self, path: PathBuf) -> Self {
        self.ribbon = crate::ribbon::load_ribbon(&path);
        self.ribbon_file = Some(path);
        self
    }

    /// The panes still fading in after being shown.
    #[doc(hidden)]
    #[must_use]
    pub fn fading_panes(&self) -> Vec<PaneId> {
        self.opened.keys().copied().collect()
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

    /// Run `/editor` with `editor`, writing the prompt file in
    /// `scratch_dir`, instead of `$VISUAL` / `$EDITOR` and the system temp
    /// directory.
    ///
    /// # Returns
    ///
    /// The shell using that editor.
    #[must_use]
    pub fn with_external_editor(mut self, editor: String, scratch_dir: PathBuf) -> Self {
        self.external_edit = crate::external_edit::ExternalEdit::with_editor(editor, scratch_dir);
        self
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

    /// Add the user's `"<leader> x"` bindings (from
    /// [`crate::keys::load_leader_bindings`]); call after [`Self::with_keymap`].
    #[must_use]
    pub fn with_leader_bindings(
        mut self,
        bindings: std::collections::BTreeMap<char, String>,
    ) -> Self {
        self.keys.bind_after_leader(bindings);
        self
    }

    /// Keep `geometry` (the window placement eframe opened the window with)
    /// up to date: each frame records the window's state, a change is written
    /// once it holds, and the exit steps write it.
    ///
    /// # Arguments
    ///
    /// * `geometry` - the saved window geometry.
    ///
    /// # Returns
    ///
    /// The shell tracking the window's geometry.
    #[must_use]
    pub fn with_window_geometry(mut self, geometry: WindowGeometry) -> Self {
        self.geometry = Some(geometry);
        self
    }

    /// Drive `native` (the macOS menu bar from [`menu::install`]) each
    /// frame: its clicks run their App action, its key equivalents reach the
    /// key router as key presses, and its built-in Quit takes the close path.
    ///
    /// # Arguments
    ///
    /// * `native` - the installed menu's driver.
    /// * `keymap` - the keymap its items show the shortcuts of.
    ///
    /// # Returns
    ///
    /// The shell driving the menu.
    #[must_use]
    pub fn with_native_menu(
        mut self,
        native: craft_menu::NativeMenu,
        keymap: &craft_keymap::Keymap,
    ) -> Self {
        self.native_menu = Some((native, menu::mac_menu_layout(keymap)));
        self
    }

    /// Arm app-core's exit safety net ([`quit::arm_force_exit`]) when the
    /// window quits, as the terminal frontend does: the process ends
    /// [`quit::FORCE_EXIT_AFTER`] after the session is saved if teardown has
    /// not finished. For the real window only.
    ///
    /// [`quit::arm_force_exit`]: codemason_app_core::app::quit::arm_force_exit
    /// [`quit::FORCE_EXIT_AFTER`]: codemason_app_core::app::quit::FORCE_EXIT_AFTER
    ///
    /// # Returns
    ///
    /// The shell arming the net on quit.
    #[must_use]
    pub fn with_force_exit(mut self) -> Self {
        self.force_exit = true;
        self
    }

    /// Quit through the normal quit path once `flag` is set: the flag
    /// [`crate::signal::spawn_shutdown_signal_task`] sets on Ctrl+C or
    /// SIGTERM. [`Self::logic`] checks it each pass.
    ///
    /// # Arguments
    ///
    /// * `flag` - the shutdown flag.
    ///
    /// # Returns
    ///
    /// The shell watching the flag.
    #[must_use]
    pub fn with_shutdown_signal(mut self, flag: Arc<AtomicBool>) -> Self {
        self.shutdown_signal = Some(flag);
        self
    }

    /// Wait at most `timeout` for the MCP servers to stop when quitting
    /// (default [`MCP_SHUTDOWN_TIMEOUT`]; tests pass a short bound).
    ///
    /// [`MCP_SHUTDOWN_TIMEOUT`]: codemason_app_core::app::quit::MCP_SHUTDOWN_TIMEOUT
    ///
    /// # Arguments
    ///
    /// * `timeout` - the longest wait for the MCP shutdown.
    #[doc(hidden)]
    pub fn set_quit_mcp_timeout(&mut self, timeout: std::time::Duration) {
        self.quit_mcp_timeout = timeout;
    }

    /// Before egui reads the frame's input: add the native menu's key
    /// equivalents as the key presses they were (eframe's
    /// `raw_input_hook`). Nothing without a native menu.
    ///
    /// # Arguments
    ///
    /// * `raw` - the frame's raw input.
    pub fn native_menu_input(&mut self, raw: &mut egui::RawInput) {
        if let Some((native, _)) = self.native_menu.as_mut() {
            native.raw_input(raw);
        }
    }

    /// Run the native menu's clicks and keep it in step: while a dialog
    /// holds the keyboard only the built-in items run (Quit still asks the
    /// window to close). Without attached sources the clicks are dropped.
    fn run_native_menu(&mut self, ctx: &egui::Context) {
        let modal = dialogs::locks_prompt(&self.app) || self.close.is_asking();
        let Some((native, layout)) = self.native_menu.as_mut() else {
            return;
        };
        native.set_gate(craft_menu::Gate {
            modal,
            text_editing: ctx.egui_wants_keyboard_input(),
            ..craft_menu::Gate::default()
        });
        let clicks = native.take_clicks(ctx);
        let clashes = native.sync_with(0, || layout.clone());
        if !clashes.is_empty() {
            tracing::debug!(?clashes, "native menu shortcut clashes");
        }
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        for command in clicks {
            match menu::menu_action(&command) {
                Some(MenuAction::Key(key)) => {
                    sources.runtime.block_on(self.app.handle_key(key));
                }
                Some(MenuAction::Slash(line)) => {
                    sources
                        .runtime
                        .block_on(self.app.execute_slash_command(line));
                }
                None => tracing::debug!(?command, "menu command without an action"),
            }
            ctx.request_repaint();
        }
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
        // Panels and dialogs call App methods directly (a sidebar click
        // opens a session, which spawns): enter the runtime here too, so a
        // caller other than `draw_frame` cannot reach them outside it.
        let runtime = self.runtime();
        let _context = runtime.as_ref().map(Handle::enter);
        // A changed theme restyles the window (a repaint is requested,
        // so the next pass draws with the new visuals).
        let tokens = crate::theme::active_tokens(&self.app, ui.ctx());
        self.theme.sync(ui.ctx(), &tokens);
        self.sync_dock(ui.ctx());
        let palette = Palette::from_tokens(&tokens);
        let menus_enabled = !dialogs::locks_prompt(&self.app);
        let mut chosen = None;
        let title = self.window_title();
        let frame = &self.frame;
        egui::Panel::top("title_strip").show(ui, |ui| {
            // The caption buttons stay usable while a modal asks.
            chosen = title_strip_ui(ui, &palette, &title, menus_enabled, frame);
        });
        let was_collapsed = self.ribbon.collapsed;
        egui::Panel::top("ribbon").show(ui, |ui| {
            ui.add_enabled_ui(menus_enabled, |ui| {
                let app = &self.app;
                let shown = |pane| crate::dock::pane_wanted(app, pane);
                chosen = ribbon_ui(ui, &palette, &mut self.ribbon, &shown).or(chosen);
            });
        });
        if self.ribbon.collapsed != was_collapsed
            && let Some(path) = &self.ribbon_file
            && let Err(err) = crate::ribbon::save_ribbon(path, &self.ribbon)
        {
            tracing::warn!(%err, "ribbon state not saved");
        }
        if let Some(action) = chosen {
            self.run_ribbon_action(ui.ctx(), action);
        }
        let mut service_command = None;
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            // The interface controls take their places at the right first,
            // so the summary wraps in the width left over instead of running
            // under them. Right to left: the interface scale, then the YOLO
            // toggle, so YOLO sits to its left.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    crate::panels::status_bar::zoom_ui(ui, &palette);
                    crate::panels::status_bar::yolo_toggle_ui(ui, &mut self.app, &palette);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        service_command = status_bar_ui(ui, &mut self.app, &palette);
                    });
                });
            });
        });
        if let Some(line) = service_command {
            self.run_service_command(ui.ctx(), line);
        }
        let mut sessions_rect = None;
        egui::CentralPanel::default().show(ui, |ui| {
            sessions_rect = self.show_dock(ui, &palette);
        });
        if self.app.palette.is_some() {
            dialogs::palette::set_action_keys(ui.ctx(), self.keys.action_keys());
        }
        // The leader waits for its second key: after a moment, list the keys.
        let now = ui.input(|i| i.time);
        if let Some(waited) = self.keys.leader_waited(now) {
            if waited >= dialogs::which_key::WHICH_KEY_DELAY {
                dialogs::which_key::show(ui.ctx(), &self.keys.leader_entries());
            }
            // Wake to show the popup, and to end the wait at its timeout.
            let next = if waited < dialogs::which_key::WHICH_KEY_DELAY {
                dialogs::which_key::WHICH_KEY_DELAY - waited
            } else {
                (self.keys.leader_timeout() - waited).max(0.0) + 0.01
            };
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(next));
        }
        dialogs::show(ui.ctx(), &mut self.app);
        dialogs::image_view::show(ui.ctx(), &palette);
        self.close.show(ui.ctx(), &mut self.app);
        if matches!(self.frame, craft_window::policy::Frame::Integrated(_)) {
            crate::title_bar::resize_edges(ui.ctx());
        }
        #[cfg(target_os = "macos")]
        if matches!(self.frame, craft_window::policy::Frame::Inset(_)) {
            craft_window::title_bar::sync_movable(ui.ctx(), &mut self.movable_applied);
        }
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
        let shown_before: BTreeSet<PaneId> = PaneId::ALL
            .into_iter()
            .filter(|pane| self.layout.contains(pane))
            .collect();
        match dock::sync_visibility(&mut self.layout, &mut self.hidden, |pane| {
            dock::pane_wanted(app, pane)
        }) {
            Ok(true) => {
                self.layout_dirty = true;
                ctx.request_repaint();
                // A pane that has just come back fades in.
                let now = ctx.input(|i| i.time);
                for pane in PaneId::ALL {
                    if self.layout.contains(&pane) && !shown_before.contains(&pane) {
                        self.opened.insert(pane, now);
                    }
                }
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
    /// The title strip's text: the app name and the project folder.
    /// The title strip's text: the focused session's title (as the
    /// sidebar lists it), else the project folder, after the app name.
    fn window_title(&mut self) -> String {
        sessions_sidebar_state::refresh_if_dirty(&mut self.app);
        let subject = sessions_sidebar_state::current_title(&mut self.app).or_else(|| {
            self.app
                .cwd_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });
        match subject {
            Some(subject) => format!("codemason - {subject}"),
            None => "codemason".to_string(),
        }
    }

    /// Run a ribbon or quick access command, as the native menu runs its
    /// commands: a key press through `App::handle_key`, a slash command
    /// through `App::execute_slash_command`, a panel toggle as its keymap
    /// command's key press, and a file dialog through the window's
    /// dialogs. Without attached sources there is no runtime, so key presses
    /// and slash commands are dropped.
    fn run_ribbon_action(&mut self, ctx: &egui::Context, action: RibbonAction) {
        let key = match action {
            RibbonAction::Files(intent) => {
                let start = &self.app.cwd_path;
                let start = (!start.as_os_str().is_empty()).then_some(start.as_path());
                self.files.ask(intent, start);
                return;
            }
            RibbonAction::Panel(pane) => crate::dock::panel_command(pane)
                .and_then(|command| {
                    crate::keys::command_key_input(&craft_keymap::CommandRef::new(command))
                })
                .map(MenuAction::Key),
            RibbonAction::Menu(menu_action) => Some(menu_action),
        };
        let (Some(menu_action), Some(sources)) = (key, self.sources.as_ref()) else {
            tracing::debug!(?action, "ribbon command not run");
            return;
        };
        match menu_action {
            MenuAction::Key(key) => {
                sources.runtime.block_on(self.app.handle_key(key));
            }
            MenuAction::Slash(line) => {
                sources
                    .runtime
                    .block_on(self.app.execute_slash_command(line));
            }
        }
        ctx.request_repaint();
    }

    /// Run a status-bar service chip's `/<command> on|off` as the ribbon runs
    /// its slash commands, keeping the draft: running a slash command clears
    /// the prompt (it is normally the prompt's own submission), and flipping
    /// a service is not a submission.
    fn run_service_command(&mut self, ctx: &egui::Context, line: &'static str) {
        let draft = self.app.session.input.clone();
        let cursor = self.app.session.input_cursor;
        let menu = self.app.slash_menu.clone();
        self.run_ribbon_action(ctx, RibbonAction::Menu(MenuAction::Slash(line)));
        self.app.session.input = draft;
        self.app.session.input_cursor = cursor;
        self.app.slash_menu = menu;
    }

    fn show_dock(&mut self, ui: &mut egui::Ui, palette: &Palette) -> Option<egui::Rect> {
        let mut style = DockStyle::from_ui(ui);
        m3_dock_style(&mut style, palette);
        let mut sessions_rect = None;
        let now = ui.input(|i| i.time);
        let fade = crate::material::motion_time(ui.ctx());
        let Self {
            app,
            messages,
            sessions,
            layout,
            opened,
            ..
        } = self;
        let sessions_tab = crate::panels::sessions::tab_title(app);
        let output = DockArea::new(egui::Id::unique("codemason_dock")).show(
            ui,
            layout,
            &style,
            |pane: &PaneId| match pane {
                PaneId::Sessions => sessions_tab.clone(),
                other => other.label().to_string(),
            },
            |pane: &PaneId| pane.permissions(),
            |ui, pane| {
                // A pane that has just come back fades in.
                if let Some(&start) = opened.get(pane) {
                    match crate::material::fade_in_opacity(now - start, fade) {
                        Some(opacity) => {
                            ui.set_opacity(opacity);
                            ui.ctx().request_repaint();
                        }
                        None => {
                            opened.remove(pane);
                        }
                    }
                }
                match pane {
                    PaneId::Messages => {
                        page_ui(ui, palette, PageStyle::Page, |ui| {
                            messages_body(ui, app, messages, palette);
                        });
                    }
                    PaneId::Prompt => {
                        page_ui(ui, palette, PageStyle::Field, |ui| {
                            queue_ui(ui, app);
                            prompt_ui(ui, app);
                        });
                    }
                    PaneId::Sessions => {
                        crate::material::sheet::drawer_style(ui.style_mut(), palette);
                        sessions.show(ui, app, palette);
                        sessions_rect = Some(ui.max_rect());
                    }
                    PaneId::Log => log_ui(ui, app, palette),
                    PaneId::Profile => profile_ui(ui, app, palette),
                    PaneId::Tasks => tasks_ui(ui, app, palette),
                    PaneId::Memory => memory_ui(ui, app, palette),
                    PaneId::Context => context_ui(ui, app, palette),
                    PaneId::ActiveAgents => agents_ui(ui, app, palette),
                    PaneId::Teams => teams_ui(ui, app, palette),
                    PaneId::Queue => placeholder(ui, *pane),
                    PaneId::Diff => crate::panels::diff::diff_pane_ui(ui, app, palette),
                }
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
        let runtime = self.runtime();
        let _context = runtime.as_ref().map(Handle::enter);
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
    /// window. While background work is outstanding the pass requests the
    /// next wake (at most `HOUSEKEEPING_INTERVAL` away), and the timers
    /// request theirs, so swarm unblocking and completion, autopilot,
    /// timed-question defaults and the history flush keep going while the
    /// window is hidden, as in the TUI. It reads no input and
    /// draws nothing.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's context.
    pub fn logic(&mut self, ctx: &egui::Context) {
        // Every App call below runs inside the runtime context (see
        // `Self::runtime`).
        let runtime = self.runtime();
        let _context = runtime.as_ref().map(Handle::enter);
        // App actions run on this thread; their clipboard writes go to the
        // window's clipboard (`forward_copies`).
        clipboard::route_copies_to_window();
        self.drain_sources();
        self.apply_file_answers(ctx);
        self.housekeeping(ctx);
        self.poll_timers(ctx);
        self.external_edit.poll(ctx, &mut self.app);
        clipboard::forward_copies(ctx);
        if let Some(geometry) = self.geometry.as_mut() {
            geometry.track(ctx);
        }
        // Menu clicks (Quit asks the window to close, as the close button).
        self.run_native_menu(ctx);
        // A close request quits, or asks first while a turn runs.
        self.close.logic(ctx, &mut self.app);
        // Ctrl+C / SIGTERM quits as `/quit` does.
        if self.app.is_running
            && self
                .shutdown_signal
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            self.app.quit();
        }
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
    /// changed layout and the window geometry (on macOS the process ends
    /// before another frame). Everything is saved first; then the exit
    /// safety net is armed (with [`Self::with_force_exit`]) and the services
    /// are stopped, the MCP shutdown bounded. Without attached sources there
    /// is no runtime, so the services step does not run.
    fn finish_quit(&mut self) {
        if self.quit_finished {
            return;
        }
        self.quit_finished = true;
        self.app.save_on_quit();
        if self.layout_dirty {
            self.save_layout();
        }
        if let Some(geometry) = self.geometry.as_mut() {
            geometry.save();
        }
        if self.force_exit {
            codemason_app_core::app::quit::arm_force_exit(
                codemason_app_core::app::quit::FORCE_EXIT_AFTER,
            );
        }
        if let Some(sources) = self.sources.as_ref() {
            let processor = std::sync::Arc::clone(&self.app.session_processor);
            sources.runtime.block_on(
                self.app
                    .stop_services_on_quit_within(&processor, self.quit_mcp_timeout),
            );
        }
    }

    /// Route this frame's keys and draw it (the shown half of a frame).
    fn draw_frame(&mut self, ui: &mut egui::Ui) {
        // Panels and dialogs call App methods directly (a sidebar click
        // opens a session); they run inside the runtime context too.
        let runtime = self.runtime();
        let _context = runtime.as_ref().map(Handle::enter);
        // A context-menu item chosen last frame acts on the prompt editor
        // before the prompt reads this frame's input.
        dialogs::context_menu::apply_pending(ui.ctx());
        // The Diff pane's own keys, while it holds the keyboard focus, before
        // the prompt or the App can take them.
        crate::panels::diff::take_pane_keys(ui.ctx(), &mut self.app);
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

    /// The runtime the App's async work runs on, to enter for a whole frame.
    ///
    /// eframe runs the window on the process main thread, which is not a
    /// runtime thread. App methods that `tokio::spawn` or `spawn_blocking`
    /// (a session start announces its instruction files with
    /// `spawn_blocking`) panic there with "there is no reactor running"
    /// unless the runtime context is entered, so every entry point from
    /// eframe enters it for its whole duration, not only around
    /// `Handle::block_on`. `None` without attached sources.
    fn runtime(&self) -> Option<Handle> {
        self.sources.as_ref().map(|sources| sources.runtime.clone())
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
    /// terminal frontend runs it every loop wake. Background jobs fill
    /// shared slots without waking anything, so while one is outstanding
    /// (`App::has_pending_work`) it asks for a wake when the next pass is
    /// due, which lands the result while the user is idle or the window is
    /// hidden; with nothing outstanding the window sleeps.
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
        // Keep the wake for the next pass only while something is
        // outstanding (`App::has_pending_work`); otherwise sleep until
        // input, an agent event (the bridge wakes the window), the next
        // timer (`Self::poll_timers`) or a periodic step.
        let due = if self.app.has_pending_work() {
            Some(self.app.housekeeping_due_at())
        } else {
            self.app.next_periodic_due()
        };
        if let Some(due) = due {
            ctx.request_repaint_after(due.saturating_duration_since(std::time::Instant::now()));
        }
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
        let label_selection = ctx
            .with_plugin::<egui::text_selection::LabelSelectionState, _>(|s| s.has_selection())
            .unwrap_or(false);
        self.keys.set_label_selection(label_selection);
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
        // A modal dialog (or a locked prompt) keeps the keyboard: a keyless
        // action such as the palette must not open over it. Keyed actions
        // went through `App::handle_key`, which applies the same guards.
        let actions = self.keys.take_actions();
        if dialogs::active(&self.app).is_none() && !self.app.is_input_locked() {
            for action in actions {
                self.run_action(ctx, &action);
            }
        }
        // An action a palette row handed to the frontend.
        while let Some(action) = self.app.take_frontend_action() {
            self.run_action(ctx, &action);
        }
    }

    /// Run the catalog action `id` that has no terminal key press: through
    /// `App::run_action` when app-core knows it, else here.
    fn run_action(&mut self, ctx: &egui::Context, id: &str) {
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        if sources.runtime.block_on(self.app.run_action(id)) {
            ctx.request_repaint();
            return;
        }
        match id {
            "prompt.editor" => self.external_edit.start(ctx, &mut self.app),
            _ => tracing::debug!(action = id, "no handler for the action"),
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

/// Restyle the dock as Material side sheets (the word-processor layout):
/// panes on `surface_container_low` (the desk), tab strips on
/// `surface_container`, the open tab as a tonal (`secondary_container`)
/// tab, the accent in `primary` and dividers in `outline_variant`.
fn m3_dock_style(style: &mut DockStyle, palette: &Palette) {
    use crate::palette::rgb;

    let m3 = &palette.m3;
    style.background = rgb(m3.surface_container_low);
    style.tab_background = rgb(m3.surface_container);
    style.active_background = rgb(m3.secondary_container);
    style.text = rgb(m3.on_secondary_container);
    style.inactive_text = rgb(m3.on_surface_variant);
    style.accent = rgb(m3.primary);
    style.border = egui::Stroke::new(1.0, rgb(m3.outline_variant));
    style.tab_height = 32.0;
    style.floating_background = rgb(m3.surface_container_high);
    style.floating_stroke = egui::Stroke::new(1.0, rgb(m3.outline_variant));
}

/// The Messages pane: the transcript, whose right-click opens the context
/// menu.
fn messages_body(ui: &mut egui::Ui, app: &mut App, messages: &mut MessagesView, palette: &Palette) {
    research_ui(ui, app, palette);
    if crate::panels::empty_state::is_empty_session(app) {
        crate::panels::empty_state::empty_state_ui(ui, app, palette);
    }
    let transcript = messages.show(ui, app, palette);
    messages.apply_actions(ui.ctx(), app);
    let right_click = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    // `contains_pointer`, not `hovered`: the transcript's selectable text
    // sits on top of its hover-only node.
    if right_click && transcript.contains_pointer() && !dialogs::locks_prompt(app) {
        let message = messages.message_at_pointer().map(str::to_string);
        dialogs::context_menu::open_transcript(ui.ctx(), app, message);
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
        let runtime = self.runtime();
        let _context = runtime.as_ref().map(Handle::enter);
        // eframe has just called `logic` for this frame.
        self.draw_frame(ui);
        // Show a dialog the ribbon queued, with the window as parent.
        self.files.show_pending(Some(frame));
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.native_menu_input(raw_input);
    }

    fn on_exit(&mut self) {
        let runtime = self.runtime();
        let _context = runtime.as_ref().map(Handle::enter);
        // Normally the close path has already run the exit steps. An exit
        // that skipped it (macOS `applicationWillTerminate:` at logout)
        // still saves what `/quit` saves.
        self.app.quit();
        self.finish_quit();
    }
}
