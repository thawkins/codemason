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

use super::{key_buttons, modal, plain};

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
    modal(ctx, "codemason_permission_dialog", PERMISSION_TITLE, |ui| {
        ui.label(format!("Permission: {permission}"));
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
                egui::Frame::NONE
                    .fill(palette.raised1)
                    .stroke(egui::Stroke::new(1.0, palette.border))
                    .corner_radius(crate::metrics::radius::SM)
                    .inner_margin(crate::metrics::space::SM)
                    .show(ui, |ui| {
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
        ui.weak(countdown);
        if queued > 1 {
            ui.weak(format!("{queued} queued"));
        }
        ui.add_space(crate::metrics::space::SM);
        ui.add_enabled_ui(answerable, |ui| {
            key_buttons(
                ui,
                &[
                    (ALLOW_ONCE, plain(Key::Char('y'))),
                    (ALWAYS_ALLOW, plain(Key::Char('a'))),
                    (DENY, plain(Key::Char('n'))),
                ],
            );
        });
        ui.weak("Press y to allow, a to always allow, n to deny");
    });
    // The countdown changes every second; disabled buttons wake sooner.
    ctx.request_repaint_after(if answerable {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(100)
    });
    true
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
