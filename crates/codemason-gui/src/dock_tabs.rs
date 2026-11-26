//! Suppression of the secondary-click menu on the left column's tab strip.
//!
//! craft-ui's docking renderer opens a tab menu on every tab
//! (`craft_ui::docking`'s `render::group`, `response.context_menu(...)`), and
//! that menu is not switchable: [`craft_ui::docking::Permissions`] gates its
//! Float, Move and Close items but not the menu itself, and
//! [`craft_ui::docking::DockStyle::close_menu_items`] only drops the Close
//! items, so Minimize and Collapse to Icons remain for a pinned tab.
//!
//! The left column's tabs are fixed: they cannot be closed, moved or floated
//! at all ([`crate::dock::PaneId::permissions`]), so a menu offering their
//! group actions says nothing useful and offers the one gesture that would
//! pull the strip apart. This module takes the secondary button away from the
//! strip instead of drawing a menu.
//!
//! The mechanism is the one [`crate::dialogs::context_menu`] already uses to
//! keep a right-press off the transcript: an [`egui::plugin::Plugin`] whose
//! [`egui::plugin::Plugin::input_hook`] drops the press and its release before
//! egui turns them into [`egui::Response::secondary_clicked`]. The strip's
//! rectangle is recorded each frame by the dock renderer
//! ([`set_strip`]) and read by the hook on the next pass.
//!
//! Only the strip is covered. The panes' bodies keep their own right-click
//! menus (the log and memory panes' Cut/Copy/Paste), and the dock's other
//! groups keep craft-ui's menu.
//!
//! Dependencies: `egui` (plugin hooks and input events) and
//! [`crate::dock`] for the strip the shell records.

/// Where the left column's tab strip was drawn last frame.
#[derive(Debug, Default)]
pub struct StripMenuGuard {
    /// The strip's layer and rectangle, while the strip is drawn.
    strip: Option<(egui::LayerId, egui::Rect)>,
    /// A secondary press landed on the strip and its release has not come
    /// yet, so the release is taken even if the pointer has since left.
    holding: bool,
}

impl StripMenuGuard {
    /// Whether `pos` lies on the recorded strip and on its layer, so a menu
    /// drawn over the strip (or a floating pane window) keeps its own
    /// right-clicks.
    fn over_strip(&self, ctx: &egui::Context, pos: egui::Pos2) -> bool {
        self.strip
            .is_some_and(|(layer, rect)| rect.contains(pos) && ctx.layer_id_at(pos) == Some(layer))
    }
}

impl egui::plugin::Plugin for StripMenuGuard {
    fn debug_name(&self) -> &'static str {
        "codemason_dock_strip_menu_guard"
    }

    fn input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        // A secondary press on the strip is not a tab menu's; the release that
        // completes it goes with it, so no widget sees a click either.
        input.events.retain(|event| {
            let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed,
                ..
            } = event
            else {
                return true;
            };
            if *pressed && self.over_strip(ctx, *pos) {
                self.holding = true;
                return false;
            }
            if !*pressed && self.holding {
                self.holding = false;
                return false;
            }
            true
        });
    }
}

/// Record where the left column's tab strip was drawn this frame, for the
/// next frame's check.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `layer` - the strip's layer.
/// * `rect` - the strip's rectangle, the tab row only, not the pane's body.
pub fn set_strip(ctx: &egui::Context, layer: egui::LayerId, rect: egui::Rect) {
    ctx.with_plugin::<StripMenuGuard, _>(|guard| guard.strip = Some((layer, rect)));
}

/// Forget the recorded strip, so no right-click is taken while the left
/// column is not drawn (the sidebar hidden with every panel off).
///
/// # Arguments
///
/// * `ctx` - the frame's context.
pub fn clear_strip(ctx: &egui::Context) {
    ctx.with_plugin::<StripMenuGuard, _>(|guard| guard.strip = None);
}

/// Install the guard, once per context.
///
/// # Arguments
///
/// * `ctx` - the window's context.
pub fn install(ctx: &egui::Context) {
    if ctx.with_plugin::<StripMenuGuard, _>(|_| ()).is_none() {
        ctx.add_plugin(StripMenuGuard::default());
    }
}
