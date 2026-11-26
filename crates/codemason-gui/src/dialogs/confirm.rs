//! The confirmation dialogs: the post-loop rollback offer, plan approval,
//! team force-cleanup, the router configuration save, the single-button
//! notice, the `/db prune` confirmation, the destructive row-action
//! confirmation of the Agents, Teams and Sessions panels, the Memory
//! panel's delete confirmation, the `/memory clear` confirmation, and the
//! full-memory view the Memory panel opens.
//!
//! Each is a small app-core state with keys only (`input.rs`): Enter
//! confirms and Esc cancels, the plan approval moves its Approve/Reject
//! cursor with Left/Right (and `r` rejects), and the `/db prune` dialog moves
//! between Yes and No with Left/Right/Tab. The TUI draws no dialog for the
//! rollback offer and the plan approval; here each gets one, with the same
//! keys. The keys reach the App through the router, and each button sends
//! the key presses that make the same choice, so the App's own arm acts.
//!
//! The row-action, memory-clear and `/db prune` dialogs select No when they
//! open (app-core's default), so a stray Enter performs nothing; Left, Right
//! and Tab move between Yes and No. The memory delete confirms with Enter and
//! cancels with Esc, as in the TUI.
//!
//! The rollback offer, plan approval and force-cleanup answer something the
//! agent (or a loop) is waiting on. Each is one `Option` slot that app-core
//! takes before it acts (`ConfirmRollback`, `ApprovePlan`,
//! `ConfirmForceCleanup` and their cancels in `input_handler.rs`), so the
//! answer clears it locally at once and a second Enter or click finds
//! nothing to answer; no queue sits behind it.
//!
//! Each is a Material dialog: the title is its headline, its text is
//! `dialog_body`, and its answers are one right-aligned action row with the
//! confirming action filled and rightmost and the cancelling one a text
//! button. A Yes/No choice stays app-core's: app-core's cursor decides what
//! Enter does, so the row still moves the choice to the button pressed
//! (Right) before it sends Enter, and a hint names the selected option.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::{Key, KeyInput};

use super::{action_row, material_actions, material_hint, material_modal, plain};
use crate::material::button::ButtonKind;
use crate::material::dialog::dialog_body;
use crate::metrics::space;
use crate::palette::Palette;

/// Title of the rollback offer.
pub const ROLLBACK_TITLE: &str = "Roll back the loop's changes?";

/// The room the memory view's metadata lines, buttons and hint take, in
/// points, beside its scrolling content.
const MEMORY_VIEW_CHROME: f32 = 300.0;

/// The button that rolls the workspace back (Enter).
pub const ROLL_BACK: &str = "Roll back";

/// The button that keeps the loop's changes (Esc).
pub const KEEP_CHANGES: &str = "Keep changes";

/// Title of the plan approval dialog.
pub const PLAN_TITLE: &str = "Approve plan?";

/// The button that approves the plan.
pub const APPROVE: &str = "Approve";

/// The button that rejects the plan (`r`).
pub const REJECT: &str = "Reject";

/// Title of the force-cleanup confirmation (the TUI's heading).
pub const FORCE_CLEANUP_TITLE: &str = "Force-cleanup Confirmation";

/// The button that confirms a confirmation dialog (Enter).
pub const CONFIRM: &str = "Confirm";

/// The button that cancels a confirmation dialog (Esc).
pub const CANCEL: &str = "Cancel";

/// Title of the router save confirmation (the TUI's heading).
pub const ROUTER_SAVE_TITLE: &str = "Save Router Configuration";

/// Width of the router save confirmation's content, in points.
const ROUTER_SAVE_WIDTH: f32 = 400.0;

/// The button that saves the router configuration (Enter).
pub const SAVE: &str = "Save";

/// The notice dialog's only button (Enter).
pub const NOTICE_OK: &str = "OK";

/// The `/db prune` dialog's Yes button.
pub const YES: &str = "Yes";

/// The `/db prune` dialog's No button.
pub const NO: &str = "No";

/// Title of the `/db prune` confirmation for a window of `days` days (the
/// TUI's title).
///
/// # Arguments
///
/// * `days` - the retention window the armed prune applies.
///
/// # Returns
///
/// The dialog title.
#[must_use]
pub fn db_prune_title(days: u32) -> String {
    format!("Delete messages older than {days} day(s)?")
}

/// Title of the `/compact prune --all` confirmation.
pub const COMPACT_PRUNE_TITLE: &str = "Delete every compaction summary?";

/// Enter and Esc as the TUI names them in these dialogs.
const ENTER: KeyInput = plain(Key::Enter);
/// See [`ENTER`].
const ESC: KeyInput = plain(Key::Esc);

/// Width of a confirmation's content, in points.
const WIDTH: f32 = 440.0;

/// Draw the post-loop rollback offer (`App::session.pending_rollback`).
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose rollback offer is read.
///
/// # Returns
///
/// `true` while the offer is open.
pub fn show_rollback(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(offer) = app.session.pending_rollback.as_ref() else {
        return false;
    };
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_rollback_dialog",
        ROLLBACK_TITLE,
        WIDTH,
        |ui| {
            dialog_body(
                ui,
                &palette,
                &format!(
                    "The loop ended ({}) after {} iteration(s).",
                    offer.status, offer.iterations
                ),
            );
            if !offer.diffstat.is_empty() {
                ui.label(offer.diffstat.as_str());
                ui.add_space(space::MD);
            }
            egui::ScrollArea::vertical()
                .id_salt("rollback_files")
                .max_height(super::body_height(ctx, 0.4, super::DIALOG_CHROME))
                .show(ui, |ui| {
                    for file in &offer.files {
                        ui.monospace(file.as_str());
                    }
                });
            ui.add_space(space::LG);
            material_actions(
                ui,
                &palette,
                &[(ROLL_BACK, ENTER), (KEEP_CHANGES, ESC)],
                true,
            );
            ui.add_space(space::LG);
            material_hint(
                ui,
                &palette,
                "Enter rolls back to the pre-loop snapshot  Esc keeps the changes",
            );
        },
    );
    true
}

/// Draw the plan approval dialog (`App::session.plan_approval_pending`).
///
/// Approve sends Right first when the TUI's cursor is on Reject (Left and
/// Right toggle it), then Enter; Reject sends `r`.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose pending plan is read.
///
/// # Returns
///
/// `true` while the plan waits for approval.
pub fn show_plan_approval(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(state) = app.session.plan_approval_pending.as_ref() else {
        return false;
    };
    let approve: &[KeyInput] = if state.cursor_approve {
        &[ENTER]
    } else {
        &[plain(Key::Right), ENTER]
    };
    let palette = crate::palette::palette_for(app, ctx);
    let cursor = if state.cursor_approve {
        APPROVE
    } else {
        REJECT
    };
    material_modal(
        ctx,
        &palette,
        "codemason_plan_dialog",
        PLAN_TITLE,
        WIDTH,
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("plan_text")
                .max_height(super::body_height(ctx, 0.6, super::DIALOG_CHROME))
                .show(ui, |ui| {
                    ui.label(state.plan_text.as_str());
                });
            ui.add_space(space::LG);
            // Approve moves app-core's cursor to itself first (Right) unless
            // it is already there, so its own Next/Previous arm confirms.
            action_row(
                ui,
                &palette,
                &[
                    (APPROVE, ButtonKind::Filled, approve, true),
                    (REJECT, ButtonKind::Text, &[plain(Key::Char('r'))], true),
                ],
            );
            ui.add_space(space::LG);
            material_hint(ui, &palette, &format!("Selected: {cursor}"));
            material_hint(
                ui,
                &palette,
                "Left/Right choose  Enter confirm  r or Esc reject",
            );
        },
    );
    true
}

/// Draw the team force-cleanup confirmation
/// (`App::session.pending_forcecleanup`).
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose pending force-cleanup is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_force_cleanup(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(pending) = app.session.pending_forcecleanup.as_ref() else {
        return false;
    };
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_forcecleanup_dialog",
        FORCE_CLEANUP_TITLE,
        WIDTH,
        |ui| {
            dialog_body(ui, &palette, &format!("Team: {}", pending.team_name));
            if !pending.active_members.is_empty() {
                ui.label("Active teammates:");
                for member in &pending.active_members {
                    ui.monospace(format!("  - {member}"));
                }
            }
            ui.add_space(space::LG);
            material_actions(ui, &palette, &[(CONFIRM, ENTER), (CANCEL, ESC)], true);
            ui.add_space(space::LG);
            material_hint(ui, &palette, "Enter confirm  Esc cancel");
        },
    );
    true
}

/// Draw the router configuration save confirmation
/// (`App::pending_router_save`).
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose router draft is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_router_save(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(draft) = app.pending_router_save.as_ref() else {
        return false;
    };
    let total: usize = draft.tiers.values().map(|t| t.models.len()).sum();
    let palette = crate::palette::palette_for(app, ctx);
    super::material_modal(
        ctx,
        &palette,
        "codemason_router_save_dialog",
        ROUTER_SAVE_TITLE,
        ROUTER_SAVE_WIDTH,
        |ui| {
            crate::material::dialog::dialog_body(
                ui,
                &palette,
                &format!(
                    "This replaces the Model Router cluster in codemason.json with \
                     {total} tier {}.",
                    if total == 1 { "entry" } else { "entries" }
                ),
            );
            super::material_hint(ui, &palette, "Enter save  Esc cancel");
            ui.add_space(crate::metrics::space::LG);
            super::material_actions(ui, &palette, &[(SAVE, ENTER), (CANCEL, ESC)], true);
        },
    );
    true
}

/// Draw the single-button notice (`App::notice_open`, with its title,
/// message and detail).
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose notice is read.
///
/// # Returns
///
/// `true` while the notice is open.
pub fn show_notice(ctx: &egui::Context, app: &mut App) -> bool {
    if !app.notice_open {
        return false;
    }
    // The App keeps the TUI's border padding around the title.
    let title = app.notice_title.trim();
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_notice_dialog",
        title,
        WIDTH,
        |ui| {
            dialog_body(ui, &palette, &app.notice_message);
            if let Some(detail) = app.notice_detail.as_deref() {
                // A stack trace or log scrolls on its own, so OK stays in view.
                egui::ScrollArea::both()
                    .id_salt("notice_detail")
                    .max_height(super::body_height(ctx, 0.5, super::DIALOG_CHROME))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.monospace(detail);
                    });
            }
            ui.add_space(space::LG);
            material_actions(ui, &palette, &[(NOTICE_OK, ENTER)], true);
            ui.add_space(space::LG);
            material_hint(ui, &palette, "Enter or Esc closes");
        },
    );
    true
}

/// Draw the `/db prune` confirmation (`App::db_prune_confirm_open`).
///
/// Yes and No first move the App's choice to themselves (Right toggles it,
/// as Left/Right/Tab do in the TUI), then send Enter, which acts on the
/// choice.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose prune confirmation is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_db_prune(ctx: &egui::Context, app: &mut App) -> bool {
    if !app.db_prune_confirm_open {
        return false;
    }
    let title = db_prune_title(app.db_prune_confirm_days.unwrap_or_default());
    let yes = app.db_prune_confirm_is_yes();
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_db_prune_dialog",
        &title,
        WIDTH,
        |ui| {
            yes_no_actions(ui, &palette, yes);
            ui.add_space(space::LG);
            material_hint(
                ui,
                &palette,
                "Left/Right move * Enter on Yes performs * Enter on No cancels * Esc cancels",
            );
        },
    );
    true
}

/// Title of the `/memory clear` confirmation (the TUI's title).
pub const MEMORY_CLEAR_TITLE: &str = "Clear this project's memory?";

/// Draw the `/compact prune --all` confirmation
/// (`App::compact_prune_confirm_open`).
///
/// Same two-option shape as [`show_db_prune`]: Yes and No first move the App's
/// choice to themselves (Right toggles it), then send Enter, which acts on the
/// choice.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose prune confirmation is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_compact_prune(ctx: &egui::Context, app: &mut App) -> bool {
    if !app.compact_prune_confirm_open {
        return false;
    }
    let yes = app.compact_prune_confirm_is_yes();
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_compact_prune_dialog",
        COMPACT_PRUNE_TITLE,
        WIDTH,
        |ui| {
            yes_no_actions(ui, &palette, yes);
            ui.add_space(space::LG);
            material_hint(
                ui,
                &palette,
                "Left/Right move * Enter on Yes deletes * Enter on No cancels * Esc cancels",
            );
        },
    );
    true
}

/// Title of the Memory panel's delete confirmation (the TUI's heading).
pub const MEMORY_DELETE_TITLE: &str = "Delete Memory?";

/// The title of the open row-action confirmation (`App::row_action_confirm`:
/// kill a sub-agent, cancel a background task, suspend, shut down or resume
/// a teammate, delete sessions), without the TUI's border padding.
///
/// # Arguments
///
/// * `app` - the App whose confirmation is read.
///
/// # Returns
///
/// The title, or `None` while no row action waits for an answer.
#[must_use]
pub fn row_action_title(app: &App) -> Option<String> {
    app.row_action_confirm
        .as_ref()
        .map(|pending| pending.action.title().trim().to_string())
}

/// Yes and No buttons over a two-option app-core dialog whose choice
/// Right toggles: the button first moves the choice to itself, then sends
/// Enter, which acts on the choice.
fn yes_no_actions(ui: &mut egui::Ui, palette: &Palette, yes_selected: bool) {
    let enter = plain(Key::Enter);
    let right_then_enter = [plain(Key::Right), enter];
    let yes_keys: &[KeyInput] = if yes_selected {
        &[enter]
    } else {
        &right_then_enter
    };
    let no_keys: &[KeyInput] = if yes_selected {
        &right_then_enter
    } else {
        &[enter]
    };
    material_hint(
        ui,
        palette,
        &format!("Selected: {}", if yes_selected { YES } else { NO }),
    );
    ui.add_space(space::MD);
    action_row(
        ui,
        palette,
        &[
            (YES, ButtonKind::Filled, yes_keys, true),
            (NO, ButtonKind::Text, no_keys, true),
        ],
    );
}

/// Draw the `/memory clear` confirmation (`App::memory_clear_confirm_open`).
/// No is selected when it opens, so a stray Enter clears nothing; memory is
/// cleared only by app-core's `ConfirmMemoryClear` on Yes.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose confirmation is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_memory_clear(ctx: &egui::Context, app: &mut App) -> bool {
    if !app.memory_clear_confirm_open {
        return false;
    }
    let yes = app.memory_clear_confirm_is_yes();
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_memory_clear_dialog",
        MEMORY_CLEAR_TITLE,
        WIDTH,
        |ui| {
            yes_no_actions(ui, &palette, yes);
            ui.add_space(space::LG);
            material_hint(
                ui,
                &palette,
                "Left/Right move * Enter on Yes performs * Enter on No cancels * Esc cancels",
            );
        },
    );
    true
}

/// Draw the Memory panel's delete confirmation
/// (`App::pending_memory_delete`): the memory's id and preview; Enter
/// deletes it, Esc keeps it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose pending delete is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_memory_delete(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(pending) = app.pending_memory_delete.as_ref() else {
        return false;
    };
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_memory_delete_dialog",
        MEMORY_DELETE_TITLE,
        WIDTH,
        |ui| {
            dialog_body(ui, &palette, &format!("Memory ID: {}", pending.id));
            material_hint(ui, &palette, &format!("Preview: {}", pending.preview));
            ui.add_space(space::LG);
            material_actions(ui, &palette, &[(CONFIRM, ENTER), (CANCEL, ESC)], true);
            ui.add_space(space::LG);
            material_hint(ui, &palette, "Enter confirm  Esc cancel");
        },
    );
    true
}

/// Draw the destructive row-action confirmation (`App::row_action_confirm`)
/// the Agents, Teams and Sessions panels arm: its title names the action,
/// its body the target. No is selected when it opens; the action runs only
/// in app-core's `row_action_confirm_accept` on Yes.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose pending row action is read.
///
/// # Returns
///
/// `true` while the confirmation is open.
pub fn show_row_action(ctx: &egui::Context, app: &mut App) -> bool {
    let (Some(title), Some(pending)) = (row_action_title(app), app.row_action_confirm.as_ref())
    else {
        return false;
    };
    let body = pending.action.body();
    let yes = app.row_action_confirm_is_yes();
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_row_action_dialog",
        &title,
        WIDTH,
        |ui| {
            dialog_body(ui, &palette, &body);
            ui.add_space(space::LG);
            yes_no_actions(ui, &palette, yes);
            ui.add_space(space::LG);
            material_hint(
                ui,
                &palette,
                codemason_app_core::app::ROW_ACTION_TOGGLE_HINT,
            );
            material_hint(
                ui,
                &palette,
                codemason_app_core::app::ROW_ACTION_OUTCOME_HINT,
            );
        },
    );
    true
}

/// Title of the full-memory view for memory `id` in `category` (the TUI's
/// title).
///
/// # Arguments
///
/// * `id` - the memory's id.
/// * `category` - its category.
///
/// # Returns
///
/// The dialog title.
#[must_use]
pub fn memory_view_title(id: i64, category: &str) -> String {
    format!("Memory #{id} - {category}")
}

/// Draw the full-memory view (`App::memory_view`) the Memory panel opens
/// with Enter: the memory's id, category, confidence, source and update
/// time, then its content. Esc (or Close) closes it through app-core.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose memory view is read.
///
/// # Returns
///
/// `true` while the view is open.
pub fn show_memory_view(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(view) = app.memory_view.as_ref() else {
        return false;
    };
    let row = &view.row;
    let title = memory_view_title(row.id, &row.category);
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_memory_view_dialog",
        &title,
        WIDTH,
        |ui| {
            material_hint(ui, &palette, &format!("ID: {}", row.id));
            material_hint(ui, &palette, &format!("Category: {}", row.category));
            material_hint(ui, &palette, &format!("Confidence: {:.2}", row.confidence));
            material_hint(ui, &palette, &format!("Source: {}", row.source));
            material_hint(ui, &palette, &format!("Updated: {}", row.updated_at));
            ui.add_space(space::MD);
            ui.label("Content:");
            // Five metadata lines sit above the content.
            egui::ScrollArea::vertical()
                .id_salt("memory_view_content")
                .max_height(super::body_height(ctx, 0.6, MEMORY_VIEW_CHROME))
                .show(ui, |ui| {
                    ui.label(row.content.as_str());
                });
            ui.add_space(space::LG);
            // A read-only view has one action, and it is the emphasised one.
            action_row(
                ui,
                &palette,
                &[(super::output_view::CLOSE, ButtonKind::Filled, &[ESC], true)],
            );
            ui.add_space(space::LG);
            material_hint(ui, &palette, "Esc close");
        },
    );
    true
}
