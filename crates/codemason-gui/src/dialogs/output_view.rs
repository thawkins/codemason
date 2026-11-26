//! The tool-output overlay (`App::output_view`): the transcript of a
//! sub-agent's or teammate's session, opened from the Agents and Teams
//! windows.
//!
//! app-core decides what it shows ([`App::output_view_content`]: the title,
//! the target session and its messages) and runs its keys
//! (`input::handle_key`): PageUp/PageDown scroll, Ctrl+PageUp/PageDown jump
//! to the start or end, Alt+Up/Down move the teammate focus, Esc closes. The
//! keys reach the App through the router, as in the TUI.
//!
//! Outputs can be long, so the overlay reuses the virtualised
//! [`MessagesView`]: only the messages in view are laid out. app-core keeps
//! the scroll position in lines from the bottom (`scroll_offset`, clamped to
//! `max_scroll`, which the frontend measures); this overlay writes
//! `max_scroll` from the view's real height every frame, moves the view
//! when a key changed `scroll_offset`, and writes the wheel's scrolling back
//! to `scroll_offset`, so keys and wheel agree. As in the TUI, offset 0 pins
//! the view to the tail, so a running agent's new output stays in view, and
//! any other offset keeps its distance from the bottom. A session other
//! than the focused one is read back from storage, which does not keep
//! message edit counters, so its view is rebuilt whenever the output
//! changes and put back at app-core's offset.
//!
//! Dependencies: `codemason-app-core`, [`crate::panels::messages`] and
//! `egui`.
//!
//! The overlay is a Material dialog of a fixed size (the TUI's 90 x 70
//! share of the window) that draws its own headline, the virtualised
//! transcript in its body and one filled `Close` above the key legend.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;

use super::{material_actions, material_hint, material_modal_sized, plain};
use crate::material::button::HEIGHT as BUTTON_HEIGHT;
use crate::metrics::space;
use crate::palette::palette_for;
use crate::panels::messages::{MessageSource, MessagesView, ScrollTo};

/// Accessible label of the output transcript.
pub const OUTPUT_LABEL: &str = "Output";

/// The button that closes the overlay (Esc).
pub const CLOSE: &str = "Close";

/// Share of the window the overlay takes, as the TUI's 90 x 70 frame.
const SIZE_SHARE: egui::Vec2 = egui::vec2(0.8, 0.7);

/// The overlay's footer: the Close button, its gap and the hint line, which
/// the transcript leaves free.
const FOOTER_HEIGHT: f32 =
    BUTTON_HEIGHT + space::LG + space::MD + crate::metrics::text_size::SMALL + space::XS;

/// What the overlay shows while its target has produced no output.
pub const NO_OUTPUT: &str = "No output yet for this target";

/// The overlay's key legend.
pub const FOOTER_HINT: &str = "PgUp/PgDn scroll * Ctrl+PgUp/PgDn jump * Esc close";

/// What the overlay keeps between frames.
#[derive(Debug, Default)]
struct OutputView {
    /// The virtualised transcript.
    view: MessagesView,
    /// The session and output change key the view was built for.
    built_for: Option<(Option<String>, u64)>,
    /// The `scroll_offset` written back last frame, to tell a key's scroll
    /// from the wheel's.
    last_offset: Option<u16>,
    /// Move the view to app-core's offset this frame (the view was rebuilt).
    force_scroll: bool,
}

/// The overlay's state, kept in egui memory between frames.
type SharedOutputView = std::sync::Arc<std::sync::Mutex<OutputView>>;

/// The egui memory slot of the overlay's state.
fn state_id() -> egui::Id {
    egui::Id::unique("codemason_output_view")
}

/// Forget the overlay's state while it is not shown, so the next overlay
/// starts with a fresh view at its bottom.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
pub fn reset(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<SharedOutputView>(state_id()));
}

/// Draw the output overlay while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `output_view` is drawn.
///
/// # Returns
///
/// `true` while the overlay is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let shared: SharedOutputView = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<SharedOutputView>(state_id())
            .clone()
    });
    let mut state = shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    show_with(ctx, app, &mut state)
}

/// [`show`] with the overlay's state at hand.
fn show_with(ctx: &egui::Context, app: &mut App, state: &mut OutputView) -> bool {
    let Some(content) = app.output_view_content() else {
        return false;
    };
    let key = (content.session_id.clone(), content.generation);
    if !content.focused && state.built_for.as_ref() != Some(&key) {
        // A stored session's messages carry no edit counter: rebuild the
        // view, then put it back where app-core's offset says (the tail
        // when pinned to the bottom).
        state.view = MessagesView::default();
        state.force_scroll = state.built_for.is_some();
    }
    state.built_for = Some(key);
    let offset_lines = app.output_view.as_ref().map_or(0, |v| v.scroll_offset);
    let key_scrolled = state.last_offset.is_some_and(|last| last != offset_lines);
    let follow_app = key_scrolled || std::mem::take(&mut state.force_scroll);
    let palette = palette_for(app, ctx);
    let mut scroll = None;
    let window = ctx.content_rect().size();
    let size = egui::vec2(
        (window.x * SIZE_SHARE.x).max(0.0),
        (window.y * SIZE_SHARE.y).max(0.0),
    );
    material_modal_sized(
        ctx,
        &palette,
        "codemason_output_view",
        &content.title,
        size,
        |ui| {
            crate::material::dialog::dialog_headline(ui, &palette, &content.title);
            let width = ui.available_width();
            let body = (ui.available_height() - FOOTER_HEIGHT).max(0.0);
            if content.messages.is_empty() {
                ui.allocate_ui(egui::vec2(width, body), |ui| {
                    material_hint(ui, &palette, NO_OUTPUT);
                });
            } else {
                ui.allocate_ui(egui::vec2(width, body), |ui| {
                    let row = ui.ctx().fonts_mut(|f| {
                        f.row_height(&egui::FontId::proportional(crate::styled_text::FONT_SIZE))
                    });
                    // As in the TUI, app-core's offset counts lines from the
                    // bottom: 0 pins the view to the tail, so new output stays
                    // in view; any other offset keeps that distance from the
                    // bottom.
                    let scroll_to = follow_app.then(|| {
                        if offset_lines == 0 {
                            ScrollTo::Bottom
                        } else {
                            ScrollTo::FromBottom(f32::from(offset_lines) * row)
                        }
                    });
                    let source = MessageSource {
                        messages: &content.messages,
                        processing: content.focused && app.session.is_processing,
                    };
                    // The node is made before the transcript, so its messages
                    // are placed under it.
                    let node = ui.scope_id().with("output_transcript");
                    ui.interact(ui.available_rect_before_wrap(), node, egui::Sense::hover())
                        .widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Log, true, OUTPUT_LABEL)
                        });
                    let geometry = state
                        .view
                        .show_messages(ui, app, source, &palette, scroll_to, node);
                    scroll = Some(geometry);
                });
            }
            ui.add_space(space::LG);
            material_actions(ui, &palette, &[(CLOSE, plain(Key::Esc))], true);
            ui.add_space(space::MD);
            material_hint(ui, &palette, FOOTER_HINT);
        },
    );
    if let (Some(geometry), Some(view)) = (scroll, app.output_view.as_mut()) {
        let row = geometry.row_height.max(1.0);
        let max_lines = lines(geometry.max_offset / row);
        let from_bottom = lines((geometry.max_offset - geometry.offset).max(0.0) / row);
        view.max_scroll = max_lines;
        view.scroll_offset = from_bottom.min(max_lines);
        state.last_offset = Some(view.scroll_offset);
    }
    // A code block's Insert or an image thumbnail clicked in the overlay.
    state.view.apply_actions(ctx, app);
    true
}

/// A non-negative count of rows as app-core's `u16` line count, rounded to
/// the nearest row and saturating.
fn lines(rows: f32) -> u16 {
    let rounded = rows.round().clamp(0.0, f32::from(u16::MAX));
    // The value is clamped to the u16 range above, so the cast is exact.
    rounded as u16
}
