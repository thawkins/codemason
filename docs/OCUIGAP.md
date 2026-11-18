# OCUIGAP: Codemason vs opencode v2 — Aesthetic, UI and UX Gap Analysis

Date: 2026-11-18

Companion to [`OCGAP.md`](OCGAP.md) (feature gaps). This document covers only how
the product looks and feels to use. The remediation plan is
[`OCGAPPLAN.md`](OCGAPPLAN.md).

## Scope and sources

- **Codemason**: branch `fix/bug-batch` at `454bfb2`. Evidence is the source of
  `crates/codemason-gui` (egui/eframe 0.37 with craft-ui, craft-keymap,
  craft-dialogs) and `crates/codemason-app-core`, which builds every transcript
  line. File:line references are to that tree. The `--no-tui` plain mode
  (`src/main.rs`) is covered briefly.
- **opencode v2**: the V2 CLI/TUI docs (`https://opencode.ai/docs/cli/tui/`,
  `/docs/cli/config/`, `/docs/cli/keybinds/`, `/docs/cli/theme/`,
  `/docs/cli/web/`, `/docs/themes/`), the landing page and the changelog to
  v2.3.2. No opencode build was run and no screenshots were compared, so its
  rendering details (fonts, header, toasts, desktop look) are **UNVERIFIED**
  where the docs are silent.
- Codemason's comparison point is its egui GUI, since the TUI was removed. Where
  opencode's terminal UI is the reference, the gap is judged on the experience
  delivered, not on the rendering technology.

Legend: **Yes** = present, **Partial** = present with a narrower scope,
**No** = not found, **?** = UNVERIFIED.

## Summary

Codemason's GUI is a terminal UI painted with egui. The transcript is a single
block of ANSI-coloured text per message: no markdown, no syntax highlighting, no
diff view, no collapsing, no text selection. The palette is xterm ANSI on pure
black, there is one dark theme (a high-contrast theme exists but cannot be
selected), and the status bar is two lines of service flags. The structure
underneath is strong: 11 dockable panes, a session sidebar with pins and
sections, AccessKit labels throughout, and a virtualised transcript.

opencode v2 has invested in presentation: 33 themes on a semantic token system
with light/dark/system modes, rendered markdown, split/unified diffs with a
review workflow, collapsible activity rows that keep transcripts quiet, a
command palette, a leader-key keymap where every action is rebindable, and
first-class session navigation (tabs, timeline, child sessions, undo/redo).

### Top 5 aesthetic/UX gaps in Codemason

1. **Transcript rendering**: no markdown, syntax highlighting, diffs, clickable
   links, image previews, collapsible tool or thinking blocks, or text
   selection/copy.
2. **Theming**: one usable dark theme on raw xterm ANSI colours; no light mode,
   no system-follow, no user themes, no semantic tokens in app-core.
3. **Visual polish**: debug-style status bar, ASCII markers (`[ok]`, `[x]`,
   `└`), monospace bold mixed into proportional text, magic-number spacing,
   welcome banner that spells "RAGENT".
4. **Discoverability**: no command palette, no which-key, only 6 rebindable
   keys, no visual agent picker.
5. **Session ergonomics**: no session tabs, timeline/jump-to-message, child
   session navigation, redo, or share/export.

### Where Codemason is already ahead or level

1. Dockable, persistent multi-pane layout (11 panes) — opencode's desktop app is
   tab/side-panel based; a docking layout is not documented.
2. Context pane with a per-partition token breakdown (system prompt, tools,
   history) — opencode documents only per-turn token debug output.
3. AccessKit roles and labels across panels and dialogs, tested with
   egui_kittest — opencode documents no screen-reader support.
4. Session sidebar with pin, sections, attention marks, multi-select and a
   context menu — comparable to opencode's pin/rename/unread.
5. WCAG contrast helpers already exist (`crates/codemason-gui/src/theme.rs:26-69`).

## 1. Visual identity and theming

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Built-in themes | 1 usable (`Default`, dark) (`theme_mode.rs:9-15`) | 33 (opencode, tokyonight, catppuccin, gruvbox, nord, ...) | Codemason |
| Light mode | No (dark forced, `theme.rs:189`) | Yes (`mode: light`) | Codemason |
| Follow OS/terminal | No | Yes (`mode: system`, plus a `system` theme read from the terminal palette) | Codemason |
| High-contrast theme | Partial (implemented, unreachable: only `Default` is assigned, `app/init.rs:468`) | ? | Codemason (wiring) |
| User/project themes | No | Yes (`~/.config/opencode/themes/*.json`, `.opencode/themes/`) | Codemason |
| Token model | 9 colours + 16 ANSI (`palette.rs:53-74`); app-core uses literal ANSI names (`message_lines.rs`, `markdown.rs`) | Semantic groups: text, background (raised levels), border, diff, syntax, markdown, states, categorical hues | Codemason |
| Hardcoded colours | Widget greys and selection blue in `theme.rs:85-98,133-145`; thinking `Rgb(180,180,220)` in `message_text.rs:35` | Theme references (`$hue.blue.500`) | Codemason |
| Background | Pure black `#000000` (`palette.rs:167`) | Theme-defined, layered `raised` surfaces | Codemason |
| Fonts | egui defaults (Ubuntu-Light, Hack) + Hack Bold for *all* bold, so bold prose is monospace (`fonts.rs:65-79`) | Terminal font (TUI); desktop ? | Codemason |
| Type scale | Single 14 px (`styled_text.rs:21`) | Terminal (n/a); desktop ? | Codemason |
| Spacing system | None; ~25 ad-hoc `add_space(4/6)` calls | ? | Codemason |
| Iconography | ASCII/Unicode glyphs (`● ✗ ◆ ℹ ⚠`, `[ok]`, `[attach]`) | Tab status icons; desktop ? | Partial |
| Agent colour coding | No | Yes (`base.categorical`, per-agent `color`) | Codemason |
| Plugin access to theme | No | Yes (`context.theme`, `themeMode`) | Codemason |

## 2. Layout and chrome

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Pane system | Yes: 11 dockable, tabbable, floatable panes, saved to `gui-layout.json` (`dock.rs:46-69,454-572`) | Sidebar (auto/hide), tabs (horizontal/vertical), desktop side panel | Codemason ahead |
| Sidebar toggle | Via docking only | `<leader>b` | Partial |
| Session tabs | No | Yes (Ctrl+1..9, Ctrl+Tab, reopen closed tab) | Codemason |
| File tree / diff / terminal panes | No | Desktop: diffs, persistent terminals, built-in browser; TUI `/diff` viewer with file tree | Codemason |
| Status bar | Two wrapping lines of version, path, branch, backend, model, flags `GCF/CDX/LOG/AUTO/EDIT/TELE/YOLO [ok]/[x]` (`status_bar.rs:98-286`) — reads as debug output | ? (footer contents not documented; `mini.footer` shows activity, model, usage, context) | Codemason |
| Context/cost in footer | No (`status_bar.rs:17-20`; SPEC claims it) | `mini.footer`; tokens/sec via `session.tps` | Codemason |
| Placeholder content | "Queue panel" text-only pane (`shell.rs:976-978`) | — | Codemason |
| Menu bar | File menu only (3 items) | ? | Partial |
| Minimal mode | `--no-tui`: bare stdin/stdout, final text only, no colour or streaming (`src/main.rs:1242-1282`) | `opencode mini` with spinners, footer, turn summary, mono option | Codemason |

## 3. Transcript and content rendering

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Message structure | One text block per message, separated by blank lines; `You:` / `*` prefixes (`message_lines.rs:109`) | Turns with grouped activity rows | Codemason |
| Markdown | No in live chat (raw lines, `message_lines.rs:125-145`); a pulldown-cmark ASCII renderer exists for `/research open` only | Yes (`session.markdown: rendered`, concealed markers) | Codemason |
| Syntax highlighting | No (no syntect/tree-sitter highlighter in GUI or app-core) — `SPEC.md:2058` claims it | Yes (`syntax` theme group) | Codemason |
| Tables | Parsed, not drawn | ? | Codemason |
| Links | Plain `[text](url)`, not clickable (`markdown.rs:165-172`) | ? | Codemason |
| Diffs | `(+N -M)` counts only (`message_lines.rs:224-245`) | Split/unified/auto, word wrap, file tree, hunk nav, mark-reviewed | Codemason |
| Tool calls | One coloured line + `└ result` line; not collapsible; no running spinner | Collapsed into one activity row by default (v2.3.0); verbosity setting expands | Codemason |
| Reasoning / thinking | Always fully shown, lavender italic, unlabelled | `show`/`hide`, `/thinking` toggle, collapsed in activity rows | Codemason |
| Images | `[attach] [image: name]` text | Inline preview (`session.image_preview`) | Codemason |
| Code block actions | No | Yes (copy etc., v2.2.6) | Codemason |
| Comment on selected code | No | Yes (desktop, v2.3.1) | Codemason |
| Text selection / copy | No ("The transcript has no text selection", `shell.rs:968-970`); context menu items disabled | Native terminal selection or `terminal.copy: select` | Codemason |
| Streaming feel | Whole-message re-render, 33 ms throttle, no cursor | ? | Partial |
| Scrolling | Virtualised, sticky bottom, PageUp/Down | Configurable speed/acceleration, `submit_scroll`, optional scrollbar | Partial |

## 4. Input and command surface

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Composer | 1-row `TextEdit` with a literal "Prompt" label and `NN>` counter (`prompt.rs:487-505`) | Composer with readline keys, stash, paste-compact | Partial |
| Newline | Alt+Enter | Shift/Ctrl/Alt+Enter, Ctrl+J | Partial |
| `@file` mentions | Yes (plain popup, 8 rows) | Yes, fuzzy, with `#20-45` line ranges | Partial |
| Slash completion | Yes (88 commands, plain list) | Yes, filtered as you type | Level |
| Command palette | No | Ctrl+P, context-aware | Codemason |
| Keymap | 6 rebindable GUI commands (`keys.rs:11-31`); rest hardcoded Alt chords | Every action has an ID and is rebindable; leader key (Ctrl+X) with timeout | Codemason |
| Which-key help | Static `?` overlay | Which-key panel (Ctrl+Alt+K) | Partial |
| External editor | No | `/editor`, Ctrl+X E | Codemason |
| Queue vs steer | Queue only | Enter steers, leader+Enter queues; queue list with undo/delete | Partial |
| Image paste | Yes (Ctrl+V/Alt+V; filenames listed) | Yes, with image viewer `<leader>i` | Partial |
| Model picker | Modal list with filter | `<leader>m`, favourites, recent-cycle F2, usage view | Partial |
| Agent picker | Tab cycles; no visual picker | `<leader>a`, Shift+Tab cycle | Partial |

## 5. Feedback, status and safety UX

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Turn-in-progress indicator | Status text turns yellow; no spinner (`status_bar.rs:124-133`) | Work spinner (13 styles in mini) | Codemason |
| Toasts | No (cost banner only, `dialogs/banner.rs`) | ? | ? |
| Attention / notifications | Sidebar attention marks | Desktop notifications + sound packs per event (permission, question, error, done, subagent_done) | Codemason |
| Permission dialog | Modal: name, first pattern, countdown, Allow/Always/Deny (`dialogs/permission.rs:50-86`); no diff preview | Fullscreen toggle (Ctrl+F), `autoaccept` mode; layout ? | Partial |
| Context meter | Context pane with 10-cell bars (`panels/context.rs`) | Footer usage/context (mini), `compaction.buffer` | Codemason ahead on depth, behind on glanceability |
| Empty / welcome state | ASCII logo (spells "RAGENT", `logo.rs:9-14`), author email, `[ok] Session created` injected as assistant text into every session (`app/sessions.rs:507-549`) | Splash (mini `splash`) | Codemason |
| Tips | Yes, some stale ("status bar shows token usage") | ? | Codemason |
| Error display | Red text in transcript/status/Log | ? | ? |

## 6. Session navigation

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Undo | `/undo` drops last turn pair | `/undo` reverts files too, prompt returns to composer for editing | Partial |
| Redo | No | `/redo`, `<leader>r` | Codemason |
| Timeline / jump to message | No | `<leader>g` timeline; desktop Cmd/Ctrl+Alt+G | Codemason |
| Child (subagent) sessions | Active agents pane | Navigate into children with arrows, back to parent | Partial |
| Quick switch | No | `<leader>1..9` slots | Codemason |
| Share / export transcript | No | Share UI ? (share page 404) | ? |
| Read/unread | Yes | Yes (v2.2.7) | Level |

## 7. Accessibility and comfort

| Aspect | Codemason | opencode v2 | Gap |
|---|---|---|---|
| Screen reader | Yes (AccessKit roles across panels/dialogs, tested) | ? | Codemason ahead |
| High contrast | Built, unreachable | ? | Wiring |
| Font size setting | No (egui zoom only, persisted) | Terminal-controlled | Codemason |
| Reduced motion | n/a (little motion) | `animations` toggle | Level |
| Monochrome mode | No | `mini.mono` | Codemason |
| Cursor style | egui default | `cursor.style`, blink | Minor |

## 8. Accuracy issues in Codemason's own docs

`SPEC.md:2058-2062` describes the GUI as having "markdown and syntax
highlighting", context usage in the status bar and a "Teammate strip". None of
these exist in `crates/codemason-gui`. Either the plan delivers them or the spec
is corrected.

## 9. Gap ranking by user impact

| Rank | Gap | Why it matters |
|---|---|---|
| 1 | Markdown + syntax highlighting + selection in transcript | Every response is read here; raw markdown and uncopyable text are the first thing a new user notices |
| 2 | Theme system (tokens, light/dark/system, more themes) | Pure black + xterm ANSI is the dominant aesthetic impression; tokens unblock all later visual work |
| 3 | Collapsible tool/thinking activity rows | Long agent runs bury the answer under tool and reasoning lines |
| 4 | Diff rendering for edits | Users cannot see what an edit changed without leaving the app |
| 5 | Chrome cleanup (status bar, welcome, placeholder pane, fonts) | Cheap fixes that remove the "debug build" impression |
| 6 | Command palette + full keymap | Discoverability of 88 commands and 11 panes |
| 7 | Progress/notification feedback | Users cannot tell at a glance that a turn is running |
| 8 | Session navigation (tabs, timeline, redo) | Matters to heavy users more than first impressions |
