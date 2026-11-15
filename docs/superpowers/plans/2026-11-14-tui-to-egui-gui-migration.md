# TUI (ratatui) to GUI (egui) Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace codemason's interactive ratatui/crossterm terminal UI (`crates/codemason-tui`) with a native desktop GUI built on egui/eframe and the storytold craft-libs UI crates, keeping feature parity. Then retire the TUI and rename `TUI-QUICKSTART.md` to `GUI-QUICKSTART.md`.

**Architecture:** This is a strangler migration in five phases. Each phase leaves `main` green.
1. Phase 1 pulls the UI-agnostic application core (state, slash commands, session ops, event handling) out of `codemason-tui` into a new `codemason-app-core` crate. That crate has no ratatui or crossterm types. The TUI keeps working through thin adapters, so the 258 existing test files keep passing.
2. Phases 2 to 4 build `codemason-gui` on top of the same core. The GUI starts behind `cdmn --gui` and grows to parity panel by panel.
3. Phase 5 switches the default and deletes `codemason-tui`, `ratatui` and `crossterm`.

**Tech Stack:** Rust 2024, eframe/egui 0.37 (the storytold fork, see Global Constraints), and these craft-libs crates pinned at git tag `v0.1.0`:

| craft crate | Features | Replaces / provides |
| --- | --- | --- |
| `craft-ui` | `serde`, `system_fonts`, `testing` | Docking (`docking::Layout`, `DockArea`), tabs (`tabs::Tab`), panes, buttons, `ComboBox`, icons, IME (`ime`), `theme::WidgetPalette`, kittest harness (`testing`) |
| `craft-keymap` | `egui` | The Alt+L / Alt+P / Alt+I / Alt+M / Alt+Q / Esc bindings now hard-coded in `app/input_handler.rs` (`Keymap`, `Binding`, `Scope`, `Lookup`, `reserved`) |
| `craft-dialogs` | default | Native file and folder pickers for attachments, `/new` and project open (`Dialogs<T>`, `FileDialog`, `Message`) |
| `craft-window` | `egui`, `restore` | Window geometry restore, the title bar, and detached session windows (`viewports::Viewports`, `restore`) |
| `craft-quit` + `craft-menu` | `egui`, `menu` | macOS app menu and close handling ("agent still running, quit?") |

Not used: `craft-markdown` (a document read/write model; rendering stays on `pulldown-cmark` and is converted to egui `LayoutJob`), `craft-settings` (`codemason-config` owns configuration), `craft-font-manager`, `craft-platform`, and every file-format crate.

**Spec:** None exists yet. This plan argues from the user request ("migrate codemason from a ratatui TUI to an egui GUI, use relevant craft-libs crates, migrate the TUI quickstart to a GUI quickstart") and the current behaviour documented in `TUI-QUICKSTART.md`, which serves as the parity checklist.

## Assumptions and open decisions (confirm before Task 0.1)

1. **The GUI replaces the TUI.** There is no long-term dual frontend. Non-interactive CLI modes (`cdmn run`, server, ACP) stay terminal-based and unchanged.
2. **Headless and SSH users lose the interactive mode** once Phase 5 lands. If that is not acceptable, Phase 5 instead keeps `codemason-tui` behind a `tui` cargo feature.
3. **Quickstart file name:** the existing file is `TUI-QUICKSTART.md`, not `QUICKSTART-TUI.md`. The plan uses `GUI-QUICKSTART.md` to keep the existing naming pattern. Swap in `QUICKSTART-GUI.md` if you meant that name literally.
4. **The egui fork is accepted.** craft-libs patches `egui`, `eframe`, `epaint`, `egui-winit` and others to `github.com/storytold/egui` at rev `6d518fc799681571963842f95b55a6b68431b15a`, and `winit` to `github.com/storytold/winit` at rev `a517b3846ee8457d84ee4975110bfda6797c44dc`. Cargo `[patch]` entries do not propagate from dependencies, so codemason must copy that patch table. Otherwise craft-ui may not compile against crates.io egui.
5. **Git dependencies are accepted** in `deny.toml`. Today `allow-git = []`, and craft-libs has not been published to crates.io.

## Global Constraints

- `rust-version` in the root `Cargo.toml` goes from `"1.91"` to `"1.95"`. craft-libs declares MSRV 1.95.
- craft-libs dependencies are `git = "https://github.com/storytold/craft-libs", tag = "v0.1.0"`, declared once in `[workspace.dependencies]`.
- `egui`, `eframe` and `egui_kittest` are `"0.37"`, declared once in `[workspace.dependencies]` and redirected by the copied `[patch.crates-io]` table (Assumption 4).
- The craft-libs licence is `MIT OR Apache-2.0`, which is compatible with codemason's MIT. `deny.toml` `[sources] allow-git` lists exactly: `https://github.com/storytold/craft-libs`, `https://github.com/storytold/egui`, `https://github.com/storytold/winit`.
- No new `unsafe`. craft-libs is `unsafe_code = "forbid"`. The new crates carry `#![deny(unsafe_code)]`.
- No `.unwrap()` on user-facing paths. Tests write only under `target/temp/` (AGENTS.md "Test Layout").
- Tests live in `crates/<crate>/tests/test_*.rs`, included from `tests/all.rs` with `autotests = false`. Run them with `cargo nextest run`.
- Run `cargo fmt` after every `.rs` edit. Every task finishes with `cargo clippy --workspace --all-targets` showing zero warnings.
- ASCII only in code and comments.
- GUI tests use `egui_kittest` queries over the accessibility tree (`Harness::get_by_label`, `query_by_label`), with no wgpu snapshots, so CI needs no GPU.

## Review Focus

1. **No display available** (`DISPLAY` and `WAYLAND_DISPLAY` unset, for example over SSH). Expected: `cdmn` exits non-zero with "no graphical display found; use `cdmn run` for headless mode" and does not panic. Test: Task 2.1 `test_gui_launch_no_display_returns_error`.
2. **Very long transcripts** (a 20,000-message session restored with `--resume`). Expected: scrolling stays above 30 fps, because only visible rows are laid out. Test: Task 3.1 `test_message_view_lays_out_only_visible_rows`.
3. **IME and non-ASCII input** (CJK composition, emoji paste) in the prompt editor. Expected: committed text arrives intact, and the cursor moves by grapheme. Test: Task 3.2 `test_prompt_editor_ime_commit_inserts_text`.
4. **Alt-chord shortcuts on macOS**, where Option+L types a symbol. Expected: the panel bindings stay reachable through Cmd-based defaults chosen per `craft_keymap::Platform::current()`. Test: Task 2.3 `test_keymap_mac_defaults_use_cmd`.
5. **Closing the window while an agent turn is running.** Expected: a confirmation dialog; confirming cancels the turn and saves the session, the same as `/quit`. Test: Task 4.2 `test_close_request_with_running_turn_prompts`.

---

## Phase 1: Extract a UI-agnostic core (about 1 to 2 weeks)

### Task 1.1: Create `codemason-app-core` with frontend-neutral text and input types

**Files:**
- Create: `crates/codemason-app-core/Cargo.toml`, `crates/codemason-app-core/src/lib.rs`, `src/styled.rs`, `src/input_event.rs`, `src/geometry.rs`, `tests/all.rs`, `tests/test_styled.rs`, `tests/test_input_event.rs`
- Create: `crates/codemason-tui/src/adapters.rs` (ratatui and crossterm conversions)
- Modify: root `Cargo.toml` (`[workspace.dependencies]` entry), `crates/codemason-tui/Cargo.toml`, `crates/codemason-tui/src/lib.rs`

**Interfaces:**
- Produces, in `codemason_app_core::styled`:
  - `pub enum ThemeColor { Accent, Text, Muted, Error, Warning, Success, Info, Border, Rgb(u8, u8, u8) }`
  - `pub struct TextStyle { pub fg: Option<ThemeColor>, pub bg: Option<ThemeColor>, pub bold: bool, pub italic: bool, pub underline: bool, pub dim: bool, pub monospace: bool }`
  - `pub struct StyledSpan { pub text: String, pub style: TextStyle }`
  - `pub struct StyledLine { pub spans: Vec<StyledSpan> }`
  - All of these derive `Debug, Clone, PartialEq, Default`.
- Produces, in `codemason_app_core::input_event`:
  - `pub enum Key { Char(char), Enter, Esc, Tab, BackTab, Backspace, Delete, Up, Down, Left, Right, Home, End, PageUp, PageDown, F(u8) }`
  - `pub struct Modifiers { pub ctrl: bool, pub alt: bool, pub shift: bool, pub cmd: bool }`
  - `pub struct KeyInput { pub key: Key, pub mods: Modifiers }`
  - `pub enum UiEvent { Key(KeyInput), Paste(String), Scroll { lines: i32 }, Resize { cols: u16, rows: u16 } }`
- Produces, in `codemason_app_core::geometry`: `pub struct Area { pub x: u16, pub y: u16, pub width: u16, pub height: u16 }`
- Produces, in `codemason_tui::adapters`:
  - `pub fn to_ratatui_line(line: &StyledLine) -> ratatui::text::Line<'static>` (resolves `ThemeColor` via the existing `theme::{heading, muted, error, ...}` style fns and the current `theme::ThemeMode`)
  - `pub fn from_crossterm_key(ev: crossterm::event::KeyEvent) -> Option<KeyInput>`
  - `impl From<Area> for ratatui::layout::Rect` and the reverse.

- [ ] **Step 1: Write failing tests.**
  - `test_styled::test_styled_line_plain_text_roundtrip`: `StyledLine::plain("hi").text() == "hi"`.
  - `test_input_event::test_from_crossterm_alt_l`: `KeyEvent(Char('l'), ALT)` maps to `KeyInput { key: Char('l'), mods: alt }`.
  - The second test lives in `crates/codemason-tui/tests/test_adapters.rs`.
- [ ] **Step 2: Run the tests to see them fail.** `cargo nextest run -p codemason-app-core -p codemason-tui -E 'test(test_styled::) + test(test_adapters::)'` must FAIL to compile.
- [ ] **Step 3: Implement the types.** Add `StyledLine::plain(&str) -> Self` and `StyledLine::text(&self) -> String`. `to_ratatui_line` maps each `ThemeColor` onto the matching existing `theme.rs` style fn.
- [ ] **Step 4: Verify.** The same command must PASS. `cargo clippy -p codemason-app-core -p codemason-tui --all-targets` must show 0 warnings.
- [ ] **Step 5: Commit** with the message `feat(app-core): add frontend-neutral styled text and input types`.

### Task 1.2: Move ratatui-free modules into `codemason-app-core`

About 39k of the TUI's 81k lines already import no ratatui or crossterm type. These include `app/slash.rs` (15k lines), `research_adapter.rs`, `branch_names.rs`, `tips.rs`, `open_in.rs`, `clipboard.rs`, and most of `app/*` except the files named in Task 1.3.

**Files:**
- Move (git mv): every file under `crates/codemason-tui/src` where `grep -LE "ratatui|crossterm"` matches, except `lib.rs`. The grep baseline is recorded in the commit message.
- Modify: `crates/codemason-tui/src/lib.rs` and `src/app.rs` gain `pub use codemason_app_core::<module>;` re-exports, so `codemason_tui::<path>` keeps resolving for `src/main.rs`, `src/plugins.rs` and the existing tests.
- Move: the matching `crates/codemason-tui/tests/test_*.rs` files into `crates/codemason-app-core/tests/`. Update both `tests/all.rs` files.

**Interfaces:**
- Produces: `codemason_app_core::{app::SLASH_COMMANDS, research_adapter, branch_names, tips, open_in, clipboard, ...}` with unchanged public signatures.

- [ ] **Step 1: Record the baseline test count.** Run `cargo nextest list -p codemason-tui 2>&1 | tail -1 > target/temp/tui-baseline.txt`.
- [ ] **Step 2: Move the modules** and add the re-exports.
- [ ] **Step 3: Verify.** `timeout 1500 cargo nextest run -p codemason-tui -p codemason-app-core --no-fail-fast 2>&1 | tee target/temp/p1t2.log` must PASS, and the combined test count must equal the baseline.
- [ ] **Step 4: Commit** with the message `refactor: move ratatui-free modules to codemason-app-core`.

### Task 1.3: Decouple `App` state and input handling from ratatui and crossterm

**Files:**
- Modify, then move into app-core: `app/state.rs`. Today line 8 uses `ratatui::layout::Rect`, and lines 494, 496, 515, 517, 1596 and 1631 hold `ratatui::text::Line<'static>`.
- Modify, then move: `app/session_ops.rs`, `app/input_handler.rs`, `input.rs`, `input_field.rs`, `app/loop_dialog.rs`, `session_menu.rs`, `project_properties.rs`. These currently take crossterm `KeyEvent`.
- Move to app-core as well: `StartupSession` (`lib.rs:215`) and `theme::ThemeMode` (`theme.rs:138`); `codemason-tui` re-exports both.
- Keep in `codemason-tui` (render only): `layout*.rs`, `widgets/*`, `theme.rs`, `sessions_sidebar.rs` rendering, `teammate_strip.rs`, `tracing_layer.rs`, `utils.rs`, and `lib.rs` (`TerminalGuard`, `run_tui`).

**Interfaces:**
- Consumes: Task 1.1 types.
- Produces:
  - `App` fields use `Vec<StyledLine>` instead of `Vec<Line<'static>>`, and `Area` instead of `Rect`.
  - `pub fn handle_key(&mut self, input: KeyInput) -> HandleOutcome` replaces the crossterm-typed entry point.
  - `pub enum HandleOutcome { Consumed, Ignored, Quit }`.
  - Wrapped-line caches stay width-keyed (`wrapped_lines: Vec<StyledLine>`, `wrap_width: u16`), so the TUI can still reuse them.
  - `codemason_tui::run_tui` converts crossterm events with `adapters::from_crossterm_key` before calling `App::handle_key`.

- [ ] **Step 1: Write a failing test.** `test_app_core_input::test_handle_key_alt_l_toggles_log_panel` builds `support::make_app()`, sends `KeyInput { Char('l'), alt }`, and asserts `app.show_log_panel` flips.
- [ ] **Step 2: Run it.** `cargo nextest run -p codemason-app-core -E 'test(test_app_core_input::)'` must FAIL.
- [ ] **Step 3: Refactor** one file at a time. After each file, `cargo check -p codemason-tui` must pass. Rendering in `layout.rs` calls `adapters::to_ratatui_line` at draw time.
- [ ] **Step 4: Verify.** `timeout 1500 cargo nextest run -p codemason-tui -p codemason-app-core -p codemason --no-fail-fast 2>&1 | tee target/temp/p1t3.log` must PASS. Also `grep -rE "ratatui|crossterm" crates/codemason-app-core` must print nothing, and `cargo tree -p codemason-app-core -i ratatui` must report that ratatui is not in the tree.
- [ ] **Step 5: Commit** with the message `refactor(app-core): make App state and input frontend-neutral`.

---

## Phase 2: GUI shell (about 1 week)

### Task 2.1: `codemason-gui` crate, dependency plumbing and the `--gui` launch path

**Files:**
- Create: `crates/codemason-gui/{Cargo.toml, src/lib.rs, src/shell.rs, src/bridge.rs, tests/all.rs, tests/test_launch.rs}`
- Modify: root `Cargo.toml`. Set `rust-version = "1.95"`, add the `[workspace.dependencies]` entries for egui, eframe, egui_kittest and the craft crates, add the `[patch.crates-io]` table copied from craft-libs `Cargo.toml` at tag `v0.1.0`, and add `codemason-gui` as a dependency of the root binary.
- Modify: `deny.toml` `allow-git` (the three URLs in Global Constraints). `src/cli.rs` gains the `--gui` flag. `src/main.rs` dispatches to `codemason_gui::run_gui` beside `run_tui` (around lines 1291 and 1535).

**Interfaces:**
- Consumes: `codemason_app_core::app::App`, `StartupSession`, and the tracing log receiver from `tracing_layer::tui_log_channel` (moved to app-core in Task 1.2 or 1.3).
- Produces:
  - `pub async fn run_gui(opts: GuiOptions) -> anyhow::Result<()>`
  - `pub struct GuiOptions { pub startup: StartupSession, pub log_rx: LogReceiver, ... }`, carrying the same fields `run_tui` takes today.
  - `pub enum GuiLaunchError { NoDisplay, Backend(String) }` (thiserror).
  - In `bridge.rs`: `pub fn spawn_event_bridge(bus: EventBus, ctx: egui::Context) -> JoinHandle<()>`. It forwards agent events into the `App` queue and calls `ctx.request_repaint()`. eframe owns the main thread, and the tokio runtime runs on a worker thread.

- [ ] **Step 1: Write failing tests.**
  - `test_launch::test_gui_launch_no_display_returns_error`: with `global_env_lock()` held, remove `DISPLAY` and `WAYLAND_DISPLAY` and assert `matches!(check_display(), Err(GuiLaunchError::NoDisplay))`.
  - `test_launch::test_shell_renders_empty_session`: a kittest `Harness` over `Shell::new(make_app())`, asserting `harness.get_by_label("Prompt")` exists.
- [ ] **Step 2: Run them.** `cargo nextest run -p codemason-gui` must FAIL.
- [ ] **Step 3: Implement `Shell: eframe::App`.** It draws a central placeholder and a prompt `TextEdit` with the accessible label `"Prompt"`. Implement `check_display() -> Result<(), GuiLaunchError>` (Linux and BSD only; other platforms return `Ok`).
- [ ] **Step 4: Verify.**
  - `cargo build` must succeed with the fork patch applied.
  - `cargo deny check sources` must pass.
  - `cargo nextest run -p codemason-gui` must PASS.
  - `cargo run -- --gui` must open a window on a desktop. This last check is manual and is reported to the user, not claimed.
- [ ] **Step 5: Commit** with the message `feat(gui): add egui shell and --gui launch path`.

### Task 2.2: Docked layout with craft-ui

**Files:** `crates/codemason-gui/src/dock.rs`, `tests/test_dock.rs`

**Interfaces:**
- Produces:
  - `pub enum PaneId { Messages, Prompt, Sessions, Log, Profile, Tasks, Memory, ActiveAgents, Teams, Queue }`, deriving `serde`.
  - `pub fn default_layout() -> craft_ui::docking::Layout<PaneId>`
  - `pub fn toggle_pane(layout: &mut Layout<PaneId>, pane: PaneId)`
  - The layout is persisted with feature `serde` to `<config_dir>/codemason/gui-layout.json`. Tests pass the path explicitly under `target/temp/`.

- [ ] **Step 1: Write failing tests.**
  - `test_dock_default_layout_contains_messages_and_prompt`.
  - `test_dock_toggle_log_twice_restores_layout`: after two toggles, `Layout::location(PaneId::Log)` is unchanged.
  - `test_dock_layout_roundtrips_json`.
- [ ] **Step 2: Run them, implement, and verify.** `cargo nextest run -p codemason-gui -E 'test(test_dock::)'` goes from FAIL to PASS.
- [ ] **Step 3: Commit** with the message `feat(gui): dockable pane layout`.

### Task 2.3: Keymap via craft-keymap

**Files:** `crates/codemason-gui/src/keys.rs`, `tests/test_keys.rs`

**Interfaces:**
- Consumes: `App::handle_key(KeyInput)` from Task 1.3.
- Produces:
  - `pub fn default_keymap(platform: craft_keymap::Platform) -> craft_keymap::Keymap`. The command ids are `panel.log`, `panel.profile`, `panel.tasks`, `panel.memory`, `queue.menu`, `agent.stop` and `app.quit`.
  - `pub fn to_key_input(ev: &egui::Event) -> Option<KeyInput>`. Unbound keys fall through to `App::handle_key`.
  - User overrides are loaded from `craft_keymap::Overrides` stored beside the layout file.

- [ ] **Step 1: Write failing tests.**
  - `test_keymap_linux_defaults_use_alt`: `panel.log` is bound to `Alt+L` on `Platform::Windows` and Linux.
  - `test_keymap_mac_defaults_use_cmd`: `panel.log` is bound to `Cmd+Shift+L` on `Platform::Mac`.
  - `test_to_key_input_maps_escape`.
- [ ] **Step 2: Run them, implement, and verify.** `-E 'test(test_keys::)'` goes from FAIL to PASS.
- [ ] **Step 3: Commit** with the message `feat(gui): craft-keymap bindings`.

---

## Phase 3: Widget parity (about 3 to 4 weeks; the largest phase)

Each task below ports one TUI surface. The parity source is the matching `TUI-QUICKSTART.md` section, and the logic stays in app-core. Every task has the same shape:
- a kittest test asserting the surface's labelled elements and one interaction;
- an implementation file in `crates/codemason-gui/src/panels/`;
- verification with `cargo nextest run -p codemason-gui -E 'test(<file>::)'`;
- one commit.

### Task 3.1: Message transcript view

**Files:** `src/panels/messages.rs`, `src/markdown.rs`, `tests/test_messages.rs`, `tests/test_markdown.rs`

**Interfaces:**
- Produces:
  - `pub fn styled_to_layout_job(lines: &[StyledLine], palette: &Palette, wrap_width: f32) -> egui::text::LayoutJob`
  - `pub fn markdown_to_styled(md: &str) -> Vec<StyledLine>`, which moves from `widgets/message_widget.rs` into app-core and keeps its `pulldown-cmark` base.
  - The view uses `egui::ScrollArea::vertical().show_rows(...)` over cached per-message row heights.

- [ ] **Step 1: Write failing tests.**
  - `test_markdown_code_block_is_monospace`.
  - `test_message_view_lays_out_only_visible_rows`: with 20,000 messages and a 600 px viewport, the layout counter (a `#[doc(hidden)]` counter on the view) records at most 200 rows laid out per frame.
  - `test_message_view_tool_call_shows_status_label`.
- [ ] **Step 2: Implement, verify and commit.**

### Task 3.2: Prompt editor, mentions and input queue

**Files:** `src/panels/prompt.rs`, `src/panels/queue.rs`, `tests/test_prompt.rs`

**Interfaces:** Consumes the app-core `input::InputState`. Produces `pub fn prompt_ui(ui: &mut egui::Ui, app: &mut App)`, which uses `craft_ui::ime` for composition.

- [ ] **Step 1: Write failing tests.**
  - `test_prompt_enter_submits_and_clears`.
  - `test_prompt_shift_enter_inserts_newline`.
  - `test_prompt_at_mention_opens_completion`.
  - `test_prompt_editor_ime_commit_inserts_text`: send `egui::Event::Ime(ImeEvent::Commit("你好"))` and assert the prompt text equals `"你好"`.
  - `test_queue_menu_shows_queued_items`.
- [ ] **Step 2: Implement, verify and commit.**

### Task 3.3: Dialogs

The dialogs are permission, provider setup, model picker, loop setup, project properties and the session menu.

**Files:** `src/dialogs/{permission,provider,model_picker,loop_setup,project_properties,session_menu}.rs`, `tests/test_dialogs.rs`

**Interfaces:** Each file exposes `pub fn show(ctx: &egui::Context, app: &mut App) -> bool` (`true` while open), built on `egui::Modal`, `craft_ui::buttons` and `craft_ui::ComboBox`.

- [ ] **Step 1: Write failing tests.**
  - `test_permission_dialog_allow_once_resolves_request`.
  - `test_model_picker_filter_narrows_list`.
  - `test_provider_dialog_saves_selection`.
  - `test_loop_dialog_rejects_empty_goal`.
- [ ] **Step 2: Implement, verify and commit.**

### Task 3.4: Side panels

The side panels are sessions sidebar, log, profile, tasks, memory, active agents, teams and status bar.

**Files:** `src/panels/{sessions,log,profile,tasks,memory,agents,teams,status_bar}.rs`, `tests/test_panels.rs`

- [ ] **Step 1: Write failing tests.**
  - `test_sessions_panel_select_switches_session`.
  - `test_log_panel_shows_tracing_lines`.
  - `test_tasks_panel_lists_tasks`.
  - `test_memory_panel_clear_requires_confirm`.
  - `test_status_bar_shows_model_and_cost`.
- [ ] **Step 2: Implement, verify and commit.** If a panel grows past about 400 lines, split it into its own commit.

### Task 3.5: Research and deck progress views, and theme mapping

**Files:** `src/panels/research.rs`, `src/theme.rs`, `tests/test_theme.rs`

**Interfaces:** `pub struct Palette` (one `egui::Color32` per `ThemeColor`) and `pub fn palette_for(mode: ThemeMode) -> Palette`, with `ThemeMode` moved from `codemason-tui/src/theme.rs` to app-core in Task 1.3, which maps every `ThemeColor` variant. It applies widget colours with `craft_ui::theme::WidgetPalette::apply_to`.

- [ ] **Step 1: Write failing tests.**
  - `test_theme_every_theme_color_resolves`.
  - `test_research_progress_shows_phase_label`.
- [ ] **Step 2: Implement, verify and commit.**

---

## Phase 4: Platform integration (about 1 week)

### Task 4.1: Native file dialogs and clipboard

**Files:** `src/platform/files.rs`, `src/platform/clipboard.rs`, `tests/test_files.rs`

**Interfaces:**
- `craft_dialogs::Dialogs<FileIntent>` holds the pending requests.
- `pub enum FileIntent { Attach, OpenProject, NewProjectDir }`.
- Answers are drained each frame with `take_answers()`. The `Wake` callback calls `ctx.request_repaint()`.
- Image paste keeps using `arboard`, and text paste uses `egui::Event::Paste`.
- Tests use `Dialogs::set_override` to inject answers.

- [ ] **Step 1: Write failing tests.**
  - `test_attach_dialog_answer_adds_attachment`.
  - `test_cancelled_dialog_changes_nothing`.
- [ ] **Step 2: Implement, verify and commit.**

### Task 4.2: Window lifecycle: close confirmation, macOS menu, geometry restore

**Files:** `src/platform/window.rs`, `tests/test_window.rs`

**Interfaces:**
- `craft_quit` with the `egui` feature intercepts `ViewportCommand::Close`.
- `craft_menu` builds the macOS app menu (macOS only, `#[cfg(target_os = "macos")]`).
- `craft_window::restore` persists the window rect beside the layout file.

- [ ] **Step 1: Write failing tests.**
  - `test_close_request_with_running_turn_prompts`.
  - `test_close_request_idle_closes_immediately`.
  - `test_window_rect_roundtrips`.
- [ ] **Step 2: Implement, verify and commit.**

---

## Phase 5: Cutover and TUI removal (about 3 to 4 days)

### Task 5.1: Make the GUI the default and delete the TUI

**Files:**
- Delete: `crates/codemason-tui/` (source, tests and the four benches). First port `bench_markdown` to `crates/codemason-app-core/benches/` because its target, markdown rendering, now lives in app-core.
- Modify:
  - root `Cargo.toml`: remove the `codemason-tui` dependency and its `telemetry` feature mapping, remove the `ratatui` and `crossterm` workspace dependencies, and change keywords from `"tui"` to `"gui"`.
  - `description`: change it to "An AI coding agent desktop app, built in Rust."
  - `src/main.rs`: `run_gui` becomes the default interactive path, and `--gui` stays as an accepted no-op.
  - `src/plugins.rs`: change `codemason_tui::app::SLASH_COMMANDS` to `codemason_app_core::app::SLASH_COMMANDS`.
  - `src/main.rs:408`: the logo moves to app-core.

- [ ] **Step 1: Write a failing test.** In root `tests/test_cli_gui_default.rs`, `test_cli_default_interactive_mode_is_gui` parses empty argv and asserts `Mode::Gui`.
- [ ] **Step 2: Remove the TUI and fix the build.**
- [ ] **Step 3: Verify.**
  - `timeout 1500 cargo nextest run --workspace --no-fail-fast 2>&1 | tee target/temp/p5.log` must PASS.
  - `cargo test --doc` must PASS.
  - `cargo clippy --workspace --all-targets` must show 0 warnings.
  - `cargo tree -i ratatui` must report the package as not found.
  - `cargo deny check` must pass.
  - `scripts/check-test-env-isolation.sh` must pass.
- [ ] **Step 4: Commit** with the message `feat!: replace ratatui TUI with egui GUI`.

### Task 5.2: Migrate the quickstart and other docs

**Files:**
- Rename: `git mv TUI-QUICKSTART.md GUI-QUICKSTART.md`, then rewrite it.
- Modify: `AGENTS.md` (the root-exception list in "Documentation Standards": `TUI-QUICKSTART.md` becomes `GUI-QUICKSTART.md`), `README.md`, `QUICKSTART.md`, `SPEC.md`, `CHANGELOG.md` (`### Changed` and `### Removed` under Unreleased).

Rewrite rules for `GUI-QUICKSTART.md`:
- Keep the section structure (1 to 13 and onward).
- Section 1 becomes "Starting the GUI": launching `cdmn`, the no-display fallback, and `cdmn run` for headless use.
- Replace every terminal keystroke with the craft-keymap default from Task 2.3, giving the Linux/Windows and macOS forms side by side.
- Describe panels as dockable panes: drag tabs, double-click to float, and the layout persists.
- Add "Attaching files" (native dialog plus drag-and-drop) and "Window and layout reset" sections.
- Drop terminal-only content: bracketed paste, terminal selection and copy modes, alternate-screen notes.
- Slash commands (`/loop`, `/research`, `/spec`, `/new`, `/plugins`) carry over unchanged because their logic lives in app-core.

- [ ] **Step 1: Verify there are no stale references.** `grep -rn "TUI-QUICKSTART\|ratatui\|crossterm" --include=*.md . | grep -v "^./specs/\|^./CHANGELOG.md\|^./docs/superpowers/"` must print nothing. Historical spec plans stay untouched.
- [ ] **Step 2: Commit** with the message `docs: migrate TUI-QUICKSTART to GUI-QUICKSTART`.
