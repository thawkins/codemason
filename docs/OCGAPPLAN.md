# OCGAPPLAN: Making Codemason More Attractive to Use and Look At

Date: 2026-11-18

Plan for closing the aesthetic, UI and UX gaps in [`OCUIGAP.md`](OCUIGAP.md).
Feature gaps (web UI, HTTP API breadth, Code Mode) are out of scope; see
[`OCGAP.md`](OCGAP.md).

## Goals

1. A new user's first session looks like a finished product: readable markdown,
   highlighted code, no debug chrome, no stray banner.
2. Long agent runs stay scannable: the answer is visible, tool and reasoning
   detail is one click away.
3. Every visual property comes from one semantic theme, so light mode, user
   themes and high contrast are data, not code.
4. Every command is discoverable from one palette and every key is rebindable.

## Principles

- **Semantic first.** app-core emits meaning (`Role::ToolHeader`,
  `Span::DiffAdded`), the GUI maps meaning to theme tokens. No new literal colour
  in app-core.
- **Structured, not styled lines.** The transcript moves from `StyledLine` text
  to a block model (paragraph, code, diff, tool group, thinking). This is the
  enabling refactor for phases 2-4.
- **Keep what is ahead.** Docking, the Context pane, the session sidebar and
  AccessKit coverage stay; every new widget gets AccessKit roles and an
  egui_kittest test.
- **Fix the spec.** `SPEC.md` GUI claims are corrected in the same PR that
  delivers or drops each feature.

## Phase 0 — Quick wins (1-2 days, no architecture change)

Each item is a small, independent PR.

| # | Change | Where | Done when |
|---|---|---|---|
| 0.1 | Replace the "RAGENT" logo with a codemason wordmark; stop injecting the logo, author email and `[ok] Session created` into the transcript as assistant text. Show a proper empty-state view (wordmark, version, 3 starter actions, one tip) when the transcript is empty | `app-core/src/logo.rs`, `app/sessions.rs:507-549`, `panels/messages.rs` | New session shows an empty-state view; transcript contains no banner text |
| 0.2 | Status bar to one line: model, agent, context % meter, cost, turn state. Move service flags (`GCF/CDX/LOG/AUTO/EDIT/TELE/YOLO`) into a hover popover or the Profile pane; keep YOLO visible as a warning chip only when on | `panels/status_bar.rs` | Single line at default window width; flags reachable on hover |
| 0.3 | Turn-in-progress spinner in the status bar and on the running tool line (egui `Spinner`) | `status_bar.rs`, `panels/messages.rs` | Spinner animates while a turn runs; AccessKit `ProgressIndicator` |
| 0.4 | Remove the "Queue panel" placeholder pane or render the real queue in it | `dock.rs`, `shell.rs:976-978` | No placeholder text reachable |
| 0.5 | Fonts: bundle Inter (or another OFL sans) Regular/SemiBold for UI and JetBrains Mono (or Hack Regular/Bold) for code; stop mapping proportional bold to Hack Bold | `fonts.rs`, `assets/fonts/` | Bold prose renders in the sans family |
| 0.6 | Expose the existing high-contrast theme via `/theme high-contrast` and a config key | `app/init.rs:468`, config, slash registry | Theme switches live; test asserts palette change |
| 0.7 | Composer: drop the literal "Prompt" label, auto-grow 1-8 rows, Shift+Enter newline alongside Alt+Enter | `panels/prompt.rs:487-505` | Visual check + kittest |
| 0.8 | Remove stale tips; correct `SPEC.md:2058-2062` to match current GUI | `tips.rs`, `SPEC.md` | No tip or spec line describes a missing feature |

## Phase 1 — Theme and design tokens (about 1 week)

1. Define `ThemeTokens` in app-core (no egui types): `text.{base,muted,subtle}`,
   `bg.{base,raised1,raised2,overlay}`, `border.{base,focus}`,
   `accent`, `state.{success,warning,error,info}`, `diff.{added,removed,
   context,added_bg,removed_bg,line_number}`, `syntax.{keyword,string,
   comment,function,type,number,operator,punctuation}`, `markdown.{heading,link,
   code,blockquote}`, `categorical[8]` for agents.
2. Replace every `ThemeColor::<ansi>` use in `message_lines.rs`, `markdown.rs`,
   `message_text.rs` and `status.rs` with semantic roles. Add a lint script
   (`scripts/check-theme-literals.sh`) that fails on new literal colours outside
   the theme modules.
3. Map tokens to egui `Visuals` in `theme.rs`; remove the hardcoded widget greys
   and selection blue.
4. Ship themes as JSON in the same token shape: `codemason-dark` (default,
   off-black `#0f1115`-class background with raised surfaces),
   `codemason-light`, `high-contrast`, plus ports of 4 popular palettes with
   permissive licences (e.g. Catppuccin, Tokyo Night, Gruvbox, Nord).
5. `theme.mode = system|dark|light` following the OS via eframe; remove the
   forced `ctx.set_theme(Dark)` (`theme.rs:189`).
6. User themes from `~/.config/codemason/themes/*.json` and
   `.codemason/themes/*.json`; project wins. Validate contrast with the existing
   WCAG helpers and warn in the Log pane on failures.
7. Spacing and shape constants: a 4 px spacing scale (`space.xs..xl`), radius
   scale (`radius.sm=4, md=6, lg=10`), type scale (12/14/16/20). Replace the
   ad-hoc `add_space` calls in dialogs.
8. Theme picker dialog with live preview (`/theme`, palette entry).

Done when: no literal colour remains outside theme modules (script passes),
light and dark both pass the contrast check, theme switch is live.

## Phase 2 — Transcript block model and rich rendering (about 2 weeks)

1. **Block model.** app-core produces `TranscriptBlock` values
   (`Paragraph(inline spans)`, `Heading`, `List`, `Table`, `CodeBlock{lang,
   text}`, `Quote`, `Image`, `ToolGroup`, `Thinking`, `Diff`) instead of
   `StyledLine`s. Parse assistant markdown incrementally with pulldown-cmark
   (already a dependency); re-parse only the trailing open block while
   streaming.
2. **Markdown widgets.** Headings on the type scale, lists, blockquotes,
   tables (egui `Grid`), clickable links (open in browser), inline code chips.
   Evaluate `egui_commonmark` against the forked egui 0.37 first; build in-house
   only if it does not fit.
3. **Syntax highlighting.** `syntect` (or `tree-sitter-highlight`, reusing
   grammars from `codemason-codeindex`) mapped to `syntax.*` tokens; cache
   highlighted blocks by content hash.
4. **Code block chrome.** Language label, Copy button, wrap toggle; "Insert into
   prompt" action.
5. **Selection and copy.** Selectable text in all blocks; enable the existing
   disabled context-menu items (Copy, Copy as Markdown, Quote in reply).
6. **Message framing.** User turns as a subtle raised card with the role chip;
   assistant turns unframed with a left accent rule; consistent vertical rhythm
   from the spacing scale. Drop the `You:` / `*` prefixes.
7. **Images.** Inline thumbnails for attachments and tool-result images,
   click to enlarge.
8. **Streaming caret** at the end of the live block.

Done when: a response with headings, a table, a link and two fenced code blocks
renders correctly and is selectable; kittest snapshot tests cover each block.

## Phase 3 — Quiet transcripts: activity rows, thinking, diffs (about 1.5 weeks)

1. **Activity rows.** Consecutive tool calls and reasoning in a turn collapse
   into one row: spinner or status icon, "Read 4 files, edited 2, ran tests
   (12 s)", expand chevron. Expanded shows each call with its argument summary,
   duration and result preview.
2. **Verbosity setting** `low|medium|high` (`/verbosity`, palette, config):
   low = activity rows collapsed, medium = per-tool lines, high = full output.
3. **Thinking blocks.** Labelled "Thinking" header, collapsed by default after
   the turn ends, expanded while streaming; `/thinking show|hide`.
4. **Diff viewer.** Edits render an inline unified diff with `diff.*` tokens
   and line numbers, collapsed to the first hunk; "Open diff" opens a new
   **Diff pane** (dockable) with unified/split toggle, file list, hunk
   navigation (`]`/`[`) and mark-reviewed.
5. **Permission dialog** shows the diff or command preview for edit and bash
   permissions.
6. **Icons.** Bundle a small icon font (e.g. Lucide or Phosphor, permissive
   licence) for tool kinds, status and chevrons; keep ASCII fallback for missing
   glyphs (`styled_text.rs:148-177` pattern).

Done when: a 30-tool-call turn fits in one screen at default verbosity, and an
edit shows its diff inline.

## Phase 4 — Discoverability and keyboard (about 1 week)

1. **Command palette** (Ctrl+Shift+P and Ctrl+P): fuzzy search over slash
   commands, pane toggles, themes, models, agents and sessions, with key hints.
2. **Action registry.** Every GUI and app-core action gets a stable ID; all are
   rebindable in `gui-keys.json` (extend `keys.rs`), not just 6.
3. **Leader key** (default Ctrl+X, timeout configurable) for session and pane
   actions, plus a which-key popup listing continuations.
4. **Pickers** for agents (`<leader>a`) and sessions (`<leader>l`), sharing one
   fuzzy-list widget with the model picker; model favourites and recent-cycle.
5. **`@file#20-45`** line-range mentions and fuzzy ranking in the mention popup.
6. **External editor** (`/editor`): open `$VISUAL`/`$EDITOR` on the composer
   text.
7. **Shortcuts settings page** listing and editing bindings.

Done when: every slash command and pane toggle is reachable from the palette,
and a kittest test rebinds an arbitrary action.

## Phase 5 — Session ergonomics and feedback (about 1.5 weeks)

1. **Session tabs** above the transcript (Ctrl+1..9, Ctrl+Tab, reopen closed),
   with status indicators (running, needs attention, error).
2. **Timeline / jump to message** (`<leader>g`): list of user prompts, jump on
   select.
3. **Redo** to pair with `/undo`; undo returns the reverted prompt to the
   composer for editing.
4. **Subagent navigation**: open a child agent's transcript from the Active
   agents pane or its activity row, breadcrumb back to the parent.
5. **Toasts** (bottom-right, auto-dismiss, AccessKit live region) for
   completion, errors, background agent done; replace the cost banner.
6. **Desktop notifications and optional sound** when the window is unfocused,
   per event type (permission, question, error, done, subagent done).
7. **Export transcript** to Markdown/HTML.
8. **Agent colour chips** from `categorical` tokens in tabs, activity rows and
   the Active agents pane.

## Phase 6 — Plain mode and accessibility (about 3 days)

1. `--no-tui`: prompt string, streamed text, one-line tool summaries, ANSI
   colours from the theme's ANSI mapping, `NO_COLOR` and `--mono` honoured,
   errors to stderr (CLI presentation surface allows `eprintln!`).
2. Font size setting (`ui.font_size`, Ctrl+= / Ctrl+-) separate from window
   zoom.
3. Reduced-motion setting disabling spinners and smooth scroll.
4. AccessKit live region for streamed assistant text and toasts.

## Sequencing and effort

| Phase | Effort | Depends on | User-visible payoff |
|---|---|---|---|
| 0 Quick wins | 1-2 days | — | Removes "debug build" impression |
| 1 Theme tokens | ~1 week | 0.5, 0.6 | Light mode, themes, consistent look |
| 2 Block model + rich rendering | ~2 weeks | 1 | Markdown, highlighting, copy |
| 3 Activity rows + diffs | ~1.5 weeks | 2 | Readable long runs, visible edits |
| 4 Palette + keymap | ~1 week | — (parallel with 2-3) | Discoverability |
| 5 Session ergonomics | ~1.5 weeks | 2, 4 | Power-user flow |
| 6 Plain mode + a11y | ~3 days | 1 | Terminal users, comfort |

Total: about 8 weeks for one developer; phases 4 and 6 can run in parallel with
2-3, bringing it to about 6 weeks with two.

## Verification

- Every phase adds egui_kittest tests (AccessKit queries) for new widgets, under
  `crates/codemason-gui/tests/`, registered in `tests/all.rs`.
- Add image snapshot tests for the transcript (dark, light, high-contrast) so
  visual regressions are caught; snapshots live under `target/temp/` for
  diffs and a checked-in baseline directory updated only by an explicit flag.
- `scripts/check-theme-literals.sh` in CI from phase 1 onward.
- A before/after screenshot set of the same scripted session attached to each
  phase PR.

## Risks

| Risk | Mitigation |
|---|---|
| Forked egui 0.37 (storytold) may not support `egui_commonmark` or new text APIs | Spike in phase 2 step 2 before committing; fall back to an in-house renderer over pulldown-cmark |
| Block model changes app-core's transcript contract used by tests | Keep a `StyledLine` adapter for the Log pane and existing tests during migration |
| Re-parsing markdown while streaming costs CPU on long messages | Parse only the open trailing block; keep the 33 ms throttle |
| Font and icon licences | Use OFL/MIT/ISC assets only; record them in `NOTICE` |
| Theme ports carry licences | Port only MIT/permissive palettes, credit in theme files |
