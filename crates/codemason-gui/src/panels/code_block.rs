//! A code block in the transcript.
//!
//! [`code_block_ui`] draws a [`TranscriptBlock::Code`] as a raised card: a
//! header with the language and the [`COPY`], [`WRAP`] and [`INSERT`]
//! buttons, and the code below it, highlighted by
//! [`codemason_app_core::syntax_highlight`] in the theme's `syntax.*`
//! colours and selectable. Long lines scroll sideways unless Wrap is on; the
//! choice is kept per block for the session.
//!
//! The buttons are click-only, so they never hold the keyboard focus and
//! cannot take an Enter meant for the prompt.
//!
//! [`TranscriptBlock::Code`]: codemason_app_core::message_blocks::TranscriptBlock::Code
//!
//! Dependencies: `egui`, `codemason-app-core` (highlighting),
//! [`crate::palette`] and [`crate::metrics`].

use codemason_app_core::syntax_highlight::highlight;
use egui::text::LayoutJob;
use egui::{FontId, RichText, TextFormat};

use crate::metrics::{radius, space, text_size};
use crate::palette::Palette;
use crate::styled_text::FONT_SIZE;

/// The button that copies the code.
pub const COPY: &str = "Copy";

/// The toggle that wraps long lines.
pub const WRAP: &str = "Wrap";

/// The button that inserts the code into the prompt.
pub const INSERT: &str = "Insert";

/// The header's language label when the fence names none.
pub const NO_LANGUAGE: &str = "code";

/// What the user asked a code block to do this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeAction {
    /// Insert this code into the prompt.
    Insert(String),
}

/// The highlighted layout of `code` in `lang`, in the palette's colours.
///
/// Unsupported languages, and code too long to highlight, are drawn in the
/// body text colour.
#[must_use]
pub fn code_job(code: &str, lang: Option<&str>, palette: &Palette) -> LayoutJob {
    let font = FontId::monospace(FONT_SIZE);
    let format = |color| TextFormat::simple(font.clone(), color);
    let mut job = LayoutJob::default();
    match lang.and_then(|lang| highlight(code, lang)) {
        Some(spans) => {
            for span in spans {
                let color = span
                    .role
                    .map_or(palette.text, |role| palette.syntax.color(role));
                job.append(&code[span.range], 0.0, format(color));
            }
        }
        None => job.append(code, 0.0, format(palette.text)),
    }
    job
}

/// Draw one code block.
///
/// # Arguments
///
/// * `ui` - where to draw; the card fills its width.
/// * `id` - a stable id for the block (its wrap choice is kept under it).
/// * `lang` - the language after the fence, if any.
/// * `code` - the code.
/// * `palette` - the active palette.
///
/// # Returns
///
/// The action the user took, if any. Copy is done here.
pub fn code_block_ui(
    ui: &mut egui::Ui,
    id: egui::Id,
    lang: Option<&str>,
    code: &str,
    palette: &Palette,
) -> Option<CodeAction> {
    let wrap_id = id.with("wrap");
    let mut wrap = ui.data(|d| d.get_temp::<bool>(wrap_id).unwrap_or(false));
    let mut action = None;
    egui::Frame::NONE
        .fill(palette.raised1)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(radius::SM)
        .inner_margin(space::SM)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(lang.unwrap_or(NO_LANGUAGE))
                        .size(text_size::SMALL)
                        .color(palette.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = |text: &str| {
                        egui::Button::new(RichText::new(text).size(text_size::SMALL))
                            .sense(egui::Sense::CLICK)
                    };
                    if ui.add(button(INSERT)).clicked() {
                        action = Some(CodeAction::Insert(code.to_string()));
                    }
                    let toggle = button(WRAP).selected(wrap);
                    if ui.add(toggle).clicked() {
                        wrap = !wrap;
                        ui.data_mut(|d| d.insert_temp(wrap_id, wrap));
                    }
                    if ui.add(button(COPY)).clicked() {
                        ui.ctx().copy_text(code.to_string());
                    }
                });
            });
            ui.add_space(space::XS);
            let job = code_job(code, lang, palette);
            if wrap {
                ui.add(egui::Label::new(job).wrap());
            } else {
                egui::ScrollArea::horizontal()
                    .id_salt(id.with("scroll"))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(job).extend());
                    });
            }
        });
    action
}
