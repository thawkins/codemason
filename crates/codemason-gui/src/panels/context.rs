//! The Context panel (the Context pane): what fills the model's context.
//!
//! The pane is a pinned tab beside the sessions sidebar; `Alt+C` (app-core's
//! `ToggleContextPanel`) brings it to the front or covers it again, and
//! `App::show_context_panel` says whether it is in front. Each frame it
//! reads `App::context_effective_snapshot` (the background-refreshed cache,
//! so a frame never does disk or database work) and draws the view
//! `codemason_app_core::context_panel::context_view` builds:
//!
//! - a header with the share of the window used and `used / window`;
//! - one stacked bar with a coloured segment per partition;
//! - the tokens sent on the last turn;
//! - one row per partition, with its colour swatch, tokens and share of the
//!   window; a row with items (tools, skills, memories, instruction files,
//!   message roles) opens on a click to list them, largest first;
//! - the free space and the stream I/O counters.
//!
//! Which rows are open is kept in egui's persisted memory, so it survives a
//! restart. Drawing never touches the conversation (FR-016). The plain text
//! of the view is cached on `App::context_content_lines`, the copy source of
//! the panel's text selection. The list scrolls to app-core's
//! `context_scroll_offset` (rows from the top) when that changes.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::context_panel::{
    ContextCategory, ContextRow, ContextView, context_view, format_percent, format_tokens,
};
use egui::{Color32, Id, RichText, Role};

use super::rows::label_panel;
use crate::material::feedback::stacked_progress;
use crate::material::list::{DENSE_HEIGHT, Disclosure, dense_item};
use crate::metrics::{space, text_size};
use crate::palette::{Palette, rgb};

/// Accessible label of the Context panel's list.
pub const CONTEXT_LABEL: &str = "Context";

/// Indent of the rows listed under an open row, in points.
const ITEM_INDENT: f32 = 16.0;

/// The swatch and bar colour of `category`.
///
/// # Arguments
///
/// * `category` - the partition.
/// * `palette` - the colours to pick from.
///
/// # Returns
///
/// A colour distinct from the other categories' in the same palette.
#[must_use]
pub fn category_colour(category: ContextCategory, palette: &Palette) -> Color32 {
    match category {
        ContextCategory::History => palette.info,
        ContextCategory::SystemPrompt => palette.accent,
        ContextCategory::Skills => palette.syntax.function,
        ContextCategory::Memory => palette.success,
        ContextCategory::Instructions => palette.syntax.string,
        ContextCategory::BuiltInTools => palette.warning,
        ContextCategory::McpTools => palette.syntax.keyword,
        ContextCategory::ToolMetadata => palette.muted,
    }
}

/// The persisted-memory id of whether `category`'s row is open.
fn open_id(category: ContextCategory) -> Id {
    Id::unique(("codemason_context_row_open", category.label()))
}

/// Draw the header: the caption, the usage summary and the stacked bar.
fn header(ui: &mut egui::Ui, view: &ContextView, palette: &Palette) {
    let variant = rgb(palette.m3.on_surface_variant);
    ui.label(
        RichText::new("Context window")
            .size(text_size::SMALL)
            .color(variant),
    );
    ui.horizontal(|ui| {
        let used = view.used_percent.map_or_else(
            || format_tokens(view.used_tokens),
            |pct| format!("{pct:.0}%"),
        );
        ui.label(
            RichText::new(format!("{used} used"))
                .size(text_size::BODY)
                .strong()
                .color(rgb(palette.m3.on_surface)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let window = view
                .window_tokens
                .map_or_else(|| "window unknown".to_string(), format_tokens);
            ui.label(
                RichText::new(format!("{} / {window}", format_tokens(view.used_tokens)))
                    .size(text_size::SMALL)
                    .color(variant),
            );
        });
    });
    // Without a window the segments share the used tokens out instead.
    let whole = view.window_tokens.unwrap_or(view.used_tokens).max(1) as f32;
    let segments: Vec<(f32, Color32)> = view
        .rows
        .iter()
        .map(|row| {
            (
                row.tokens as f32 / whole,
                category_colour(row.category, palette),
            )
        })
        .collect();
    stacked_progress(ui, palette, &segments, &view.usage_text());
}

/// Draw `row` and, when it is open, the items under it.
fn partition(ui: &mut egui::Ui, view: &ContextView, row: &ContextRow, palette: &Palette) {
    let id = open_id(row.category);
    let open = ui
        .ctx()
        .data_mut(|d| *d.get_persisted_mut_or_default::<bool>(id));
    let disclosure = match (row.items.is_empty(), open) {
        (true, _) => Disclosure::None,
        (false, false) => Disclosure::Collapsed,
        (false, true) => Disclosure::Expanded,
    };
    let tokens = format_tokens(row.tokens);
    let percent = format_percent(row.percent);
    let response = dense_item(
        ui,
        palette,
        0.0,
        disclosure,
        Some(category_colour(row.category, palette)),
        row.category.label(),
        &[&tokens, &percent],
    );
    if response.clicked() {
        ui.ctx().data_mut(|d| d.insert_persisted(id, !open));
    }
    if row.category == ContextCategory::History {
        dense_item(
            ui,
            palette,
            ITEM_INDENT,
            Disclosure::None,
            None,
            &view.messages_text(),
            &[],
        );
    }
    if disclosure != Disclosure::Expanded {
        return;
    }
    for item in &row.items {
        let tokens = format_tokens(item.tokens);
        dense_item(
            ui,
            palette,
            ITEM_INDENT,
            Disclosure::None,
            None,
            &item.name,
            &[&tokens, ""],
        );
    }
}

/// Draw the rows under the header: the sent tokens, every partition, the
/// free space and the stream counters.
fn body(ui: &mut egui::Ui, view: &ContextView, palette: &Palette) {
    let sent = format_tokens(view.sent_tokens);
    dense_item(
        ui,
        palette,
        0.0,
        Disclosure::None,
        None,
        "Sent to model",
        &[&sent, ""],
    );
    for row in &view.rows {
        partition(ui, view, row, palette);
    }
    ui.separator();
    let free = view
        .free_tokens
        .map_or_else(|| "unknown".to_string(), format_tokens);
    let free_percent = format_percent(view.free_percent);
    dense_item(
        ui,
        palette,
        0.0,
        Disclosure::None,
        Some(rgb(palette.m3.secondary_container)),
        "Free space",
        &[&free, &free_percent],
    );
    let streamed = format!("Streamed  {}", view.streamed_text());
    dense_item(ui, palette, 0.0, Disclosure::None, None, &streamed, &[]);
}

/// Draw the Context panel into `ui`.
///
/// # Arguments
///
/// * `ui` - the Context pane's body.
/// * `app` - the App whose context snapshot is drawn; its
///   `context_content_lines` copy cache is refreshed.
/// * `palette` - the colours to draw with.
pub fn context_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    let snapshot = app.context_effective_snapshot();
    let view = context_view(
        &snapshot,
        app.session.stream_out_bytes,
        app.session.stream_in_bytes,
    );
    app.context_content_lines = view.text_lines();

    // Follow app-core's scroll offset only when it moves, so the wheel and
    // the scroll bar stay egui's.
    let offset_id = Id::unique("codemason_context_scroll_offset");
    let offset = app.context_scroll_offset;
    let moved = ui.ctx().data_mut(|d| {
        let last = d.get_temp_mut_or_insert_with(offset_id, || u16::MAX);
        let moved = *last != offset;
        *last = offset;
        moved
    });
    let mut area = egui::ScrollArea::vertical()
        .id_salt("context_rows")
        .auto_shrink([false, false]);
    if moved {
        area = area.vertical_scroll_offset(f32::from(offset) * DENSE_HEIGHT);
    }
    let output = area.show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = space::XS / 2.0;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(space::SM as i8, space::SM as i8))
            .show(ui, |ui| {
                header(ui, &view, palette);
                ui.add_space(space::SM);
                body(ui, &view, palette);
            });
    });
    label_panel(
        ui,
        output.inner_rect,
        "context_list",
        Role::List,
        CONTEXT_LABEL,
    );
}
