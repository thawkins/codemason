# NEWUI: a word-processor look for the codemason GUI, built from Material Design 3

Status: plan, not started. Scope: `crates/codemason-gui` (egui 0.37 fork) and the
theme tokens in `crates/codemason-app-core`. The TUI and headless modes are
untouched.

## 1. Goal

Make the desktop window read like a classic desktop word processor (the visual
language popularised by Microsoft Word), while every control is a Material
Design 3 (M3) component:

| Word-style trait | Where it lands in codemason | M3 component used |
| --- | --- | --- |
| Title bar + Quick Access Toolbar | Top strip: app name, session title, Save/Undo-style quick actions (New session, Copy, Stop turn) | Small top app bar + standard icon buttons |
| Ribbon: tabs over grouped command bands | Replaces the in-window `File` menu bar (`egui::MenuBar` in `shell.rs`) | Primary tabs + grouped buttons, segmented buttons, menus |
| Page on a grey "desk" | The transcript (`PaneId::Messages`) renders as a centred white page with a drop shadow on a neutral canvas | Elevated card (level 1) on `surface-container` |
| Navigation pane on the left | Sessions sidebar (`PaneId::Sessions`) | Navigation drawer (standard, not modal) |
| Task panes on the right | Tasks, Memory, Context, Agents, Teams, Queue | Side sheets (standard) |
| Status bar: page/word count, view buttons, zoom slider | `panels/status_bar.rs` | Bottom bar text, icon buttons, slider |
| Dialogs with OK/Cancel at bottom right | `dialogs/*` | Basic dialog, text + filled buttons |
| Accent blue on white | Theme tokens | M3 colour roles from a single blue seed |

Non-goals: no new features, no change to keyboard behaviour (`keys.rs`, the
keymap), no change to app-core logic, no new panes.

## 2. Constraints and decisions

1. **No Microsoft branding.** No logos, product names, the Word icon, or
   proprietary fonts (Aptos, Segoe UI, Calibri). Colours are our own seed, not
   copied brand hex values. The aim is the layout grammar (ribbon, page, desk,
   status bar), not a clone.
2. **Material components are built in-house on egui.** There is no first-party
   Material library for egui, and the workspace pins forked egui/eframe 0.37
   (`Cargo.toml` `[patch]`), so a third-party crate such as `egui-material3`
   would need forking too. A small `material` module owned by this crate is
   less code to keep in sync. Decision point: revisit if a maintained crate
   tracks egui 0.37 exactly.
3. **Tokens stay the single source of colour.** `ThemeTokens`
   (`crates/codemason-app-core/src/theme_tokens.rs`) gains M3 roles; all eight
   bundled themes keep working (dark themes get the dark M3 scheme).
4. **Assets (licences OK for bundling):**
   - Icons: Material Symbols Rounded (Apache 2.0) replaces Lucide in
     `icons.rs`; the `Icon` enum and its ASCII fallbacks stay.
   - Type: Roboto Flex (OFL 1.1) for UI text per the M3 type scale; Hack stays
     for code. Inter is removed only after Phase 2 is verified.
5. **Accessibility gates stay green:** `contrast_ratio` checks at
   `AA_CONTRAST` (text) and `UI_CONTRAST` (controls) for every role pair.
6. Tests follow AGENTS.md: new files under `crates/codemason-gui/tests/`, one
   `#[path]` line each in `tests/all.rs`, no writes into the repo
   (`target/temp/` for any rendered snapshots).

## 3. Phases

Each phase ends with `cargo fmt --check`, `cargo clippy -p codemason-gui`, and
`cargo nextest run -p codemason-gui`. Estimates are for one engineer familiar
with egui.

### Phase 1: M3 design tokens (1 to 2 days) - DONE

Done as planned, with these differences:
- The `document-*` schemes are generated with HCT tones approximated in
  CIE L*Ch (tone = L*), not the exact CAM16-based HCT; the values are fixed
  in the theme JSON.
- The M3 contrast check runs only on a theme file's own `m3` block; derived
  roles are covered by a test over the built-in themes instead, so a bad
  token is not reported twice.
- Only the three existing radii changed (SM 8, MD 12, LG 16); the XS 4 and
  XL 28 radii are added in Phase 3, where they are first used.

1. Add `M3Roles` to `theme_tokens.rs`: `primary`, `on_primary`,
   `primary_container`, `on_primary_container`, `secondary(+on/container)`,
   `tertiary(+on/container)`, `error(+on/container)`, `surface`, `on_surface`,
   `on_surface_variant`, `surface_container_{lowest,low,,high,highest}`,
   `outline`, `outline_variant`, `inverse_surface`, `scrim`, `shadow`.
2. Make the new block optional in theme JSON; when absent, derive it from the
   existing tokens (`accent` -> primary family, `bg.*` -> surface containers,
   `border.*` -> outline roles) so third-party theme files still load.
3. Add a new built-in theme pair `document-light` / `document-dark` generated
   from one blue seed with the M3 tonal-palette algorithm (HCT tones
   40/90/10/100 for light, 80/30/90/20 for dark). Both are opt-in through
   `/theme`; the default theme does not change.
4. Replace `metrics.rs` scales with M3 values: spacing stays on the 4 pt grid;
   radius scale becomes `XS 4 / SM 8 / MD 12 / LG 16 / XL 28`; text sizes move to
   the M3 type scale (Label S 11, Body M 14, Title M 16, Title L 22,
   Headline S 24).
   - Verify: a token test asserting every on-X / X pair meets `AA_CONTRAST`
     for all bundled themes.

### Phase 2: fonts and icons (1 day) - DONE

Done as planned, with these differences:
- Roboto Flex ships as two static instances (wght 400 and 500, all other
  axes at their defaults) cut with fonttools, not the variable font.
- Inter is not removed: it follows Roboto Flex in the proportional and bold
  families, because Roboto Flex lacks the U+2717 cross, the U+26A0 warning
  sign and much of Greek and Cyrillic.
- The Material Symbols font is a 3 KB subset of the 12 icons in use; the
  rebuild command is in `assets/fonts/MaterialSymbols-LICENSE.txt`. Later
  phases add their icons to that command.

1. Bundle `RobotoFlex` and `MaterialSymbolsRounded` under
   `crates/codemason-gui/assets/fonts/` with their licence files; add both to
   `THIRD-PARTY-NOTICES.txt`.
2. `fonts.rs`: Roboto Flex leads the proportional family, a 500-weight
   instance leads `BOLD_FAMILY`; keep the Hack and system-font fallbacks.
3. `icons.rs`: remap every `Icon::glyph` to its Material Symbols code point;
   keep the private family so model text never falls back to it.
   - Verify: existing icon/font tests pass; a test that every `Icon` has a
     glyph present in the bundled face.

### Phase 3: `material` component module (3 to 4 days) - DONE

Done as planned, with these differences:
- `visuals_for` maps egui's widgets onto the M3 roles only for themes with
  their own `m3` block (`document-*`), so the default and ported themes look
  exactly as before.
- Two roles were added for the snackbar and tooltip: `inverse_on_surface`
  and `inverse_primary` (required in an `m3` block, derived otherwise).
- The tooltip is drawn as its own `Area` (`plain_tooltip`) rather than
  restyling egui's hover popup, whose frame cannot be changed per call.
- `examples/material_gallery.rs` shows every component for review.

New module `crates/codemason-gui/src/material/` with one file per component.
Each takes `&Palette` and returns an `egui::Response`; no state beyond what
egui keeps.

| Component | File | Notes |
| --- | --- | --- |
| Buttons: filled, tonal, outlined, text | `button.rs` | 40 pt high, full radius, state layers (hover 8%, focus/press 10% of on-colour) |
| Icon button: standard, filled, toggle | `icon_button.rs` | 40 pt target, 24 pt glyph; `icon_button_dense` is the 32 pt toolbar variant (20 pt glyph) for a strip of controls over content, e.g. the Diff pane's one row in its ~270 pt width; `icon_button_inline` is an 18 pt variant for a control sharing a line with text, e.g. the prompt row's Scroll to bottom and Collapse all buttons |
| Segmented button | `segmented.rs` | Used for ribbon view toggles |
| Tabs (primary) | `tabs.rs` | 3 pt active indicator under label |
| Menu | `menu.rs` | `surface_container`, elevation 2, 48 pt rows |
| Card (elevated/filled/outlined) | `card.rs` | Elevation via layered soft shadows |
| Text field (outlined) | `text_field.rs` | Wraps `TextEdit`; label, supporting text, error state |
| Dialog | `dialog.rs` | 28 pt radius, `surface_container_high`, actions bottom-right |
| Navigation drawer / side sheet frame | `sheet.rs` | Frames for left and right panes |
| Chip (assist/filter) | `chip.rs` | Replaces ad-hoc chips in panels |
| Slider, linear progress, tooltip, snackbar | `feedback.rs` | Snackbar replaces `dialogs/banner.rs` styling |

`theme.rs::visuals_for` maps egui's default widgets onto the same roles, so any
widget not yet migrated already looks M3-adjacent.
- Verify: one kittest file per component checking size, state colours, and
  keyboard focus ring visibility.

### Phase 4: window chrome, the Word-style frame (3 to 4 days) - DONE

Done, with these differences:
- The frame is drawn only for themes with their own Material roles (the
  `document-*` themes); every other theme keeps the `File` menu bar,
  following the decision that the new look is opt-in through `/theme`.
- The title strip shows `codemason - <project folder>`, not the session
  title: the session title is only read from the database, too costly per
  frame. Its quick access buttons are New session, Undo, Copy transcript
  (`/clip`), Stop (Esc) and Resume (`/resume`).
- File is an ordinary ribbon tab (project folders, About, Quit), not a
  full-window backstage sheet.
- The prompt keeps its own editor (completion, history, attachments) inside
  an outlined frame; it is not replaced by `material::text_field`.
- The collapsed state is not persisted yet; it resets on restart.
- craft-libs: the ribbon's tab strip and `material::tabs` use
  `craft_ui::tabs` (`Tab`, `fit_whole`).
- Follow-up (owner's decision): the title strip is the real window title
  bar on every theme. `craft_window::policy::decide_for` picks the frame at
  startup (`CODEMASON_TITLEBAR=system` opts out), `craft_window::title_bar`
  makes the strip's empty area move the window, and `title_bar.rs` draws
  the caption buttons in the desktop's layout and resizes the undecorated
  window from its edges.

1. **Title strip** (`shell.rs`, replaces `egui::Panel::top("menu_bar")`):
   small top app bar showing `codemason - <session title>` and a Quick Access
   Toolbar of icon buttons (New session, Copy last reply, Stop turn, Undo
   edit). On macOS the native menu (`platform/menu.rs`) stays; the strip
   shows only the toolbar.
2. **Ribbon** (new `ribbon.rs`): primary tabs with grouped bands. Each band
   is a labelled group (label in Label S under the group, thin
   `outline_variant` dividers between groups). Every button dispatches an
   existing slash command or keymap chord, the same path the menu and
   `keys::command_key_input` use, so no new behaviour:
   - **Home**: Session (New, Resume, Rename), Clipboard (Copy, Paste
     attachment), Turn (Send, Stop, Queue).
   - **Insert**: Attach file, Attach image, Mention file, Snippet.
   - **Agent**: Provider, Model picker, Agent profile, Permissions mode, Loop,
     Resume.
   - **View**: Pane toggles as toggle icon buttons (Sessions, Log, Tasks,
     Memory, Context, Agents, Teams, Queue), Theme, Zoom.
   - **Review**: Diff, Context breakdown, Usage.
   The `File` tab is a backstage-style full-window sheet listing the current
   `File` menu items (`platform/files.rs`). The ribbon collapses to tabs-only
   with `Ctrl+F1`-style toggle (pick a chord free in the keymap; document it
   in `GUI-QUICKSTART.md`).
3. **Desk and page**: `messages_body` in `shell.rs` draws the central dock
   area on `surface_container_low`; the transcript sits in an elevated card
   capped at a readable width (about 816 pt, Letter width at 96 dpi),
   centred, with page-like inner margins (`space::XL` horizontally) and a
   level-1 shadow. Messages remain one scrolling page (no pagination).
4. **Prompt**: `panels/prompt.rs` becomes an outlined text field anchored to
   the bottom of the page, same width as the page.
   - Verify: kittest layout tests for page width clamp, ribbon tab switching,
     and that every ribbon button maps to an existing command (table-driven).

### Phase 5: panes, status bar and dialogs (3 days) - DONE

Done for the `document-*` themes, with these differences:
- Panes are restyled through craft-ui's `DockStyle` (sheet colours, tonal
  tabs, 32 pt tab strip) rather than wrapped in a second header: the dock's
  own tab already carries the title and the close button.
- The sessions sidebar keeps its own rows (pinning, search, project groups)
  and gets the drawer look from `material::sheet::drawer_style`.
- The status bar gains the zoom slider; no "focus mode" or "split diff"
  buttons were added, as the window has no such commands.
- Dialogs are not rebuilt: the Phase 3 visuals already give egui's windows
  the M3 dialog surface, 28 pt corners and shadow, and their buttons the
  tonal style.
   - Superseded: every dialog was rebuilt on the Material components
     afterwards. `docs/plans/MATERIALDIALOGS.md` converts the remaining
     twenty-two `modal()` call sites, the eight non-modal popup surfaces and
     the three in-panel popups, and retires the plain helper; the sweep test
     `tests/test_material_dialogs.rs` fails if a dialog is drawn without a
     Material frame.
- The slider holds a new zoom until the pointer is released: applying it
  while dragging rescales the slider under the pointer.

1. **Left drawer**: Sessions pane framed as a standard navigation drawer
   (`surface_container_low`, active row as `secondary_container` pill).
2. **Right side sheets**: Tasks, Memory, Context, Agents, Teams, Queue, Log,
   Profile use the side-sheet frame with a header (Title M + close icon
   button). Dock behaviour in `dock.rs` is unchanged; only the frame and the
   tab strip style change (tab strip uses M3 secondary tabs).
3. **Status bar** (`panels/status_bar.rs`): left side reads like a word
   processor's counters (model, tokens used / context window, message count,
   cost); right side holds view icon buttons (focus mode, split diff) and an
   M3 slider bound to the existing zoom factor.
4. **Dialogs** (`dialogs/*`): rebuild frames on `material::dialog`; confirm,
   permission, question and pickers keep their content and key handling.
   Lists in pickers use M3 list items (56 pt two-line rows).
5. **Rows and chips** (`panels/rows.rs`, code blocks, diff): code blocks are
   filled cards on `surface_container_highest`; diff colours keep their own
   tokens.
   - Verify: the existing 76 GUI test files pass unchanged in behaviour; update
     only assertions that read colours or sizes, each with a reason.

### Phase 6: polish, defaults and docs (1 to 2 days) - DONE

Done, with these differences:
- The default theme is unchanged (owner's decision); step 1 needed no code.
- Motion: the ribbon band slides open and shut and the tab underline
  glides (200 ms, cubic ease-out, `material::motion_time`); the snackbar
  and tooltip use egui's own `Area` fade-in. The side panels are opened by
  the dock, which has no open animation, so they do not slide. There is no
  reduced-motion setting in the app; egui's `animation_time` of zero
  turns the motion off.
- AAA check: two derived roles of `high-contrast` missed 7:1 (`on_primary`
  6.81, `on_error` 5.75). The theme's accent and error colours were lifted
  to `#1a9fff` and `#ff6060`, which also raises them as text on black.
- Screenshots were captured under `target/temp/newui/` and not committed.

1. Keep the current default theme. `document-light` and `document-dark` are
   listed by `/theme` and selectable like the other bundled themes.
2. Motion: M3 standard easing for ribbon tab switch, drawer open/close and
   snackbar (egui `animate_value_with_time`, 200 to 250 ms). Respect a reduced
   motion setting if app-core exposes one; otherwise instant.
3. High-contrast theme: verify every role pair at `AAA_CONTRAST`.
4. Update `GUI-QUICKSTART.md` (ribbon, collapse chord, page view), `SPEC.md`
   (GUI section), `CHANGELOG.md`.
5. Capture before/after screenshots into `target/temp/newui/` for review (not
   committed).

## 4. Order of work and total effort

| Phase | Depends on | Effort |
| --- | --- | --- |
| 1 Tokens | - | 1-2 days |
| 2 Fonts and icons | - | 1 day |
| 3 Components | 1, 2 | 3-4 days |
| 4 Chrome (ribbon, page) | 3 | 3-4 days |
| 5 Panes, status bar, dialogs | 3 | 3 days |
| 6 Polish and docs | 4, 5 | 1-2 days |

Total: about 12 to 16 working days. Phases 1 and 2 can run in parallel; 4 and
5 can run in parallel once 3 lands.

## 5. Risks

| Risk | Mitigation |
| --- | --- |
| Ribbon eats vertical space on small screens | Collapsible ribbon (tabs only), remembered in `gui-layout.json` |
| Custom shadows cost frame time | Draw shadows as 2-3 pre-tessellated rounded rects; measure with the existing perf checks |
| Third-party theme JSON lacks M3 roles | Derivation fallback in Phase 1 step 2 |
| Material Symbols font is large (variable font, several MB) | Ship a subset with only the glyphs `Icon` uses; record the subset command in the asset licence note |
| Look drifts toward a copy of a real product | Review against section 2.1 at the end of Phase 4 |

## 6. Decisions (resolved with the owner)

1. **Fonts:** switch from Inter to Roboto Flex where needed (Phase 2). Update
   tests that assert text metrics, each with a reason.
2. **macOS menu:** the ribbon sits below the native menu bar; the native menu
   (`platform/menu.rs`) is kept unchanged (Phase 4 step 1).
3. **Default theme:** unchanged. `document-light` / `document-dark` are
   available through `/theme` only (Phase 1 step 3, Phase 6 step 1).

## Follow-up after merge (owner's request)

- The layout no longer depends on the theme: the title strip, ribbon,
  Material panes, drawer rows and zoom slider are drawn on every theme, in
  that theme's Material roles (derived for themes without an `m3` block).
  The `File` menu bar is removed; its three items are ribbon buttons.
- The transcript and prompt fill their panes; the 816-point page column
  and desk margins are gone.
- The title strip shows the focused session's title, read from the
  sessions sidebar's cached rows (`sessions_sidebar_state::current_title`);
  `/name` now marks that cache dirty, so a rename shows at once.
- The ribbon's collapsed state is saved in `gui-ribbon.json`
  (`ribbon::load_ribbon` / `save_ribbon`).
- A pane the dock shows fades in over the Material motion time
  (`material::fade_in_opacity`); the dock has no slide animation, so the
  panes fade rather than slide.
