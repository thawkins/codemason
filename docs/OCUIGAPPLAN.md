# OCUIGAP Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining aesthetic, UI and UX gaps in
[`OCUIGAP.md`](OCUIGAP.md) so the Codemason GUI reads like a finished product
next to opencode v2.

**Architecture:** app-core turns each message into `TranscriptBlock`s
(`crates/codemason-app-core/src/message_blocks.rs`), and the GUI draws each
block with theme tokens (`crates/codemason-app-core/src/theme_tokens.rs` →
`crates/codemason-gui/src/palette.rs`). Every task in this plan either adds a
block variant or derived model in app-core (no egui types), or a widget, pane
or dialog in `codemason-gui` that draws it. Each phase ends in its own PR.

**Tech Stack:** Rust 2024, egui/eframe 0.37 (storytold fork), craft-ui /
craft-keymap / craft-dialogs, `egui_commonmark` 0.26, `egui_extras`,
`tree-sitter-highlight` 0.27, `similar` (workspace), egui_kittest, nextest.

**Spec:** [`docs/OCUIGAP.md`](OCUIGAP.md) (gaps) and
[`docs/OCGAPPLAN.md`](OCGAPPLAN.md) (phase design). This plan is the
task-level breakdown of OCGAPPLAN phases 2-6.

## Status at the time of writing (2026-11-18, `feat/ui-phase2` at `d4bd4f1`)

| OCGAPPLAN phase | State | Evidence |
|---|---|---|
| 0 Quick wins | Merged (PR #45). 0.4 Queue pane listing was reverted (`e54cfd4`); the placeholder text is gone | `9a9357e`..`12bf091` |
| 1 Theme tokens | Merged (PR #46) | `ThemeTokens`, `ThemeCatalog`, `metrics.rs`, `scripts/check-theme-literals.sh` |
| 2 Rich rendering | Step 1 committed (`d4bd4f1`, `TranscriptBlock`). Markdown via `egui_commonmark`, tree-sitter highlighting (`syntax_highlight.rs`) and code-block chrome (`panels/code_block.rs`) are **uncommitted work in the tree** | `git status` |
| 3-6 | Not started | — |

Task 1 lands the uncommitted phase 2 work; it belongs to whoever is editing it.
Do not overwrite, revert or restash it. If that work is still in progress,
start at Task 2 on top of it once it is committed.

## Global Constraints

- Run `cargo fmt` after every `.rs` edit; `cargo fmt --check` and
  `cargo clippy --workspace --all-targets` clean with zero warnings.
- Tests go in `crates/<crate>/tests/test_<unit>.rs`, registered as one
  `#[path = "test_<unit>.rs"] mod test_<unit>;` line in that crate's
  `tests/all.rs`. No inline `#[cfg(test)]` modules. Run with
  `timeout 1500 cargo nextest run -p <crate> -E 'test(test_<unit>::)'`.
- Tests write only under `target/temp/` (resolved from
  `env!("CARGO_MANIFEST_DIR")`); config/theme/keymap paths go through explicit
  parameters, never the real `~/.config`.
- No colour literals outside the theme modules: `scripts/check-theme-literals.sh`
  must pass. New colours are new `ThemeTokens` fields filled in every built-in
  theme JSON.
- Spacing, radius and text sizes come from `crates/codemason-gui/src/metrics.rs`
  (`space::*`, `radius::*`, `text_size::*`); no new magic numbers.
- app-core stays egui-free. GUI widgets get an AccessKit role and label, and
  every new widget is found in its test through egui_kittest accessibility
  queries.
- No `.unwrap()`/`.expect()` on user-facing paths; no new `unsafe`.
- New assets (icon font) must be OFL/MIT/ISC, recorded in `NOTICE`.
- Each task that adds a user-visible command, setting or key updates
  `SPEC.md` (GUI section), `GUI-QUICKSTART.md` and `CHANGELOG.md`
  (`[Unreleased]`) in the same commit. Never describe a feature that is not
  in the GUI (OCUIGAP section 8).
- Commit per task; do not push.

## Review Focus

1. **Streaming a half-written message.** An unterminated fence, table or
   list in the middle of a stream must render as text-so-far, never panic or
   flash. Pinned by `test_markdown_stream_unterminated_fence` (Task 1).
2. **Very large content.** A 2 MB tool result, a 10k-line diff or a 64 KiB+
   code block must stay responsive: highlight is skipped above
   `MAX_HIGHLIGHT_BYTES`, diffs truncate at `DIFF_MAX_LINES`. Pinned by
   `test_diff_view_truncates_large_diff` (Task 8) and
   `test_activity_large_result_preview_truncated` (Task 6).
3. **User themes missing new tokens.** A user theme written before a token
   was added must still load (serde default from the theme's `dark` base),
   not be rejected with `deny_unknown_fields`/missing-field errors. Pinned by
   `test_theme_tokens_user_theme_without_new_fields_loads` (Task 11).
4. **Keybinding conflicts.** A user binding that collides with a default or
   the leader key must be reported in the Log pane and the user binding wins,
   not silently dropped. Pinned by `test_keys_user_binding_overrides_default`
   (Task 12).
5. **Collapse state across re-renders.** Expanding an activity row or a
   thinking block must survive streaming updates and scrolling (stable
   `egui::Id` from message id + block index). Pinned by
   `test_activity_row_stays_open_while_streaming` (Task 7).

---

## Phase 2 — Rich transcript (finish)

### Task 1: Land markdown, syntax highlighting and code-block chrome

**Files:**
- Already modified in tree: `crates/codemason-app-core/src/{message_blocks.rs,syntax_highlight.rs,lib.rs}`,
  `crates/codemason-gui/src/{panels/messages.rs,panels/code_block.rs,palette.rs,shell.rs,dialogs/output_view.rs}`,
  `Cargo.toml`, both crate manifests, `Cargo.lock`
- Test: `crates/codemason-app-core/tests/test_syntax_highlight.rs`,
  `crates/codemason-app-core/tests/test_message_blocks.rs`,
  `crates/codemason-gui/tests/test_messages.rs`

**Interfaces:**
- Produces: `syntax_highlight::highlight(code: &str, lang: &str) -> Option<Vec<HighlightSpan>>`,
  `MAX_HIGHLIGHT_BYTES`, `code_block::code_block_ui(..) -> Option<CodeAction>`,
  `CodeAction::{Copy, ToggleWrap, Insert}` (as defined in the tree).

- [ ] **Step 1:** Confirm with the owner of the uncommitted change that it is
  ready to land; if not, stop and resume at Task 2 after they commit.
- [ ] **Step 2: Add the streaming test** `test_markdown_stream_unterminated_fence`
  in `test_message_blocks.rs`: `split_code_blocks("text\n```rust\nfn a(")`
  returns `[Markdown("text"), Code { lang: Some("rust"), text: "fn a(" }]`.
- [ ] **Step 3:** Add `test_highlight_skips_oversized_input`:
  `highlight(&"a".repeat(MAX_HIGHLIGHT_BYTES + 1), "rust")` is `None`.
- [ ] **Step 4: Run** `timeout 1500 cargo nextest run -p codemason-app-core -E 'test(test_message_blocks::) + test(test_syntax_highlight::)'`
  and `-p codemason-gui -E 'test(test_messages::)'`. Expected: PASS.
- [ ] **Step 5:** Run `scripts/check-theme-literals.sh`, `cargo fmt --check`,
  `cargo clippy --workspace --all-targets`. Commit together with `Cargo.lock`
  (`git add Cargo.lock` explicitly).

### Task 2: Clickable links and tables

**Files:**
- Modify: `crates/codemason-gui/src/panels/messages.rs` (markdown block rendering)
- Test: `crates/codemason-gui/tests/test_messages.rs`

**Interfaces:**
- Consumes: `TranscriptBlock::Markdown(String)`.
- Produces: `pub fn open_link(ctx: &egui::Context, url: &str)` in
  `crates/codemason-gui/src/platform.rs` — opens only `http`, `https` and
  `mailto` schemes via `ctx.open_url`; others are ignored and logged.

- [ ] **Step 1: Write failing tests:**
  `test_messages_markdown_link_is_a_link` — a message `"see [docs](https://x.y)"`
  exposes an AccessKit node with role `Link` and label `docs`;
  `test_messages_markdown_table_renders_cells` — `"|a|b|\n|-|-|\n|1|2|"` exposes
  text nodes `a`, `b`, `1`, `2` and no raw `|-|-|` text;
  `test_open_link_rejects_file_scheme` — `open_link(ctx, "file:///etc/passwd")`
  leaves `ctx.output(|o| o.open_url.clone())` as `None`.
- [ ] **Step 2: Run** `cargo nextest run -p codemason-gui -E 'test(test_messages::)'`. Expected: FAIL.
- [ ] **Step 3:** Configure the `egui_commonmark` viewer to render tables and
  route link clicks through `open_link`; link colour is
  `tokens.markdown.link`.
- [ ] **Step 4: Run** the filter again. Expected: PASS. Commit.

### Task 3: Selection and copy in the transcript

**Files:**
- Modify: `crates/codemason-gui/src/panels/messages.rs`,
  `crates/codemason-gui/src/dialogs/context_menu.rs`, `crates/codemason-gui/src/shell.rs`
  (remove the "no text selection" note)
- Create: `crates/codemason-app-core/src/message_copy.rs`
- Test: `crates/codemason-app-core/tests/test_message_copy.rs`,
  `crates/codemason-gui/tests/test_messages.rs`

**Interfaces:**
- Consumes: `TranscriptBlock::plain_text()`.
- Produces: `pub fn message_markdown(blocks: &[TranscriptBlock]) -> String`
  (code blocks re-fenced with their language, tool/thinking blocks omitted),
  `pub fn quote_for_reply(text: &str) -> String` (every line prefixed `"> "`,
  trailing blank line) in `message_copy.rs`.

- [ ] **Step 1: Write failing tests:** `test_message_copy_refences_code`
  (`[Markdown("a"), Code{lang:Some("rs"),text:"x"}]` → `"a\n\n```rs\nx\n```\n"`),
  `test_message_copy_omits_tool_and_thinking`,
  `test_message_copy_quote_prefixes_lines` (`"a\nb"` → `"> a\n> b\n\n"`),
  and GUI `test_messages_context_menu_copy_enabled` (right-click a message:
  menu items `Copy`, `Copy as Markdown`, `Quote in reply` are enabled; clicking
  `Quote in reply` puts `"> hello\n\n"` in the composer).
- [ ] **Step 2: Run** both crate filters. Expected: FAIL.
- [ ] **Step 3:** Implement `message_copy.rs`; make markdown and plain blocks
  selectable (`egui::Label::selectable(true)` / commonmark selectable text);
  enable the three context-menu items.
- [ ] **Step 4: Run** both filters. Expected: PASS. Commit.

### Task 4: Message framing, inline images and streaming caret

**Files:**
- Modify: `crates/codemason-gui/src/panels/messages.rs`,
  `crates/codemason-app-core/src/message_lines.rs` (drop `You: ` / `* ` prefixes
  from block-rendered roles only; `StyledLine` output for the Log pane is unchanged)
- Test: `crates/codemason-gui/tests/test_messages.rs`

**Interfaces:**
- Consumes: `TranscriptBlock::{Plain, Image}`, `tokens.bg.raised1`,
  `tokens.accent`, `metrics::{space, radius}`.
- Produces: `pub const THUMBNAIL_MAX: f32 = 240.0` in `panels/messages.rs`.

- [ ] **Step 1: Write failing tests:**
  `test_messages_user_turn_in_card` — the user message is inside a group
  node labelled `"You"` and its text has no `You:` prefix;
  `test_messages_image_block_shows_thumbnail` — an `Image` block pointing at a
  PNG written under `target/temp/test_messages/` exposes an `Image` role node
  whose bounds are at most `THUMBNAIL_MAX` wide; clicking it opens the
  output-view dialog;
  `test_messages_missing_image_shows_name` — a nonexistent path renders the
  file name as text, no panic;
  `test_messages_streaming_caret_only_while_processing`.
- [ ] **Step 2: Run** `-E 'test(test_messages::)'`. Expected: FAIL.
- [ ] **Step 3:** User turns: `egui::Frame` with `bg.raised1` fill,
  `radius::MD`, `space::MD` inner margin. Assistant turns: 2 px left rule in
  `accent`. Vertical gap between turns `space::LG`. Images via
  `egui::Image::from_uri("file://…")` (install `egui_extras` image loaders
  once at startup). Caret: a `▍` glyph (ASCII `|` fallback through
  `styled_text::drawable`) appended to the last block while
  `session.is_processing`.
- [ ] **Step 4: Run** the filter. Expected: PASS. Update SPEC.md GUI section
  (markdown, highlighting, copy, images now true). Commit; open the phase 2 PR.

## Phase 3 — Quiet transcripts and diffs

### Task 5: Verbosity and thinking settings

**Files:**
- Create: `crates/codemason-app-core/src/transcript_prefs.rs`
- Modify: config schema (the crate holding `ui.theme`, `crates/codemason-config`),
  `crates/codemason-app-core/src/app/slash.rs`, `app/state.rs` (command registry)
- Test: `crates/codemason-app-core/tests/test_transcript_prefs.rs`

**Interfaces:**
- Produces: `pub enum Verbosity { Low, Medium, High }` (default `Low`),
  `pub enum ThinkingDisplay { Collapsed, Expanded, Hidden }` (default
  `Collapsed`), both `FromStr`/`Display` with lowercase names;
  `pub struct TranscriptPrefs { pub verbosity: Verbosity, pub thinking: ThinkingDisplay }`
  on `App`; config keys `ui.verbosity`, `ui.thinking`; slash commands
  `/verbosity low|medium|high` and `/thinking show|hide|collapse`.

- [ ] **Step 1: Write failing tests:** parse/display round-trip for every
  variant; `test_transcript_prefs_unknown_value_rejected` (`"loud"` →
  `Err`, status line names the valid values); `/verbosity high` sets
  `app.transcript_prefs.verbosity == High`.
- [ ] **Step 2: Run** `-p codemason-app-core -E 'test(test_transcript_prefs::)'`. Expected: FAIL.
- [ ] **Step 3:** Implement; persist like `/theme` does.
- [ ] **Step 4: Run.** Expected: PASS. Commit.

### Task 6: Activity grouping in app-core

**Files:**
- Create: `crates/codemason-app-core/src/activity.rs`
- Modify: `crates/codemason-app-core/src/message_blocks.rs`
- Test: `crates/codemason-app-core/tests/test_activity.rs`

**Interfaces:**
- Consumes: `TranscriptBlock::{Tool, Thinking}`, `Verbosity` (Task 5).
- Produces:
  - `TranscriptBlock::Activity(ActivityGroup)` (new variant).
  - `pub struct ActivityGroup { pub summary: ActivitySummary, pub items: Vec<TranscriptBlock>, pub running: bool }`
  - `pub struct ActivitySummary { pub reads: u32, pub edits: u32, pub commands: u32, pub searches: u32, pub other: u32, pub thoughts: u32, pub duration_ms: u64, pub errors: u32 }`
    with `pub fn label(&self) -> String`.
  - `pub fn group_activity(blocks: Vec<TranscriptBlock>, verbosity: Verbosity) -> Vec<TranscriptBlock>`:
    `Low` folds every maximal run of consecutive `Tool`/`Thinking` blocks into
    one `Activity`; `Medium` folds only `Thinking` and keeps tools as
    individual blocks; `High` returns the input unchanged.
  - `pub const RESULT_PREVIEW_MAX_LINES: usize = 20`.
- Tool category comes from the tool name: `read`/`view`/`list`/`glob` →
  reads; `edit`/`write`/`multiedit`/`patch` → edits; `bash`/`exec` →
  commands; `grep`/`search`/`codeindex_*` → searches; else other.

- [ ] **Step 1: Write failing tests** with exact labels:
  4 reads + 2 edits + 1 bash, 12 300 ms → `"Read 4 files, edited 2 files, ran 1 command (12 s)"`;
  1 read → `"Read 1 file"` (no duration under 1 s);
  1 error among 3 → label ends `", 1 failed"`;
  `[Markdown, Tool, Tool, Markdown, Tool]` at `Low` → 2 `Activity` groups with the markdown between them kept;
  `High` → identity;
  `test_activity_large_result_preview_truncated` — a tool with 5 000 result lines keeps `RESULT_PREVIEW_MAX_LINES` lines plus a `"… 4 980 more lines"` line.
- [ ] **Step 2: Run** `-E 'test(test_activity::)'`. Expected: FAIL.
- [ ] **Step 3:** Implement; call `group_activity` at the end of `message_to_blocks` with the app's verbosity.
- [ ] **Step 4: Run** plus `test_message_blocks::`. Expected: PASS. Commit.

### Task 7: Activity rows and thinking blocks in the GUI

**Files:**
- Modify: `crates/codemason-gui/src/panels/messages.rs`
- Test: `crates/codemason-gui/tests/test_activity_rows.rs` (new, registered in `tests/all.rs`)

**Interfaces:**
- Consumes: `TranscriptBlock::Activity(ActivityGroup)`, `ThinkingDisplay`.
- Produces: `fn block_id(message_id: &str, block_index: usize) -> egui::Id` in `panels/messages.rs` (used for every collapsible block).

- [ ] **Step 1: Write failing tests:** a collapsed activity row is a button
  node labelled with `summary.label()` and `aria-expanded=false`; clicking
  expands it to show each tool header; a running group shows an
  `egui::Spinner` (`ProgressIndicator` role);
  `test_activity_row_stays_open_while_streaming` — expand, append a new tool
  part to the same message, re-render: still expanded;
  thinking block: header `"Thinking"`, expanded while the message streams,
  collapsed after it completes under `Collapsed`, absent under `Hidden`.
- [ ] **Step 2: Run** `-p codemason-gui -E 'test(test_activity_rows::)'`. Expected: FAIL.
- [ ] **Step 3:** Draw with `egui::collapsing_header::CollapsingState` keyed by
  `block_id`; header text `text.muted`, failure count in `state.error`.
- [ ] **Step 4: Run.** Expected: PASS. Commit.

### Task 8: Diff model

**Files:**
- Create: `crates/codemason-app-core/src/diff_view.rs`
- Modify: `crates/codemason-app-core/Cargo.toml` (`similar = { workspace = true }`)
- Test: `crates/codemason-app-core/tests/test_diff_view.rs`

**Interfaces:**
- Produces:
  - `pub enum DiffLineKind { Context, Added, Removed }`
  - `pub struct DiffLine { pub kind: DiffLineKind, pub old_no: Option<u32>, pub new_no: Option<u32>, pub text: String }`
  - `pub struct DiffHunk { pub header: String, pub lines: Vec<DiffLine> }`
  - `pub struct FileDiff { pub path: String, pub hunks: Vec<DiffHunk>, pub added: u32, pub removed: u32, pub truncated: bool }`
  - `pub fn file_diff(path: &str, old: &str, new: &str) -> FileDiff` (3 lines context, `similar::TextDiff::from_lines`)
  - `pub fn diffs_from_tool_input(tool: &str, input: &serde_json::Value) -> Vec<FileDiff>`:
    `edit` uses `file_path`/`old_string`/`new_string`; `multiedit` one
    `FileDiff` per entry of `edits`; `write` → all lines added from `content`;
    anything else → empty. Step 1 verifies these key names against
    `crates/codemason-tools-core` before writing tests.
  - `pub const DIFF_MAX_LINES: usize = 2_000`.

- [ ] **Step 1:** `grep -n "old_string\|new_string\|edits\|content" crates/codemason-tools-core/src/{edit,multiedit,write}*.rs`; use the real key names in the tests.
- [ ] **Step 2: Write failing tests:** one-line change in a 10-line file → 1
  hunk, header `"@@ -3,7 +3,7 @@"`, `added == 1`, `removed == 1`, line
  numbers correct; `write` of 3 lines → 3 `Added`; unknown tool → empty;
  `test_diff_view_truncates_large_diff` — a 10 000-line rewrite yields at most
  `DIFF_MAX_LINES` lines and `truncated == true`.
- [ ] **Step 3: Run** `-E 'test(test_diff_view::)'`. Expected: FAIL.
- [ ] **Step 4:** Implement. Add `TranscriptBlock::Diff(FileDiff)` emitted
  after an edit tool's block in `message_to_blocks` (inside the activity
  group at `Low`, standalone at `Medium`/`High`).
- [ ] **Step 5: Run** plus `test_message_blocks::`. Expected: PASS. Commit.

### Task 9: Inline diff widget and Diff pane

**Files:**
- Create: `crates/codemason-gui/src/panels/diff.rs`
- Modify: `crates/codemason-gui/src/dock.rs` (`PaneId::Diff`, `ALL: [Self; 12]`,
  label `"Diff"`, not in the default layout), `panels.rs`, `panels/messages.rs`
- Test: `crates/codemason-gui/tests/test_diff_pane.rs`

**Interfaces:**
- Consumes: `FileDiff`, `tokens.diff.*`.
- Produces: `pub fn inline_diff_ui(ui: &mut egui::Ui, diff: &FileDiff, palette: &Palette) -> bool`
  (returns true when "Open diff" is clicked; shows the first hunk, collapsed
  rest); `pub enum DiffLayout { Unified, Split }`;
  `pub struct DiffPaneState { pub files: Vec<FileDiff>, pub selected: usize, pub hunk: usize, pub layout: DiffLayout, pub reviewed: HashSet<String> }`
  on `App`; keys inside the pane: `]`/`[` next/previous hunk, `n`/`p`
  next/previous file, `v` toggle layout, `m` mark reviewed.

- [ ] **Step 1: Write failing tests:** inline diff shows `+`/`-` rows with
  line numbers and an `Open diff` button; clicking it shows the Diff pane
  with the file selected; `]` moves `hunk` from 0 to 1; `v` switches to
  `Split` and the pane shows two columns; `m` collapses the file and adds its
  path to `reviewed`; closing the Diff pane and reopening keeps `reviewed`.
- [ ] **Step 2: Run** `-E 'test(test_diff_pane::) + test(test_dock::)'`. Expected: FAIL (`test_dock` counts 11 panes; update it to 12 in this step).
- [ ] **Step 3:** Implement; split view via two `egui::Grid` columns, added
  rows `diff.added_bg`, removed rows `diff.removed_bg`.
- [ ] **Step 4: Run.** Expected: PASS. Commit.

### Task 10: Diff and command preview in the permission dialog; icon font

**Files:**
- Modify: `crates/codemason-gui/src/dialogs/permission.rs`, `fonts.rs`,
  `styled_text.rs` (glyph fallback table), `NOTICE`
- Create: `crates/codemason-gui/assets/fonts/lucide.ttf`,
  `crates/codemason-gui/src/icons.rs`
- Test: `crates/codemason-gui/tests/test_dialogs.rs`, `tests/test_fonts.rs`

**Interfaces:**
- Consumes: `diffs_from_tool_input` (Task 8), `inline_diff_ui` (Task 9).
- Produces: `pub enum Icon { Read, Edit, Command, Search, Agent, Check, Cross, Warning, Info, ChevronRight, ChevronDown, Copy }`
  with `pub fn glyph(self) -> &'static str` (Lucide code point) and
  `pub fn ascii(self) -> &'static str` (`"r"`, `"e"`, `"$"`, `"?"`, `"@"`, `"ok"`, `"x"`, `"!"`, `"i"`, `">"`, `"v"`, `"cp"`).

- [ ] **Step 1: Write failing tests:** a permission request for an `edit`
  shows an inline diff node; for `bash` shows the full command in a
  monospace block (not only the first pattern); for an unknown tool shows the
  existing pattern text; `test_fonts_icon_glyphs_present` — every `Icon`
  glyph resolves in the installed font stack; with the icon font removed from
  the stack, `drawable` returns `ascii()`.
- [ ] **Step 2: Run** `-E 'test(test_dialogs::) + test(test_fonts::)'`. Expected: FAIL.
- [ ] **Step 3:** Implement; use `Icon` in activity rows, tool headers and
  status glyphs in place of `● ✗ ◆ ℹ ⚠` and `[ok]`/`[x]`.
- [ ] **Step 4: Run.** Expected: PASS. Update SPEC/quickstart/changelog.
  Commit; open the phase 3 PR.

### Task 11: Token additions stay backward compatible

**Files:**
- Modify: `crates/codemason-app-core/src/theme_tokens.rs`, built-in theme JSON files
- Test: `crates/codemason-app-core/tests/test_theme_tokens.rs`

Any token added in Tasks 4-10 (for example a caret or activity-row colour)
must be `#[serde(default)]`-able. Do this task as soon as the first new
token appears.

**Interfaces:**
- Produces: for each new field, a `fn default_<field>(dark: bool) -> Rgb`
  fallback used when the field is absent.

- [ ] **Step 1: Write failing test** `test_theme_tokens_user_theme_without_new_fields_loads`:
  a JSON theme copied from the phase 1 `codemason-dark` (before the new
  fields) parses with `ThemeTokens::from_json`, and the new fields equal
  their defaults.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Add serde defaults.
  **Step 4: Run.** Expected: PASS. Commit.

## Phase 4 — Discoverability and keyboard

### Task 12: Every action rebindable

**Files:**
- Modify: `crates/codemason-gui/src/keys.rs` (`default_keymap`, `load_keymap`),
  `crates/codemason-app-core/src/shortcuts.rs`
- Test: `crates/codemason-gui/tests/test_keys.rs`

**Interfaces:**
- Produces: `pub fn action_catalog() -> &'static [ActionSpec]` in
  `shortcuts.rs`, `pub struct ActionSpec { pub id: &'static str, pub title: &'static str, pub default_key: Option<&'static str> }`;
  ids are dotted lowercase (`session.new`, `pane.toggle.context`,
  `prompt.editor`, …), one per entry in the current `?` overlay plus every
  pane toggle. `default_keymap` registers every `ActionSpec`.
  `load_keymap(platform, path) -> (Keymap, Vec<String>)` — second value is
  warnings (unknown id, unparseable key, conflict).

- [ ] **Step 1: Write failing tests:** `action_catalog()` ids are unique and
  every one is bound in `default_keymap`; a `gui-keys.json` under
  `target/temp/test_keys/` rebinding `session.new` to `ctrl+shift+n` routes
  that key to the action; `test_keys_user_binding_overrides_default` — a
  user binding that collides with another default wins and produces one
  warning naming both ids; an unknown id produces a warning, not an error.
- [ ] **Step 2: Run** `-E 'test(test_keys::)'`. Expected: FAIL.
- [ ] **Step 3:** Implement; write warnings to the Log pane at startup.
- [ ] **Step 4: Run.** Expected: PASS. Commit.

### Task 13: Fuzzy matcher and command palette

**Files:**
- Create: `crates/codemason-app-core/src/fuzzy.rs`, `crates/codemason-gui/src/dialogs/palette.rs`
- Modify: `dialogs/mod.rs`, `keys.rs` (bind `palette.show` to Ctrl+P and Ctrl+Shift+P)
- Test: `crates/codemason-app-core/tests/test_fuzzy.rs`, `crates/codemason-gui/tests/test_palette.rs`

**Interfaces:**
- Produces: `pub fn fuzzy_score(query: &str, candidate: &str) -> Option<u32>`
  (case-insensitive subsequence match; bonus for word-start and consecutive
  matches; `None` when not a subsequence; empty query scores 0) and
  `pub fn fuzzy_rank<'a, T>(query: &str, items: &'a [T], key: impl Fn(&T) -> &str) -> Vec<&'a T>`
  (stable for equal scores). Palette entries:
  `pub struct PaletteEntry { pub id: String, pub title: String, pub hint: Option<String>, pub key: Option<String> }`,
  `pub fn palette_entries(app: &App) -> Vec<PaletteEntry>` covering
  `action_catalog()`, slash commands, themes, models, agents and sessions
  (`id` prefixes `action:`, `slash:`, `theme:`, `model:`, `agent:`, `session:`).

- [ ] **Step 1: Write failing tests:** `fuzzy_score("ns", "new session") > fuzzy_score("ns", "transcript")`;
  `fuzzy_score("xyz", "new session") == None`; ranking is stable; palette:
  Ctrl+P opens a dialog with a focused search field; typing `theme nord`
  puts `theme:nord` first; Enter applies it and closes; Esc closes without
  change; each row shows its key hint.
- [ ] **Step 2: Run** both filters. Expected: FAIL.
- [ ] **Step 3:** Implement on craft-dialogs' list modal; max 50 rows.
- [ ] **Step 4: Run.** Expected: PASS. Commit.

### Task 14: Leader key and which-key popup

**Files:**
- Modify: `crates/codemason-gui/src/keys.rs` (`KeyRouter`), config (`ui.leader`, `ui.leader_timeout_ms`)
- Create: `crates/codemason-gui/src/dialogs/which_key.rs`
- Test: `crates/codemason-gui/tests/test_keys.rs`

**Interfaces:**
- Produces: leader default `ctrl+x`, timeout default 2000 ms; bindings may
  be written `"<leader> g"`; `KeyRouter` gains
  `pub fn pending_leader(&self) -> bool`. After 400 ms of pending leader the
  which-key popup lists every `<leader>` continuation with its title.
  Default leader bindings: `a` agent picker, `l` session picker, `m` model
  picker, `g` timeline, `u` undo, `r` redo, `e` editor, `b` toggle Sessions pane, `1`-`9` session tabs.

- [ ] **Step 1: Write failing tests:** `ctrl+x` then `m` opens the model
  picker; `ctrl+x` then nothing for 2 000 ms clears the pending state
  (drive time through the kittest harness); `ctrl+x` then an unbound key does
  nothing and clears pending; the which-key popup appears after 400 ms and
  lists `m`.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Commit.

### Task 15: Agent and session pickers; model favourites

**Files:**
- Modify: `crates/codemason-gui/src/dialogs/pickers.rs`, `dialogs/model_picker.rs`
- Test: `crates/codemason-gui/tests/test_pickers.rs`

**Interfaces:**
- Consumes: `fuzzy_rank` (Task 13).
- Produces: `pub fn fuzzy_list_ui<T>(ui, state: &mut FuzzyListState, items: &[T], label: impl Fn(&T) -> String) -> Option<usize>`
  shared by the model, agent and session pickers; config key
  `models.favorites: Vec<String>`; `Ctrl+F` in the model picker toggles a
  favourite; favourites sort first; `F2` cycles the 5 most recent models.

- [ ] **Step 1: Write failing tests** for each picker: filter narrows rows,
  Enter selects, agent picker switches `app.agent`, session picker switches
  session; favourite toggle persists to a config path under `target/temp/`.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Commit.

### Task 16: `@file#range` mentions and `/editor`

**Files:**
- Modify: `crates/codemason-gui/src/panels/prompt.rs`, the mention expander in
  app-core (where `@file` is resolved before sending)
- Create: `crates/codemason-app-core/src/external_editor.rs`
- Test: `crates/codemason-app-core/tests/test_mentions.rs`, `test_external_editor.rs`

**Interfaces:**
- Produces: `pub struct FileMention { pub path: String, pub range: Option<(u32, u32)> }`,
  `pub fn parse_mention(token: &str) -> Option<FileMention>` (`@src/a.rs#20-45`,
  `@src/a.rs#20`, `@src/a.rs`); the expander inlines only lines 20-45
  (1-based, inclusive, clamped to the file length).
  `pub fn edit_in_external_editor(text: &str, editor: &str, scratch_dir: &Path) -> anyhow::Result<String>`
  — writes `text` to a temp file in `scratch_dir`, runs `editor <file>`,
  returns the saved contents; `editor` comes from `$VISUAL` then `$EDITOR`;
  neither set → error shown in the status line.

- [ ] **Step 1: Write failing tests:** the three parse forms; `#45-20` →
  `None`; expansion of `#2-3` on a 5-line file contains only lines 2-3;
  range past EOF clamps; `edit_in_external_editor` with editor
  `sh -c 'printf edited > "$0"'` returns `"edited"`; mention popup ranks with
  `fuzzy_rank`.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement; GUI runs the
  editor off the UI thread and keeps the composer read-only meanwhile.
- [ ] **Step 4: Run.** Expected: PASS. Update docs. Commit; open the phase 4 PR.

## Phase 5 — Session ergonomics and feedback

### Task 17: Session tabs

**Files:**
- Create: `crates/codemason-gui/src/panels/session_tabs.rs`
- Modify: `panels/messages.rs` (tabs strip above the transcript), `crates/codemason-app-core/src/sessions_sidebar_state.rs`
- Test: `crates/codemason-gui/tests/test_session_tabs.rs`

**Interfaces:**
- Produces: `pub struct SessionTabs { pub open: Vec<String>, pub closed: Vec<String> }`
  on `App` (session ids; max 9 open; `closed` max 10, LIFO); actions
  `tab.next` (Ctrl+Tab), `tab.prev` (Ctrl+Shift+Tab), `tab.close`
  (`<leader> w`), `tab.reopen` (Ctrl+Shift+T), `tab.goto.1`..`9`
  (Ctrl+1..9). Each tab shows title, an `Icon` status (running spinner,
  needs-attention, error) and the agent colour chip (Task 21).

- [ ] **Step 1: Write failing tests:** opening a session from the sidebar
  adds a tab; Ctrl+2 switches to the second; closing then Ctrl+Shift+T
  restores it; a 10th open replaces the least recently used tab; deleting a
  session removes its tab.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Commit.

### Task 18: Timeline, redo, and undo-to-composer

**Files:**
- Modify: `crates/codemason-app-core/src/app/undo.rs`, `app/slash.rs`, `app/state.rs`
- Create: `crates/codemason-gui/src/dialogs/timeline.rs`
- Test: `crates/codemason-app-core/tests/test_undo_redo.rs`, `crates/codemason-gui/tests/test_timeline.rs`

**Interfaces:**
- Produces: `/redo` and `handle_redo(&mut self)`; `handle_undo` pushes the
  removed turn (messages plus the file snapshot it already restores) onto
  `App::redo_stack: Vec<UndoneTurn>` and puts the undone prompt text into the
  composer; sending a new prompt clears `redo_stack`. Timeline: a picker over
  the session's user prompts (first line, relative time) that scrolls the
  transcript to the selected message (`<leader> g`).

- [ ] **Step 1: Write failing tests:** undo then redo restores the
  transcript and stored messages byte-identically; undo puts the prompt in
  the composer; undo, send, redo → `"[warn] Nothing to redo"`; redo while
  processing is refused; timeline lists prompts newest-first and Enter
  scrolls so the target message is visible.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Commit.

### Task 19: Subagent navigation

**Files:**
- Modify: `crates/codemason-gui/src/panels/agents.rs`, `panels/messages.rs`
- Test: `crates/codemason-gui/tests/test_agent_navigation.rs`

**Interfaces:**
- Produces: `App::view_stack: Vec<String>` (session ids; top is shown);
  `fn open_child(app, child_session_id)` pushes; breadcrumb strip above the
  transcript (`Parent > child`) whose segments pop back; Esc in a child view
  pops once. Activity rows for `new_agent` calls get an "Open" button.

- [ ] **Step 1: Write failing tests:** clicking an agent row opens its
  transcript and shows a two-segment breadcrumb; clicking the first segment
  returns to the parent; Esc pops; a child that finished still opens
  read-only.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Commit.

### Task 20: Toasts and desktop notifications

**Files:**
- Create: `crates/codemason-gui/src/toasts.rs`
- Modify: `dialogs/banner.rs` (cost summary becomes a toast; delete the banner
  modal), `shell.rs`, `Cargo.toml` (`notify-rust`, Linux/macOS/Windows)
- Test: `crates/codemason-gui/tests/test_toasts.rs`

**Interfaces:**
- Produces: `pub enum ToastKind { Info, Success, Warning, Error }`,
  `pub fn push_toast(app: &mut App, kind: ToastKind, text: String)`;
  bottom-right stack, max 3 visible, auto-dismiss 5 s (errors 10 s), hover
  pauses, AccessKit live region `polite` (`assertive` for errors). Events:
  turn done, error, permission needed, question, background agent done.
  When the window is unfocused the same events raise a desktop notification
  if `ui.notifications` (default `true`). The notifier sits behind
  `trait Notifier { fn notify(&self, title: &str, body: &str); }` so tests
  inject a recording fake.

- [ ] **Step 1: Write failing tests:** a finished turn pushes a `Success`
  toast containing the cost; a 4th toast evicts the oldest; toasts vanish
  after 5 s of harness time; focused window → fake notifier records nothing;
  unfocused → one record per event; `ui.notifications = false` → none.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Commit.

### Task 21: Agent colour chips and transcript export

**Files:**
- Modify: `panels/agents.rs`, `panels/session_tabs.rs`, `panels/messages.rs`, `app/slash.rs`
- Create: `crates/codemason-app-core/src/export.rs`
- Test: `crates/codemason-app-core/tests/test_export.rs`, `crates/codemason-gui/tests/test_agents_window_table_gui.rs`

**Interfaces:**
- Consumes: `tokens.categorical`, `message_markdown` (Task 3).
- Produces: `pub fn agent_color_index(agent_name: &str) -> usize` (stable
  hash mod 8); `pub fn export_markdown(messages: &[Message]) -> String` and
  `pub fn export_html(messages: &[Message], tokens: &ThemeTokens) -> String`
  (self-contained, escaped); `/export md|html [path]` defaulting to
  `<project>/.codemason/exports/<session-short-id>.<ext>`.

- [ ] **Step 1: Write failing tests:** `agent_color_index` is stable across
  calls and in `0..8`; export markdown contains each prompt as a `## You`
  section and code fences; HTML escapes `<script>` in message text; `/export`
  with a path under `target/temp/` writes the file and reports its path.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Update docs. Commit; open the phase 5 PR.

## Phase 6 — Plain mode and accessibility

### Task 22: Plain `--no-tui` output

**Files:**
- Create: `src/plain_output.rs`
- Modify: `src/main.rs` (plain loop, lines ~1242-1282)
- Test: `tests/test_plain_output.rs` (root crate)

**Interfaces:**
- Produces: `pub struct PlainRenderer<W: Write> { .. }` with
  `fn on_event(&mut self, event: &AgentEvent) -> io::Result<()>`:
  prompt `"> "`, streamed text deltas, one line per tool call
  (`"  {tool} {summary} ({duration})"`), errors to stderr in red. Colour from
  the theme's ANSI mapping only when stdout is a TTY, `NO_COLOR` is unset and
  `--mono` is not given.

- [ ] **Step 1: Write failing tests** against a `Vec<u8>` writer: text
  deltas appear in order without buffering until the end; a tool event
  prints one line; with `mono = true` the output contains no `\x1b[`; an
  error event goes to the error writer.
- [ ] **Step 2: Run** `cargo nextest run -p codemason -E 'test(test_plain_output::)'`. Expected: FAIL.
- [ ] **Step 3:** Implement; `println!`/`eprintln!` are allowed in `src/main.rs`'s
  CLI surface, the renderer writes through its writers.
- [ ] **Step 4: Run.** Expected: PASS. Commit.

### Task 23: Font size, reduced motion and live regions

**Files:**
- Modify: `crates/codemason-gui/src/metrics.rs` (scale factor), `platform/window.rs`,
  `panels/messages.rs`, config (`ui.font_size`, `ui.reduced_motion`)
- Test: `crates/codemason-gui/tests/test_accessibility.rs`

**Interfaces:**
- Produces: `ui.font_size` in 10..=24 (default 14), Ctrl+= / Ctrl+- /
  Ctrl+0 actions `font.increase`/`font.decrease`/`font.reset` (step 1,
  clamped), persisted; `text_size::*` scale by `font_size / 14`.
  `ui.reduced_motion` (default `false`, and `true` when the OS reports it)
  replaces spinners with a static `Icon` and disables smooth scroll. The
  streaming assistant block is an AccessKit live region (`polite`),
  announced once per completed paragraph, not per delta.

- [ ] **Step 1: Write failing tests:** Ctrl+= twice → body text 16 px;
  clamps at 24; reduced motion → no `ProgressIndicator` node with an
  animating spinner while a turn runs; the live region's text changes only
  when a paragraph completes.
- [ ] **Step 2: Run.** Expected: FAIL. **Step 3:** Implement.
  **Step 4: Run.** Expected: PASS. Update docs. Commit; open the phase 6 PR.

## Verification per phase PR

1. `timeout 1500 cargo nextest run -p codemason-app-core -p codemason-gui --no-fail-fast 2>&1 | tee target/temp/phaseN-tests.log`
2. `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings`
3. `scripts/check-theme-literals.sh` and `scripts/check-test-env-isolation.sh`
4. `git status --short` after the test run shows no files outside your
   changes (tests write only to `target/temp/`).
5. Screenshots of one scripted session in `codemason-dark`,
   `codemason-light` and `high-contrast`, attached to the PR.

## Coverage of OCUIGAP

| OCUIGAP gap | Task |
|---|---|
| Markdown, syntax highlighting, code actions | 1 |
| Links, tables | 2 |
| Text selection / copy | 3 |
| Message structure, images, streaming feel | 4 |
| Tool calls / thinking collapsing, verbosity | 5, 6, 7 |
| Diffs, permission preview | 8, 9, 10 |
| Iconography | 10 |
| Command palette, keymap, which-key | 12, 13, 14 |
| Model/agent pickers, `@file#range`, external editor | 15, 16 |
| Session tabs, timeline, redo, quick switch | 14, 17, 18 |
| Child sessions | 19 |
| Toasts, notifications | 20 |
| Agent colour coding, export | 21 |
| Minimal mode / monochrome | 22 |
| Font size, reduced motion | 23 |
| Themes, light/system, high contrast, fonts, spacing, status bar, welcome, placeholder, SPEC accuracy | Done in phases 0-1 (PRs #45, #46) |

Not planned, by decision: plugin access to theme tokens (no GUI plugin API
yet), queue-vs-steer and comment-on-selected-code (feature work, tracked in
`OCGAP.md`), scroll speed settings and cursor style (minor).
