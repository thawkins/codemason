//! The run-cost banner (`App::session.run_cost_banner`): the one-line
//! summary a finished run leaves at the bottom of the window.
//!
//! It is not a modal. It stays up until the next key press or until it
//! expires (`App::poll_timers`, which the shell runs every frame).
//!
//! The next key press dismisses it the way app-core's `dispatch_key` does:
//! a plain character dismisses it and still types, any other key is used up
//! dismissing it. While the prompt (or another text field) has the
//! keyboard, the router leaves typing and editing keys to the editor, so
//! [`prepare`] applies the rule first: a plain character clears the banner
//! and stays for the editor; any other first key press is taken out of the
//! input and sent to `App::handle_key`, which dismisses the banner with it.
//! The Dismiss action sends Esc.
//!
//! The banner is a Material snackbar ([`snackbar`]): a transient notice with
//! one action, in the `inverse_surface` colour, at the bottom centre of the
//! window.
//!
//! Dependencies: `codemason-app-core`, `egui`, and this crate's Material
//! feedback.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;

use super::{plain, send_key};
use crate::keys::{is_key_text, to_key_input};
use crate::material::feedback::snackbar;
use crate::palette::palette_for;

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
/// * `app` - the App whose banner is drawn.
///
/// # Returns
///
/// `true` while the banner is shown.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(text) = app.session.run_cost_banner.as_deref() else {
        return false;
    };
    let palette = palette_for(app, ctx);
    // A snackbar, as Material shows a transient notice: the run summary is
    // its message and Dismiss its action. Its accessible name stays
    // `BANNER_LABEL`, so the summary itself is what assistive technology
    // hears under it.
    let dismissed = snackbar(
        ctx,
        &palette,
        egui::Id::unique("codemason_run_cost_banner"),
        BANNER_LABEL,
        text,
        Some(DISMISS_BANNER),
    );
    if dismissed {
        send_key(ctx, plain(Key::Esc));
    }
    true
}
