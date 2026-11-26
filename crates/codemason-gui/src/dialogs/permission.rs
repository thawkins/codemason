//! The permission dialog: the agent asks before a tool runs.
//!
//! Draws the front of `App::session.permission_queue` the way the TUI's
//! `render_permission_dialog` does: the permission, its first pattern, the
//! time left before the request expires and how many more are queued. When
//! the request carries the tool call it is about ([`permission_preview`]),
//! the pattern is replaced by a preview of exactly the input the call will
//! run with: every hunk of each file an edit will change (in the dialog,
//! which scrolls; the Diff pane would sit behind it), or the full command a
//! shell call will run. The
//! TUI offers three answers, `y` (allow once), `a` (always allow) and `n`
//! (deny); each button sends that key to the App, whose handler publishes
//! the reply. The keys themselves reach the App through the router.
//!
//! The dialog is a Material dialog: the permission is its body text, the
//! preview sits in a filled card, and the three answers are one right-to-left
//! action row with Allow once (the confirming action) rightmost. `Deny` is a
//! text button, `Always allow` tonal, `Allow once` filled.
//!
//! app-core answers a request only after it has been shown for
//! `PERMISSION_ANSWER_DELAY` and only once (`App::permission_answerable`),
//! so a double-click or a double `y` cannot answer twice or grant the next
//! request unread. The buttons are disabled while that holds.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::permission_preview::{PermissionPreview, permission_preview};

use super::{material_hint, material_modal, plain, send_key};
use crate::material::button::{ButtonKind, button};
use crate::material::card::{CardKind, card_frame};
use crate::material::dialog::{dialog_actions, dialog_body};
use crate::metrics::space;
use crate::palette::{Palette, rgb};

/// Title and accessible label of the dialog.
pub const PERMISSION_TITLE: &str = "Permission required";

/// The button that allows this request once (`y`).
pub const ALLOW_ONCE: &str = "Allow once";

/// The button that always allows this permission (`a`).
pub const ALWAYS_ALLOW: &str = "Always allow";

/// The button that denies the request (`n`).
pub const DENY: &str = "Deny";

/// The heading over an edit's diffs.
pub const CHANGES_LABEL: &str = "Changes:";

/// The heading over a shell call's command.
pub const COMMAND_LABEL: &str = "Command:";

/// The keyboard hint under the actions.
const HINT: &str = "Press y to allow, a to always allow, n to deny";

/// Width of the dialog's content, in points.
const WIDTH: f32 = 520.0;

/// Seconds a request waits when it carries no `timeout_secs` (the TUI's
/// default).
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Draw the permission dialog for the front request, if any.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose permission queue is read.
///
/// # Returns
///
/// `true` while a request is waiting.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    // The request is shown from this frame on; its buttons work once
    // app-core's answer delay has passed and until it is answered.
    app.note_permission_front();
    let answerable = app.permission_answerable();
    let Some(request) = app.session.permission_queue.front() else {
        return false;
    };
    let queued = app.session.permission_queue.len();
    let countdown = countdown(&request.metadata);
    let detail = request.patterns.first().cloned().unwrap_or_default();
    let permission = request.permission.clone();
    let preview = permission_preview(request, &app.cwd_path);
    let palette = crate::palette::palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_permission_dialog",
        PERMISSION_TITLE,
        WIDTH,
        |ui| {
            dialog_body(ui, &palette, &format!("Permission: {permission}"));
            match &preview {
                Some(PermissionPreview::Diffs(diffs)) => {
                    ui.label(CHANGES_LABEL);
                    egui::ScrollArea::both()
                        .max_height(super::body_height(ctx, 0.5, super::DIALOG_CHROME))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for diff in diffs {
                                crate::panels::diff::full_diff_ui(ui, diff, &palette);
                            }
                        });
                }
                Some(PermissionPreview::Command(command)) => {
                    ui.label(COMMAND_LABEL);
                    preview_card(ui, &palette, |ui| {
                        // A long heredoc scrolls, so the answer buttons stay
                        // in view.
                        egui::ScrollArea::vertical()
                            .id_salt("permission_command")
                            .max_height(super::body_height(ctx, 0.4, super::DIALOG_CHROME))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(command).monospace())
                                        .wrap(),
                                );
                            });
                    });
                }
                None => {
                    ui.label("Details:");
                    egui::ScrollArea::vertical()
                        .id_salt("permission_detail")
                        .max_height(super::body_height(ctx, 0.4, super::DIALOG_CHROME))
                        .show(ui, |ui| {
                            ui.monospace(&detail);
                        });
                }
            }
            material_hint(ui, &palette, &countdown);
            if queued > 1 {
                material_hint(ui, &palette, &format!("{queued} queued"));
            }
            ui.add_space(space::LG);
            actions(ui, &palette, answerable);
            ui.add_space(space::LG);
            material_hint(ui, &palette, HINT);
        },
    );
    // The countdown changes every second; disabled buttons wake sooner.
    ctx.request_repaint_after(if answerable {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(100)
    });
    true
}

/// Draw the preview in a filled card: the surface a Material dialog uses for
/// the content it is asking about.
fn preview_card(ui: &mut egui::Ui, palette: &Palette, add_contents: impl FnOnce(&mut egui::Ui)) {
    card_frame(CardKind::Filled, palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.visuals_mut().panel_fill = rgb(palette.m3.surface_container_highest);
        add_contents(ui);
    });
}

/// Draw the three answers, right to left: `Allow once` (filled) rightmost,
/// then `Always allow` (tonal), then `Deny` (text). Each sends its TUI key,
/// and all three are disabled until app-core lets the request be answered.
fn actions(ui: &mut egui::Ui, palette: &Palette, answerable: bool) {
    dialog_actions(ui, |ui| {
        ui.add_enabled_ui(answerable, |ui| {
            if button(ui, palette, ButtonKind::Filled, None, ALLOW_ONCE).clicked() {
                send_key(ui.ctx(), plain(Key::Char('y')));
            }
            if button(ui, palette, ButtonKind::Tonal, None, ALWAYS_ALLOW).clicked() {
                send_key(ui.ctx(), plain(Key::Char('a')));
            }
            if button(ui, palette, ButtonKind::Text, None, DENY).clicked() {
                send_key(ui.ctx(), plain(Key::Char('n')));
            }
        });
    });
}

/// The TUI's countdown text: `(m:ss remaining)`, or `(EXPIRED)`.
fn countdown(metadata: &serde_json::Value) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let created_at = metadata
        .get("created_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(now);
    let timeout = metadata
        .get("timeout_secs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let remaining = timeout.saturating_sub(now.saturating_sub(created_at));
    if remaining == 0 {
        "(EXPIRED)".to_string()
    } else {
        format!("({}:{:02} remaining)", remaining / 60, remaining % 60)
    }
}
