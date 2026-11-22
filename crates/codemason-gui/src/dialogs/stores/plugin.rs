//! The Plugin Store dialog's own parts: its copy, a row's tag chips and the
//! plugin detail fields (version, marketplace, tags, source and the
//! homepage, a text button that opens it in the system browser).
//!
//! Dependencies: `egui`, this crate's `material` components and the shared
//! [`super::frame`].

use super::frame::{Look, MAX_ROW_TAGS, detail_field, detail_heading};
use super::snapshot::{Detail, RowSnapshot};
use crate::material::button::{ButtonKind, button};
use crate::material::chip::{ChipKind, chip_dense};
use crate::palette::Palette;

/// The Plugin Store's copy and parts.
pub const LOOK: Look = Look {
    search_placeholder: "Search plugins...",
    loading: "Loading the store index...",
    empty: "The store has no entries",
    row_extras: row_tags,
    details,
};

/// Draw a row's first [`MAX_ROW_TAGS`] tags as dense chips.
///
/// # Arguments
///
/// * `ui` - the row's third line, a horizontal ui.
/// * `palette` - the colours.
/// * `row` - the row.
pub fn row_tags(ui: &mut egui::Ui, palette: &Palette, row: &RowSnapshot) {
    for tag in row.tags.iter().take(MAX_ROW_TAGS) {
        chip_dense(ui, palette, ChipKind::Assist, None, tag);
    }
}

/// Draw a plugin's detail fields.
///
/// # Arguments
///
/// * `ui` - the details pane's ui.
/// * `palette` - the colours.
/// * `row` - the highlighted row; a connector row draws nothing.
pub fn details(ui: &mut egui::Ui, palette: &Palette, row: &RowSnapshot) {
    let detail = match &row.detail {
        Detail::Plugin(detail) => detail,
        Detail::Connector(_) => return,
    };
    detail_field(ui, palette, "Version", &detail.version);
    detail_field(ui, palette, "Marketplace", &detail.marketplace);
    if !row.tags.is_empty() {
        detail_heading(ui, palette, "Tags");
        ui.horizontal_wrapped(|ui| {
            for tag in &row.tags {
                chip_dense(ui, palette, ChipKind::Assist, None, tag);
            }
        });
    }
    detail_field(ui, palette, "Source", &detail.source);
    if let Some(homepage) = &detail.homepage {
        detail_heading(ui, palette, "Homepage");
        if button(ui, palette, ButtonKind::Text, None, homepage).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(homepage));
        }
    }
}
