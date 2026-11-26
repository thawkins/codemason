# MATERIALDIALOGS: Remaining TUI-Style Dialogs to Material

Status: Proposed
Scope: `crates/codemason-gui` presentation only. No `codemason-app-core`
behaviour changes.
Builds on: `docs/NEWUI.md` (Phase 5 dialogs, marked partly done)

Overview
--------
The GUI's dialogs are drawn two ways. Twenty-two `modal()` call sites still use
the generic helper, which is a bare `egui::Modal` with `ui.heading(title)` and
plain `ui.button(...)` actions - the TUI's look transcribed, not the Material
Design 3 look the rest of the window uses. The other nine dialogs were already
rebuilt on `material_modal()` with `surface_container_high`, a 28 pt radius,
elevation 3, outlined fields, list items and right-aligned filled/text actions.

This plan converts the remaining twenty-two modals and eight non-modal popup
surfaces, in dependency order, without touching the key paths that drive them.

Ground rules (they hold for every task)
---------------------------------------
- **app-core keeps the keys.** Every dialog is driven by `App` state and the key
  router. A Material button still sends the same `KeyInput` through
  `dialogs::send_key`, so no handler moves. `dialogs::material_actions(ui,
  palette, buttons, confirm_enabled)` already does exactly this.
- **Nothing is renamed.** Public label constants (`PERMISSION_TITLE`,
  `ALLOW_ONCE`, `OPTIONS_LABEL`, ...) stay; the tests key off them.
- **`ui.weak(...)` hint text becomes `material_hint`** in `on_surface_variant`
  small type. That is the only text-style change allowed.
- **Disabled still means disabled.** `add_enabled_ui` stays around whatever is
  gated (the answer delay, a blank prompt), so `accesskit_node().is_disabled()`
  keeps reporting it.
- **One dialog per commit.** Each task below is one reviewable change.

Inventory
---------
### A. Already Material (leave alone)

| Surface | How |
| --- | --- |
| `dialogs/provider.rs` | `material_modal` |
| `dialogs/model_picker.rs` | `material_modal` |
| `dialogs/router.rs` | `material_modal` |
| `dialogs/theme_editor.rs` | `material_modal` |
| `dialogs/properties.rs` | `material_modal` |
| `dialogs/swarm.rs` | `material_modal_response` |
| `dialogs/about.rs` | `material_modal_response` |
| `dialogs/confirm.rs::show_router_save` | `material_modal` |
| `dialogs/stores/*` | `stores::frame` on `material_modal` |
| `dialogs/diff_view.rs`, `dialogs/image_view.rs` | `dialog_frame` directly |
| `dialogs/session_menu.rs` | `material::menu::menu_frame` |

### B. Plain modals to convert (22 call sites, 9 files)

| # | File | Function | Dialog |
| --- | --- | --- | --- |
| 1 | `permission.rs` | `show` | Permission required |
| 2 | `question.rs` | `show` | Agent Question |
| 3 | `confirm.rs` | `show_rollback` | Roll back the loop's changes? |
| 4 | `confirm.rs` | `show_plan_approval` | Approve plan? |
| 5 | `confirm.rs` | `show_force_cleanup` | Force-cleanup Confirmation |
| 6 | `confirm.rs` | `show_notice` | notice |
| 7 | `confirm.rs` | `show_db_prune` | `/db prune` |
| 8 | `confirm.rs` | `show_compact_prune` | Delete every compaction summary? |
| 9 | `confirm.rs` | `show_memory_clear` | `/memory clear` |
| 10 | `confirm.rs` | `show_memory_delete` | memory delete |
| 11 | `confirm.rs` | `show_row_action` | destructive row action |
| 12 | `confirm.rs` | `show_memory_view` | full-memory view |
| 13 | `lists.rs` | `show_mcp_discover` | MCP Server Discovery |
| 14 | `lists.rs` | `show_plugins_list` | `/plugins list` |
| 15 | `lists.rs` | `show_backend_panel` | `/backend` |
| 16 | `pickers.rs` | `show_history` | History |
| 17 | `pickers.rs` | `show_theme` | Theme |
| 18 | `pickers.rs` | `show_config_saves` | Saved configurations |
| 19 | `palette.rs` | `show` | Command palette |
| 20 | `loop_setup.rs` | `show` | `/loop setup` |
| 21 | `shortcuts.rs` | `show` | Keyboard shortcuts |
| 22 | `output_view.rs` | `show` | tool-output overlay |

### C. Non-modal surfaces on `egui::Frame::popup` (7 sites, 8 surfaces)

| # | File | Surface | Target |
| --- | --- | --- | --- |
| 23 | `dialogs/banner.rs` | run-cost banner | `material::feedback::snackbar` |
| 24 | `dialogs/progress.rs` | model loading / download | new `material::feedback::circular_progress` |
| 25 | `dialogs/which_key.rs` | leader-key popup | `material::menu` |
| 26 | `dialogs/context_menu.rs` | right-click Cut/Copy/Paste | `material::menu` |
| 27 | `panels/queue.rs` | `Alt+Q` queue menu | `material::menu` |
| 28 | `panels/sessions.rs` | PR/MR request popup | `material::menu` + list items |
| 29 | `panels/prompt.rs` | completion / slash popup | `material::list::dense_item` |

`dialogs/stores/snapshot.rs` is data only and is not a dialog; `stores/` is
already Material.

Milestones
----------
Each task lists its own verify step. A milestone is done when
`cargo fmt --check`, `cargo clippy -p codemason-gui --all-targets` (zero
warnings) and `cargo nextest run -p codemason-gui` all pass.

### M1: shared groundwork (0.5 day) - DONE

**T1.1 `material::feedback::circular_progress`.** M3 indeterminate indicator:
a primary-coloured arc at 4 pt with the M3 1333 ms rotation, plus the existing
`TICK` repaint from `progress.rs`. Test: draws without the palette, exposes
`Role::ProgressIndicator`.
**T1.2 `material::menu` audit.** Confirm `menu_frame` + `menu_item` cover a
single-column menu with a trailing shortcut, a checkmark and a disabled row;
add `menu_item_enabled` if the disabled state is not reachable.
**T1.3 `dialogs::material_actions` for a non-key action.** `banner`'s Dismiss
and `output_view`'s Close already send Esc, so nothing is needed; verify only.

Done: `circular_progress` is `CIRCULAR_SIZE` 24 pt at `CIRCULAR_STROKE` 4 pt,
turning once per `CIRCULAR_PERIOD` (1.333 s), labelled `Role::ProgressIndicator`,
and it asks for its next frame after `CIRCULAR_FRAME` (33 ms) so a window
showing one still sleeps. The T1.2 audit needed no new helper:
`menu_item(.., trailing)` paints the accelerator at the trailing edge, a
checkmark is `Some(Icon::Check)` as the leading icon, and a disabled row is
`ui.add_enabled_ui(false, ..)` around it. `banner` and `output_view` each send
Esc with `send_key`, which is not the key-router path `material_actions`
encodes, so both stay as they are. Four tests added to
`test_material_menu.rs`, two to `test_material_feedback.rs`.

### M2: blocking dialogs with a user waiting (1.5 days) - highest value - DONE

**T2.1 `permission.rs`** - width 520. `Permission: <name>` becomes
`dialog_body`; the diff block and the mono command box move to a
`surface_container` card with the existing `full_diff_ui` and the
`body_height(...)` scroll caps. Actions: `Deny` (text), `Always allow`
(tonal), `Allow once` (filled) via `material_actions`, in that order so Allow
once is rightmost. The countdown and `+N more queued` become `material_hint`.
**T2.2 `question.rs`** - width 520. Question text via `dialog_body`. Options
become `material::list::list_item` rows (selected = the highlight, the default
marked by its trailing text), each click sending the same key as today; the
free-text branch swaps the labelled `TextEdit` for `material::text_field::
outlined_text_field` bound to `pending_question_input`, keeping
`answer_field_id()` so the router and the masking test still find it.
`Select`/`Dismiss` via `material_actions`, both inside `add_enabled_ui(answerable)`.

Verify: the existing `test_question_*` cases in `tests/test_dialogs_more.rs`
(option click, keys, free text, paste/IME, double-Enter-answers-once) pass
unchanged, plus a new assertion that the options are a `Role::List`.

Done: both dialogs are `material_modal` at width 520 with `dialog_body` for
their text, and pass all three gates (711 tests). `permission.rs` draws its
answer row itself rather than through `material_actions`, because that helper
can only disable the confirming button: the plan needs all three gated by
`add_enabled_ui(answerable)`, so the row keeps the helper's shape (right to
left, confirming first) and sends the same `y`/`a`/`n` keys. The command
preview moved into a `CardKind::Filled` card with `panel_fill` set to
`surface_container_highest` so the mono text sits on the card. `question.rs`
uses `list_item` rows with `DEFAULT_MARK` as the trailing text, and its
actions wrap in `add_enabled_ui(answerable)` for the same reason. A secret
question needs a masking Material field, so `material::text_field` gained
`outlined_secret_field` (the same field in `egui`'s password mode);
`answer_field_id()` is now `field_id(ANSWER_LABEL)`, the editor id the new
field actually uses. Three tests added to `test_dialogs_more.rs` (options are
a `Role::List` with the default marked, actions disabled until answerable, a
secret question reports `Role::PasswordInput`) and one to
`test_material_text_field.rs` (the secret field never reports its text).

### M3: confirmations (1 day) - DONE

**T3.1** `confirm.rs`'s ten plain dialogs onto `material_modal` at width 440.
`yes_no_buttons` becomes two buttons on the Material action row - `No` text,
`Yes` filled - still sending `Right` then `Enter`, so app-core's Yes/No cursor
stays the source of truth and the "opens on No" guarantee is preserved.
`show_memory_view` (a long read-only view) keeps a scroll body and a single
filled `Close`.
**T3.2** Delete the now-unused `yes_no_buttons` and any `ui.weak` calls under
`confirm.rs`.

Verify: `test_dialogs_more.rs`'s rollback / plan-approval / force-cleanup /
notice / db-prune cases and `tests/test_row_action_confirm_gui.rs`,
`test_memory_clear_confirm_gui.rs` pass with assertions on button labels only.

Done: all ten dialogs are `material_modal` at width 440 (`WIDTH`), with
`dialog_body` for their text, `material_hint` for every former `ui.weak`, and
`material_actions` for the one-key answers (Confirm/Cancel, OK, Close, Roll
back/Keep changes). `yes_no_buttons` is gone, replaced by `yes_no_actions`:
the "Selected: Yes|No" hint plus a row from the new `action_row` helper.
`material_actions` sends exactly one key per button, but a Yes/No or
Approve/Reject button has to move app-core's own cursor first (Right) and then
send Enter, so `action_row` takes a key sequence per button; the row still
starts with the confirming action, so `Yes`/`Approve` are filled and rightmost
and `No`/`Reject` are text buttons to their left. The plan approval's Approve
sends `[Right, Enter]` only while app-core's cursor is on Reject, exactly as
before. `show_memory_view` keeps its scroll body and gets a single filled
`Close`. Three tests added: `test_dialogs_more.rs` gained
`test_db_prune_dialog_is_a_material_row_with_the_confirming_button_rightmost`
(Yes is right of No, and clicking Yes moves the cursor before confirming) and
`test_compact_prune_dialog_no_cancels_and_yes_deletes`, and
`test_side_panels.rs`'s memory-view case now also clicks the Material `Close`
and checks the view shuts. Gates: 713 tests pass, clippy and fmt clean, doc
links ok.

### M4: pickers and lists (1.5 days) - DONE

**T4.1 `pickers.rs`** - `entry_list` gains a Material form: rows become
`list_item` (two-line where the row has a hint), selection = `selected`, double
click still sends Enter. The theme rows keep their trailing edit
`icon_button`. `entry_list` is shared with `lists.rs`, so do it once.
**T4.2 `lists.rs`** - `/mcp discover`, `/plugins list` and `/backend` reuse the
same `entry_list`; `/backend` marks the active backend with a trailing
`material::badge::badge`. Actions `Toggle`/`Switch`/`Close` via
`material_actions`, with `Close` (Esc) text.
**T4.3 `palette.rs`** - search field becomes `outlined_text_field`, rows
`list_item` with the action key as trailing text and the hint as supporting
text. Width stays the existing 60 %-of-window clamp.

Verify: `tests/test_palette.rs`, `test_command_palette.rs`,
`tests/test_dialogs.rs` history/theme/config-save cases,
`tests/test_dialogs_more.rs` history + theme picker cases; new assertions that
the row list is a `Role::List` with its label.

Done: `entry_list` is now `entry_list_with`, which draws `list_item_with` rows
(the new two-line-capable form of `list_item` that also takes a trailing slot),
a `Role::List` carrying the caller's label, and reports clicks exactly as
before. The hint per row is a `Vec<Option<String>>` so a row is two-line only
when its caller has a supporting line. The pickers are `material_modal` at 440
with `material_hint` for their former `ui.weak` lines and `action_row` for the
answers, because the plan needs several buttons gated individually (`Select`
and `Copy` are dead on an empty history) and `material_actions` can only gate
the confirming one - so that helper moved from `confirm.rs` into
`dialogs/mod.rs`. The theme rows keep their trailing edit `icon_button`, now in
the row's slot.
`lists.rs` reuses the same rows: `/backend` puts the new `ACTIVE_MARK` badge on
the active backend (and drops the old `" [active]"` text from the row), while
`/mcp discover` and `/plugins list` keep the TUI's `[x]`/`[ ]` marker in their
row text - that is the state the row reports, and the plan's badge requirement
names `/backend` alone.
`palette.rs` is a `material_modal` clamped at 60 % of the window (minimum 320),
its search field is the Material `outlined_text_field` under `SEARCH_LABEL`
whose node is re-rolled to `Role::SearchInput` (a Material field is a text
input; the palette's is a search box), and each row is a `list_item` with the
hint as its supporting line and the action's key as trailing text. `NOTHING_MATCHES` and
`SEARCH_HINT` are constants so a test names the same text the dialog draws.
One consequence worth noting: a key that used to be a separate `weak` label is
now the row's trailing text, which reaches assistive technology through the
row's description - `test_action_rows_show_their_key` asserts that instead of
two separate labels.
Four tests added: `test_command_palette.rs` gained
`test_palette_rows_are_a_material_list_with_the_hint_as_supporting_text` (the
list label, the `SearchInput` field, and an agent row's hint as its
description) and `test_palette_agent_row_carries_its_hint_as_supporting_text`;
`test_dialogs_more.rs`'s history case now asserts the entry list is a labelled
`Role::List` of two `Role::ListItem`s with the highlight marked selected;
`test_overlays.rs`'s `/backend` case asserts the rows are list items, the
active one is the selected row, and exactly one `Active` badge is drawn, and
its `/mcp discover` case asserts the list and keeps the `[ ]` marker
assertion; `test_dialog_bounds_gui.rs` gained
`test_dialog_bounds_long_picker_list_keeps_its_actions_in_a_small_window`.
Gates: 716 tests pass, clippy and fmt clean, doc links ok.

### M5: form and read-only views (1 day) - DONE

**T5.1 `loop_setup.rs`** - the seven key-driven fields become `outlined_value`
(the read-only Material field used by `properties.rs`), the active field drawn
focused; the agent picker and the checkpoints toggle become a list and a
`material::chip` respectively. `Start` filled, `Cancel` text.
**T5.2 `shortcuts.rs`** - keymap table onto `list_item` two-line rows (keys as
headline in mono trailing, description as supporting), one filled `Close`.
**T5.3 `output_view.rs`** - `material_modal_sized` with the existing virtualised
`MessagesView` in the body and a single filled `Close`; the title stays
`content.title`.

Verify: `test_dialogs_more.rs` loop cases (empty goal rejected, typing,
AltGr, IME, paste ignored off-fields) and
`tests/test_output_view_footer_gui.rs` pass unchanged.

Done: `loop_setup` is a `material_modal` at 480 with the seven fields as
`outlined_value` (the goal full width, the rest two to a row so the form fits a
normal window), the active field drawn focused with the caret `outlined_value`
already paints for a key-driven field - the TUI's `> ` marker is gone because
the field's own focus state is the marker. The agent picker is a `Role::List` of
`list_item` rows (the agent description as the supporting line) capped at three
rows so a long roster scrolls instead of pushing the form off the dialog, and
the checkpoints switch is a `material::chip` `ChipKind::Filter` whose `selected`
is app-core's boolean, so a click still sends Space (and moves app-core's active
field to `Checkpoints` first). `Start` is filled and `Cancel` text via
`material_actions`; the "Start a goal-driven loop" line and every hint are
`material_hint`.
`shortcuts` draws each `KEYBINDINGS` entry as a `list_item` whose headline is
the description and whose trailing text is the chord - the plan's "keys as
headline in mono trailing, description as supporting" would have put the long
chord (e.g. `Ctrl+Shift+Left/Right`) on the row's headline and squeezed the
description into the supporting line, so the two are the other way round: the
description reads as the row and the chord arrives as its description for
assistive technology. One filled `Close`.
`output_view` is a `material_modal_sized` at 80 % x 70 % of the window that
draws its own `dialog_headline` with `content.title` (a sized dialog has no
headline of its own), the virtualised `MessagesView` in a body measured as the
content height less the new `FOOTER_HEIGHT` (the Close button, its gaps and the
legend), and one filled `Close` above the legend. `NO_OUTPUT` and `FOOTER_HINT`
are constants so the tests name the same text the overlay draws.
One consequence: `dialogs::modal` and `dialogs::key_buttons` lost their last
callers, so `cargo clippy`'s dead-code lint failed the zero-warnings gate -
they are deleted here rather than in M8.1, which has nothing left to do.
Five tests added: `test_dialogs_more.rs` gained
`test_loop_dialog_draws_material_fields_a_list_and_a_chip` (the fields are
read-only values showing app-core's buffer and its config default, the agent
list is labelled with the selected agent marked, the chip reports checkpoints
on), `test_loop_dialog_chip_toggles_checkpoints_through_app_core` (a click
moves the field and sends Space, so app-core toggles it) and
`test_loop_dialog_agent_row_click_moves_the_highlight_only`;
`test_dialogs.rs` gained
`test_shortcuts_panel_rows_are_material_list_items_with_the_key_trailing` (every
binding is a `Role::ListItem` with its chord as the description, and the
Material `Close` closes the panel); `test_overlays.rs` gained
`test_output_view_is_a_material_dialog_with_one_filled_close` and
`test_output_view_footer_names_the_keys`. Gates: 722 tests pass, clippy and fmt
clean, doc links ok.

### M6: non-modal Material surfaces (1.5 days) - DONE

**T6.1 `banner.rs`** - `material::feedback::snackbar` with the run summary as
text and `Dismiss` as the action. The `prepare` key rule is untouched.
**T6.2 `progress.rs`** - `T1.1`'s circular indicator in a
`surface_container_highest` card at `surface_container` elevation 2 with
`on_surface_variant` label; the download keeps `linear_progress`.
**T6.3 `which_key.rs`** - `menu_frame` with `menu_item` rows (key as trailing).
**T6.4 `context_menu.rs`** - `menu_frame`/`menu_item` with icon, label and the
shortcut trailing, `menu_item` disabled where the action is. The selection
plumbing (`TranscriptSelection`, `take_transcript_right_click`,
`clear_selection_after_menu`) must not be touched: it is what made right-click
work, and three of its tests sit in `test_dialogs_more.rs`.

Verify: `test_dialogs_more.rs` banner + context-menu cases, and
`tests/test_dialogs.rs` menu cases.

Done: all four surfaces are Material and every key path is unchanged
(`prepare` in `banner.rs` and `context_menu.rs` was not touched, and the
three `TranscriptSelection` cases still pass).

`banner.rs` is a `snackbar`: the run summary is its message and Dismiss its
action (which still sends Esc), with `BANNER_LABEL` as its accessible name on
a `Role::Status`. That needed a label on `snackbar` - the first argument after
`id` is now the snackbar's name, so a caller can find its own notice and the
message is read separately. The gallery example and
`test_material_feedback.rs` were updated with it.

`progress.rs` draws each window on `card_frame(CardKind::Filled)` (a
`surface_container_highest` fill) raised to `elevation_shadow(2)` - the two
Material names the plan gives for a raised container, so the card supplies the
fill and the shadow supplies the elevation - with the window's label in
`on_surface_variant` small type, the heading and detail under it, and the
spinner as `circular_progress` (the download keeps `linear_progress`, whose
known progress a bar shows better than a spinner). The loading window's card
and its spinner are two `Role::ProgressIndicator` nodes, so the assertion is
`query_all_by_role(..).count() == 2`.

`which_key.rs` is a `menu_frame` with its title over `menu_item` rows, each
action's title as the row and its key as the trailing text (painted, so it is
the row's description rather than a widget of its own). The title is the
leading text and the key trails, so a long action name has the room and the
keys line up at the surface's trailing edge. `show` now takes the `Palette`,
as every Material component does; `shell.rs` passes the frame's.

`context_menu.rs` is a `menu_frame` holding one `menu_item` per action with
the action's icon leading and its label following, in a plain `ui.vertical`
inside the surface (so `menu_item`'s `available_width` is the surface's, and
the rows are one even block). The plan's "shortcut trailing" needed a
judgement call: the app-core context menu does not model a shortcut per item -
these are pointer actions driven by Up/Down/Enter, not chord-bound actions -
so a trailing accelerator would have been invented in the presentation layer.
The shortcut position is used for the state instead: the row app-core's
cursor is on is `menu_item_selected`, the menu's current row (filled
`secondary_container`, `selected` in accessibility). No key, and no label,
changed.

That state needed two additive changes in `material::menu`: `menu_item_at`
(the shared implementation) now reports `selected` itself (egui passes
`selected` on for tabs and options only, so an unstated one reaches assistive
technology as nothing), and `menu_item_selected` wraps it for a selected row.
The trailing description is reported in the same node builder, as a
`list_item`'s trailing text is. `menu_item` keeps its signature and its
behaviour, so `session_menu.rs` and the ribbon's tab overflow are untouched.

The context menu's icons (Cut, Paste, and a Markdown/Quote pair for the
transcript's actions) were not in the bundled Material Symbols subset, so the
font was subset again with the four extra code points (`U+E14E` Cut, `U+E14F`
Paste, `U+E244` Quote, `U+F552` Markdown) from the same upstream static
instance, and `assets/fonts/MaterialSymbols-LICENSE.txt`'s rebuild command
lists the new code points. `Icon::ALL` is 51 entries; `test_fonts.rs` proves
every one is in the face.

Tests added: `test_dialogs_more.rs` gained
`test_run_cost_banner_is_a_material_snackbar_with_a_dismiss_action` (a
labelled `Role::Status`, a Dismiss button, the summary as the message, and no
menu row for the action) and
`test_context_menu_rows_are_material_items_with_their_icons_and_state` (one
`Role::MenuListPopup` surface, a row per action matching app-core's enabled
state, the cursor's row the selected one, and a disabled click changing
nothing); `test_overlays.rs` gained
`test_context_menu_is_a_material_menu_with_an_icon_label_and_shortcut` (the
surface and its rows, the selected row, and a disabled row for Paste) and a
second-progress-node assertion for the loading card; `test_material_menu.rs`
gained `test_material_menu_item_selected_is_the_menus_current_row` and
`test_which_key_popup_is_a_menu_surface_of_items_keyed_at_the_trailing_edge`;
`test_command_palette.rs`'s which-key case now asserts a `Role::MenuItem` with
the key as its description rather than a bare label. Gates: 727 tests pass,
clippy and fmt clean, doc links ok (450 files), ASCII only.

### M7: in-panel popups (1 day) - DONE

**T7.1 `panels/queue.rs`** - `Alt+Q` menu onto `menu_frame`.
**T7.2 `panels/sessions.rs`** - the request popup onto `menu_frame` with
`list_item` rows; the `POPUP_WIDTH` and row-height maths stay.
**T7.3 `panels/prompt.rs`** - completion lists onto `dense_item`, keeping
`COMPLETION_ROWS` and the scroll-into-view behaviour.

Verify: `test_prompt.rs`'s queue-menu and completion cases,
`test_sessions_request_popup.rs`, and `test_material_list.rs`.

Done: all three popups are Material and every key and click path is
unchanged (`Alt+Q`'s four rows, Tab's acceptance of a completion, a click on
a completion row, and a click on a request all still go through app-core).

`queue.rs` draws its menu on `menu_frame` with one `menu_item` per row and
`menu_item_selected` on the row app-core's cursor is on; the empty-queue
`Next` row is drawn disabled, so a stray Enter still cannot act on it. The
rows carry no click action because app-core's keys run them, so each is a
plain item and the menu is a label surface. Two of its lines became Material:
the `^/v` footer and the `Show` panel's hint are `dialogs::material_hint`.

The `Role::Menu` label sits on the column the rows are in, not on the frame's
own response: `Area` (and `Frame::show`) give no accesskit node, so the frame
response is never the rows' ancestor and a labelled frame leaves the menu
with no children to query into.

`sessions.rs` puts the request popup on `menu_frame` and each request on
`material::list::list_item_with`: the title is the headline, `Merged * repo
#1` the supporting line, the diff size (`+30 -4`) the trailing text, and the
trailing slot paints the state glyph and, while Auto-fix watches the request,
the eye. `POPUP_WIDTH` and the row maths stay: `request_entry_height` is now
`TWO_LINE_HEIGHT`, which is what a two-line item is, so `POPUP_ROWS` still
bounds the popup. The rows' accessible names keep the shape the old
`Role::Link` rows had (`repo #1: title (state, 30 added, 4 removed,
watched by Auto-fix)`), because the diff size and the watch are painted
rather than read from a label; the state word and repository, which the item
does paint, arrive as its description.

`prompt.rs`'s completion popup is a `menu_frame` of `dense_item_option` rows
inside one vertical. That needed one additive component: `dense_item`'s row
with `Sense::click`, `Role::ListBoxOption`, the `selected` flag in
accessibility, and the `secondary_container` fill a selected row carries.
`dense_item` itself is unchanged in signature and behaviour - both now go
through a private `dense_row`, with `option: Option<bool>` selecting the
role and the click sense - so `context.rs`'s partitions are untouched.
`COMPLETION_ROWS` and the scroll-into-view maths (`first`, from the file
menu's `scroll_offset` and the slash menu's selected row) are unchanged.

Tests added: `test_material_list.rs` gained
`test_material_dense_item_option_is_the_list_boxes_selected_row` (options one
`DENSE_HEIGHT` high, only the chosen one selected, a click naming the row);
`test_prompt.rs` gained
`test_queue_menu_rows_are_material_items_with_the_cursor_selected` (one
`Role::Menu` surface, `Role::MenuItem` per row, only the cursor's row
selected, the Material hint, and `Next` disabled on an empty queue) and
`test_prompt_mention_completions_are_material_list_options` (the list box,
each match a dense `Role::ListBoxOption`, only the highlighted one selected,
and a click on a file row still accepted into the prompt);
`test_sessions_request_popup.rs` gained
`test_sessions_request_popup_rows_are_material_list_items` (two-line items of
a labelled list, the supporting line as their description, and the diff size
and Auto-fix watch in their names). Gates: 731 tests pass, clippy and fmt
clean, doc links ok (450 files), ASCII only, every shell guard clean.

### M8: retire the plain helper and document (0.5 day) - DONE

**T8.1** Delete `dialogs::modal` and `dialogs::key_buttons` once no caller
remains; `body_height` stays for the Material scroll caps. (Done in M5: its last
three callers were `loop_setup`, `shortcuts` and `output_view`, so both helpers
went with them rather than leaving a zero-warnings failure in between.)
**T8.2** Update `docs/NEWUI.md` Phase 5's "Dialogs are not rebuilt" note to
point at this plan's completion, and add the `CHANGELOG.md` entry.
**T8.3** Add `crates/codemason-gui/tests/test_material_dialogs.rs`: a sweep
that opens each dialog in `dialogs::ActiveDialog` and asserts a `Role::Dialog`
node carries its title constant - one case per dialog, table-driven, so a new
dialog added without a Material frame fails the sweep.

Done: both helpers are gone and `body_height` is the only survivor of that
pair, kept because every Material dialog's scrolling list or preview caps its
height with it (`DIALOG_CHROME`, `MIN_BODY_HEIGHT`).

`docs/NEWUI.md`'s Phase 5 bullet now carries a "Superseded:" line pointing at
this plan, so the "Dialogs are not rebuilt" note reads as the decision it was
rather than as the current state.

The sweep is `test_every_dialog_is_drawn_on_a_material_surface` over a
`VARIANTS` table with one row per `ActiveDialog` variant. The row's role is
`Role::Dialog` for a Material dialog, and the two menu surfaces depart from
`Role::Dialog` on purpose: `SessionMenu` is drawn on `Role::Menu`
(`session_menu::surface`) and the right-click menu on `Role::MenuListPopup`,
both on `material::menu`. Its exhaustiveness is structural rather than a
runtime check: `fn case(dialog: ActiveDialog) -> Case` matches every variant
with no wildcard arm, so a variant added to the enum fails to compile until
the sweep names it, and a second test asserts the `VARIANTS` table lists each
variant once and that each row opens its own dialog. Two more assertions per
row keep the sweep honest: `dialogs::active` must report that variant (a
dialog whose state something higher in app-core's priority order shadows is
caught), and the surface must be queryable by role and title (a dialog drawn
without its Material frame, or with a title that drifted from its constant,
is caught). Every case builds its own App, so no case depends on another's
state, and the harness steps a fixed four frames rather than settling because
the two store browsers keep repainting while they load.

Gates: 733 tests pass, clippy and fmt clean, doc links ok (450 files), ASCII
only, every shell guard clean. Verified in the negative: swapping one case's
title for a wrong string fails the sweep naming that dialog
(`MemoryClear draws no Dialog labelled "not a title"`).

Risks and mitigations
---------------------
| Risk | Mitigation |
| --- | --- |
| A converted dialog loses its keyboard path | Every task's verify keeps the existing key tests; buttons only add a click route |
| `outlined_text_field` binds a second editor to a key-driven field | Only `question.rs`'s free text and `palette.rs`'s query are real `TextEdit`s today; `loop_setup` uses `outlined_value` (read-only), exactly as `properties.rs` does |
| Focus moves when a real `TextEdit` is introduced | `question.rs` already requests focus for its field and `dialogs::show`'s `unfocused` check names `question::answer_field_id()`; keep both |
| Menu conversions break right-click selection | `T6.4` changes chrome only; the three `TranscriptSelection` tests are the gate |
| A dialog grows past a short window | `material_modal` already clamps width, caps height and scrolls; do not set a fixed `set_min_size` on a content-sized dialog |
| Test churn | Assert on label constants and roles, never on colours or pixel sizes |

Out of scope
------------
- Any `codemason-app-core` change: no new dialog state, no new keys.
- The TUI (`crates/codemason-tui`): it keeps its own rendering.
- `dialogs/stores/`, `provider`, `model_picker`, `router`, `theme_editor`,
  `properties`, `swarm`, `about`, `diff_view`, `image_view`, `session_menu` -
  already Material.
- Adding new Material components beyond `T1.1` and the `T1.2` audit.

Effort
------
| Milestone | Days |
| --- | --- |
| M1 groundwork | 0.5 |
| M2 permission + question | 1.5 |
| M3 confirmations (10) | 1.0 |
| M4 pickers + lists (6) | 1.5 - done |
| M5 form and read-only views (3) | 1.0 - done |
| M6 non-modal surfaces (4) | 1.5 - done |
| M7 in-panel popups (3) | 1.0 - done |
| M8 retire helper + sweep test | 0.5 - done |
| **Total** | **8.5** |

Order matters: M1 unblocks M6, M4's `entry_list` change serves `lists.rs`, and
M8's sweep test can only pass once every dialog in section B is converted.
