//! The run-cost banner (`App::session.run_cost_banner`): the one-line
//! summary a finished run leaves near the top of the window.
//!
//! It is not a modal. As in the TUI (`render_run_cost_banner`) it sits above
//! everything else, at the top, until the next key press or until it
//! expires (`App::poll_run_cost_banner_expiry`, polled while it is drawn).
//!
//! The next key press dismisses it the way app-core's `dispatch_key` does:
//! a plain character dismisses it and still types, any other key is used up
//! dismissing it. While the prompt (or another text field) has the
//! keyboard, the router leaves typing and editing keys to the editor, so
//! [`prepare`] applies the rule first: a plain character clears the banner
//! and stays for the editor; any other first key press is taken out of the
//! input and sent to `App::handle_key`, which dismisses the banner with it.
//! The Dismiss button sends Esc.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use std::time::Duration;

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;

use super::{plain, send_key};
use crate::keys::{is_key_text, to_key_input};

/// Accessible label of the banner.
pub const BANNER_LABEL: &str = "Run summary";

/// The button that dismisses the banner (Esc).
pub const DISMISS_BANNER: &str = "Dismiss";

/// Apply the banner's dismiss rule to this frame's first key press.
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose banner is dismissed.
pub fn prepare(ctx: &egui::Context, app: &mut App) {
    if app.session.run_cost_banner.is_none() {
        return;
    }
    let first = ctx.input(|input| {
        input
            .events
            .iter()
            .enumerate()
            .find_map(|(i, event)| to_key_input(event).map(|key| (i, key)))
    });
    let Some((at, key)) = first else {
        return;
    };
    let plain_char = matches!(key.key, Key::Char(_)) && !key.mods.ctrl && !key.mods.alt;
    if plain_char {
        // The TUI clears the banner and lets the character through.
        app.session.run_cost_banner = None;
        app.session.run_cost_banner_at = None;
        return;
    }
    ctx.input_mut(|input| {
        let typed = matches!(
            input.events.get(at + 1),
            Some(egui::Event::Text(text)) if is_key_text(key, text)
        );
        if typed {
            input.events.remove(at + 1);
        }
        input.events.remove(at);
    });
    send_key(ctx, key);
}

/// Draw the banner while the App has one.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose banner is drawn; an expired one is cleared.
///
/// # Returns
///
/// `true` while the banner is shown.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    app.poll_run_cost_banner_expiry();
    let Some(text) = app.session.run_cost_banner.as_deref() else {
        return false;
    };
    egui::Area::new(egui::Id::unique("codemason_run_cost_banner"))
        .order(egui::Order::Tooltip)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 8.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let row = ui.horizontal(|ui| {
                    ui.strong(text);
                    if ui.small_button(DISMISS_BANNER).clicked() {
                        send_key(ui.ctx(), plain(Key::Esc));
                    }
                });
                row.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Status, true, BANNER_LABEL)
                });
            });
        });
    // The banner expires on its own; look again in a second.
    ctx.request_repaint_after(Duration::from_secs(1));
    true
}
