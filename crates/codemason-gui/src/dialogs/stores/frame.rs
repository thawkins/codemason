//! The shared frame of the store dialogs: one wide Material dialog with a
//! header bar, a three-column body (filter rail, result list, details) and
//! a footer bar.
//!
//! [`dialog_size`] is the size rule (85% of the window, at least 960 x 600,
//! never past the window). [`draw`] lays out one [`StoreSnapshot`]: the
//! header (title, the Codex | Claude switcher when the snapshot is a plugin
//! store, the search field showing the live query, close), the body at a
//! fixed height with its own scroll area per column, and the footer (the
//! shown count and the search on the left, the last install result and
//! Refresh on the right). The focused pane carries a 2-point `primary`
//! outline. While the index loads, is empty or failed, a status replaces
//! the result list. A [`Look`] supplies the per-dialog copy and the parts
//! only one dialog has (the row's third line and the detail fields).
//!
//! Dependencies: `egui` and this crate's `material` components and palette.

use egui::{
    Align, CornerRadius, Label, Layout, Rect, RichText, Role, ScrollArea, Sense, Stroke,
    StrokeKind, UiBuilder, Vec2, WidgetInfo,
};

use super::snapshot::{FilterRow, ListState, Pane, RowSnapshot, StoreSnapshot};
use super::{
    CATEGORIES_LABEL, CLEAR_SEARCH, CLOSE, Click, DETAILS_LABEL, INSTALL, INSTALLED, KIND_OPTIONS,
    MARKETPLACES_LABEL, PANES_TOOLTIP, REFRESH, RESULTS_LABEL, RETRY, SEARCH_TOOLTIP, UNINSTALL,
};
use crate::dialogs::material_modal_sized;
use crate::icons::Icon;
use crate::material::badge::{BadgeTone, badge};
use crate::material::button::{ButtonKind, button};
use crate::material::feedback::{linear_progress, plain_tooltip};
use crate::material::icon_button::{IconButtonKind, icon_button, icon_button_dense};
use crate::material::list::{AvatarItem, avatar_list_item};
use crate::material::segmented::{SegmentMark, segmented_buttons_with};
use crate::material::sheet::drawer_item_with_count;
use crate::material::text_field::outlined_value;
use crate::metrics::{radius, space, text_size};
use crate::palette::{Palette, rgb};
use codemason_plugins::StoreKind;

/// The smallest dialog, in points, unless the window is smaller.
pub const MIN_SIZE: Vec2 = Vec2::new(960.0, 600.0);

/// The share of the window the dialog takes.
pub const SIZE_FRACTION: f32 = 0.85;

/// Width of the filter rail, in points.
pub const RAIL_WIDTH: f32 = 216.0;

/// Text size of a rail section's heading (Label Small, 20% larger).
pub const RAIL_HEADING_SIZE: f32 = text_size::SMALL * 1.2;

/// Share of the rail's height the marketplace section takes.
pub const MARKETPLACE_SHARE: f32 = 0.5;

/// Width of the details pane, in points (less in a narrow dialog).
pub const DETAILS_WIDTH: f32 = 300.0;

/// Height of a result row, in points.
pub const ROW_HEIGHT: f32 = crate::material::list::AVATAR_ITEM_HEIGHT;

/// The most tag chips a result row shows.
pub const MAX_ROW_TAGS: usize = 3;

/// The status text when the filters match nothing.
pub const NO_MATCHES: &str = "No matches";

/// Height of the header bar, in points (one outlined text field).
const HEADER_HEIGHT: f32 = 56.0;

/// Height of the footer bar, in points (one button).
const FOOTER_HEIGHT: f32 = 40.0;

/// Gap between the bars and columns, in points.
const GAP: f32 = space::SM;

/// Room the gaps between the three bars (or columns) take, in points.
const TWO_GAPS: f32 = 2.0 * GAP;

/// Width of the focused pane's outline, in points.
const FOCUS_STROKE: f32 = 2.0;

/// Widest the search field grows, in points.
const SEARCH_WIDTH: f32 = 360.0;

/// The share of a narrow dialog's width the details pane may take.
const DETAILS_SHARE: f32 = 0.3;

/// What differs between the store dialogs.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// The search field's label while it is empty.
    pub search_placeholder: &'static str,
    /// The status text while the index loads.
    pub loading: &'static str,
    /// The status text when the index has no entries.
    pub empty: &'static str,
    /// Draws a row's third line (tags, badges), in a horizontal ui.
    pub row_extras: fn(&mut egui::Ui, &Palette, &RowSnapshot),
    /// Draws the details pane's fields between the description and Install.
    pub details: fn(&mut egui::Ui, &Palette, &RowSnapshot),
}

/// The dialog's content size for a window of `window` points.
///
/// # Arguments
///
/// * `window` - the window's content size.
///
/// # Returns
///
/// [`SIZE_FRACTION`] of the window, at least [`MIN_SIZE`], and never more
/// than the window.
#[must_use]
pub fn dialog_size(window: Vec2) -> Vec2 {
    (window * SIZE_FRACTION).max(MIN_SIZE).min(window)
}

/// Draw `snap` as a store dialog.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `palette` - the colours.
/// * `id` - the dialog's egui id.
/// * `snap` - what to draw.
/// * `look` - the dialog's copy and own parts.
///
/// # Returns
///
/// What was clicked, if anything.
pub fn draw(
    ctx: &egui::Context,
    palette: &Palette,
    id: &str,
    snap: &StoreSnapshot,
    look: &Look,
) -> Option<Click> {
    let size = dialog_size(ctx.content_rect().size());
    let mut click = None;
    material_modal_sized(ctx, palette, id, snap.title, size, |ui| {
        let width = ui.available_width();
        let height = ui.available_height();
        ui.spacing_mut().item_spacing = Vec2::splat(GAP);
        header(ui, palette, snap, look, width, &mut click);
        let body_height = (height - HEADER_HEIGHT - FOOTER_HEIGHT - TWO_GAPS).max(0.0);
        body(
            ui,
            palette,
            id,
            snap,
            look,
            Vec2::new(width, body_height),
            &mut click,
        );
        footer(ui, palette, snap, width, &mut click);
    });
    click
}

/// Keep `found` in `click` when there is one.
fn set(click: &mut Option<Click>, found: Option<Click>) {
    if found.is_some() {
        *click = found;
    }
}

/// The switcher segment of `kind`.
const fn kind_index(kind: StoreKind) -> usize {
    match kind {
        StoreKind::Codex => 0,
        StoreKind::Claude => 1,
    }
}

/// The kind of switcher segment `index`.
const fn kind_at(index: usize) -> StoreKind {
    if index == 1 {
        StoreKind::Claude
    } else {
        StoreKind::Codex
    }
}

/// The header bar: title, switcher, search field, clear search and close.
fn header(
    ui: &mut egui::Ui,
    palette: &Palette,
    snap: &StoreSnapshot,
    look: &Look,
    width: f32,
    click: &mut Option<Click>,
) {
    let m3 = &palette.m3;
    ui.allocate_ui_with_layout(
        Vec2::new(width, HEADER_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_height(HEADER_HEIGHT);
            ui.label(
                RichText::new(snap.title)
                    .size(text_size::TITLE)
                    .color(rgb(m3.on_surface)),
            );
            if let Some(kind) = snap.plugin_kind {
                let mut selected = kind_index(kind);
                if segmented_buttons_with(
                    ui,
                    palette,
                    &KIND_OPTIONS,
                    &mut selected,
                    SegmentMark::Highlight,
                )
                .changed()
                {
                    *click = Some(Click::SwitchKind(kind_at(selected)));
                }
            }
            let clear_width = if snap.query.is_empty() {
                0.0
            } else {
                crate::material::icon_button::DENSE_SIZE + GAP
            };
            let search_width = (ui.available_width()
                - crate::material::icon_button::SIZE
                - clear_width
                - TWO_GAPS)
                .clamp(0.0, SEARCH_WIDTH);
            let search = ui
                .allocate_ui(Vec2::new(search_width, HEADER_HEIGHT), |ui| {
                    outlined_value(
                        ui,
                        palette,
                        look.search_placeholder,
                        &snap.query,
                        true,
                        None,
                        false,
                    )
                })
                .inner;
            plain_tooltip(search, palette, SEARCH_TOOLTIP);
            if !snap.query.is_empty()
                && icon_button_dense(
                    ui,
                    palette,
                    IconButtonKind::Standard,
                    Icon::Cross,
                    CLEAR_SEARCH,
                    None,
                )
                .clicked()
            {
                *click = Some(Click::ClearSearch);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(
                    ui,
                    palette,
                    IconButtonKind::Standard,
                    Icon::Cross,
                    CLOSE,
                    None,
                )
                .clicked()
                {
                    *click = Some(Click::Close);
                }
            });
        },
    );
}

/// The body: the filter rail, the result list and the details pane, each a
/// column of `size.y` points with its own scroll area.
#[allow(clippy::too_many_arguments)] // The frame's parts, passed down once.
fn body(
    ui: &mut egui::Ui,
    palette: &Palette,
    id: &str,
    snap: &StoreSnapshot,
    look: &Look,
    size: Vec2,
    click: &mut Option<Click>,
) {
    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Min), |ui| {
        ui.set_height(size.y);
        let details_width = DETAILS_WIDTH.min(size.x * DETAILS_SHARE);
        let list_width = (size.x - RAIL_WIDTH - details_width - TWO_GAPS).max(0.0);
        let m3 = &palette.m3;
        column(
            ui,
            Vec2::new(RAIL_WIDTH, size.y),
            rgb(m3.surface_container),
            |ui| set(click, rail(ui, palette, id, snap)),
        );
        column(ui, Vec2::new(list_width, size.y), rgb(m3.surface), |ui| {
            set(click, results(ui, palette, id, snap, look))
        });
        column(
            ui,
            Vec2::new(details_width, size.y),
            rgb(m3.surface_container_low),
            |ui| set(click, details(ui, palette, id, snap, look)),
        );
    });
}

/// Allocate a `size` column filled with `fill` and draw `add_contents` in
/// it, clipped to it.
fn column(
    ui: &mut egui::Ui,
    size: Vec2,
    fill: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(radius::MD), fill);
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    add_contents(&mut child);
}

/// Outline `rect` as the focused pane.
fn focus_outline(ui: &egui::Ui, palette: &Palette, rect: Rect) {
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(radius::MD),
        Stroke::new(FOCUS_STROKE, rgb(palette.m3.primary)),
        StrokeKind::Inside,
    );
}

/// The filter rail: the category section over the marketplace section,
/// the marketplaces fixed at [`MARKETPLACE_SHARE`] of the height, each
/// scrolling on its own.
fn rail(ui: &mut egui::Ui, palette: &Palette, id: &str, snap: &StoreSnapshot) -> Option<Click> {
    let full = ui.max_rect();
    let split = full.height().mul_add(-MARKETPLACE_SHARE, full.max.y);
    let top = Rect::from_min_max(full.min, egui::pos2(full.max.x, split));
    let bottom = Rect::from_min_max(egui::pos2(full.min.x, split), full.max);
    let categories = filter_section(
        ui,
        palette,
        (id, "store_rail_categories"),
        top,
        CATEGORIES_LABEL,
        &snap.categories,
        snap.category_cursor,
        snap.focus == Pane::Categories,
    );
    let marketplaces = filter_section(
        ui,
        palette,
        (id, "store_rail_marketplaces"),
        bottom,
        MARKETPLACES_LABEL,
        &snap.marketplaces,
        snap.marketplace_cursor,
        snap.focus == Pane::Marketplaces,
    );
    ui.advance_cursor_after_rect(full);
    marketplaces
        .map(Click::Marketplace)
        .or(categories.map(Click::Category))
}

/// One rail section drawn in `rect`: a reversed heading band over its own
/// scroll area of drawer items, as a list labelled `label`; returns the
/// clicked row.
#[allow(clippy::too_many_arguments)] // The section's parts, passed down once.
fn filter_section(
    ui: &mut egui::Ui,
    palette: &Palette,
    salt: (&str, &str),
    rect: Rect,
    label: &str,
    rows: &[FilterRow],
    cursor: usize,
    focused: bool,
) -> Option<usize> {
    let mut section = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    section.set_clip_rect(rect.intersect(ui.clip_rect()));
    section.spacing_mut().item_spacing.y = 0.0;
    let inner = section.scope(|ui| {
        section_heading(ui, palette, label);
        let mut picked = None;
        ScrollArea::vertical()
            .id_salt(salt)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(space::XS);
                for (i, row) in rows.iter().enumerate() {
                    if drawer_item_with_count(ui, palette, &row.label, row.count, i == cursor)
                        .clicked()
                    {
                        picked = Some(i);
                    }
                }
            });
        picked
    });
    inner
        .response
        .widget_info(|| WidgetInfo::labeled(Role::List, true, label));
    if focused {
        focus_outline(ui, palette, rect);
    }
    inner.inner
}

/// A rail section's heading: `label` in reversed colours (the surface
/// colour on an on-surface band) at [`RAIL_HEADING_SIZE`], full width.
fn section_heading(ui: &mut egui::Ui, palette: &Palette, label: &str) {
    let m3 = &palette.m3;
    let height = space::SM.mul_add(2.0, RAIL_HEADING_SIZE);
    let (band, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter().rect_filled(
        band,
        CornerRadius {
            nw: radius::MD,
            ne: radius::MD,
            sw: 0,
            se: 0,
        },
        rgb(m3.on_surface),
    );
    ui.painter().text(
        egui::pos2(band.min.x + space::LG, band.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(RAIL_HEADING_SIZE),
        rgb(m3.surface),
    );
    plain_tooltip(response, palette, PANES_TOOLTIP);
}

/// The result list, or the status that takes its place.
fn results(
    ui: &mut egui::Ui,
    palette: &Palette,
    id: &str,
    snap: &StoreSnapshot,
    look: &Look,
) -> Option<Click> {
    let mut click = None;
    let inner = ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let scroll = ScrollArea::vertical()
            .id_salt((id, "store_results"))
            .auto_shrink([false, false]);
        if matches!(snap.state, ListState::Ready) && !snap.rows.is_empty() {
            // Only the rows in view are laid out: drawing every row each
            // frame made a large index lag behind the pointer.
            scroll.show_rows(ui, ROW_HEIGHT, snap.rows.len(), |ui, range| {
                set(&mut click, rows(ui, palette, id, snap, look, range));
            });
            return;
        }
        scroll.show(ui, |ui| match &snap.state {
            ListState::Loading => {
                status(ui, palette, look.loading, true);
            }
            ListState::Empty => status(ui, palette, look.empty, false),
            ListState::Failed(cause) => {
                if failed(ui, palette, cause) {
                    click = Some(Click::Retry);
                }
            }
            ListState::Ready if snap.rows.is_empty() => {
                status(ui, palette, NO_MATCHES, false);
            }
            ListState::Ready => {}
        });
    });
    inner
        .response
        .widget_info(|| WidgetInfo::labeled(Role::List, true, RESULTS_LABEL));
    if snap.focus == Pane::Results {
        focus_outline(ui, palette, inner.response.rect);
    }
    click
}

/// A status line (with an indeterminate progress bar while `loading`).
fn status(ui: &mut egui::Ui, palette: &Palette, text: &str, loading: bool) {
    if loading {
        linear_progress(ui, palette, None);
    }
    ui.add_space(space::LG);
    ui.horizontal(|ui| {
        ui.add_space(space::LG);
        ui.label(RichText::new(text).color(rgb(palette.m3.on_surface_variant)));
    });
}

/// The failed status: the cause and Retry; returns whether Retry was
/// pressed.
fn failed(ui: &mut egui::Ui, palette: &Palette, cause: &str) -> bool {
    ui.add_space(space::LG);
    let mut pressed = false;
    ui.horizontal(|ui| {
        ui.add_space(space::LG);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = GAP;
            ui.add(Label::new(RichText::new(cause).color(rgb(palette.m3.error))).wrap());
            pressed = button(ui, palette, ButtonKind::Tonal, None, RETRY).clicked();
        });
    });
    pressed
}

/// The result rows in `range` (the ones in view); the highlighted row is
/// scrolled into view when the highlight moved, even when it lies outside
/// `range`, since every row is [`ROW_HEIGHT`] points high.
fn rows(
    ui: &mut egui::Ui,
    palette: &Palette,
    id: &str,
    snap: &StoreSnapshot,
    look: &Look,
    range: std::ops::Range<usize>,
) -> Option<Click> {
    let cursor_key = egui::Id::unique((id, "store_cursor"));
    let last_cursor = ui.data(|d| d.get_temp::<usize>(cursor_key));
    if last_cursor != Some(snap.cursor) && snap.cursor < snap.rows.len() {
        // `show_rows` starts the ui at row `range.start`.
        let offset = (snap.cursor as f32 - range.start as f32) * ROW_HEIGHT;
        let top = ui.cursor().top() + offset;
        let rect = Rect::from_min_size(
            egui::pos2(ui.max_rect().left(), top),
            Vec2::new(ui.available_width(), ROW_HEIGHT),
        );
        ui.scroll_to_rect(rect, None);
    }
    let mut click = None;
    for index in range {
        let Some(row) = snap.rows.get(index) else {
            break;
        };
        let selected = index == snap.cursor;
        set(
            &mut click,
            result_row(ui, palette, look, row, index, selected),
        );
    }
    ui.data_mut(|d| d.insert_temp(cursor_key, snap.cursor));
    click
}

/// One result row: a material avatar list item with the Installed badge and
/// the dialog's extras on its meta line, and Install (or Uninstall
/// when installed) in its trailing slot.
///
/// # Returns
///
/// What was clicked.
fn result_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    look: &Look,
    row: &RowSnapshot,
    index: usize,
    selected: bool,
) -> Option<Click> {
    let item = AvatarItem {
        headline: &row.name,
        subtitle: &row.subtitle,
        supporting: &row.description,
        selected,
    };
    let (response, pressed) = avatar_list_item(
        ui,
        palette,
        &item,
        |ui| {
            if row.installed {
                badge(ui, palette, BadgeTone::Success, INSTALLED);
            }
            (look.row_extras)(ui, palette, row);
        },
        |ui| {
            if row.installed {
                button(ui, palette, ButtonKind::Outlined, None, UNINSTALL).clicked()
            } else {
                // The details pane's Install is the view's one filled
                // (primary) action; the row's stays tonal.
                button(ui, palette, ButtonKind::Tonal, None, INSTALL).clicked()
            }
        },
    );
    if pressed && row.installed {
        Some(Click::RowUninstall(index))
    } else if pressed {
        Some(Click::RowInstall(index))
    } else if response.double_clicked() {
        Some(Click::Result {
            index,
            install: true,
        })
    } else if response.clicked() {
        Some(Click::Result {
            index,
            install: false,
        })
    } else {
        None
    }
}

/// The details pane: the highlighted entry in full and Install (or
/// Uninstall when installed).
fn details(
    ui: &mut egui::Ui,
    palette: &Palette,
    id: &str,
    snap: &StoreSnapshot,
    look: &Look,
) -> Option<Click> {
    let m3 = &palette.m3;
    let row = match snap.state {
        ListState::Ready => snap.rows.get(snap.cursor),
        ListState::Loading | ListState::Empty | ListState::Failed(_) => None,
    };
    let mut click = None;
    let inner = ui.scope(|ui| {
        ScrollArea::vertical()
            .id_salt((id, "store_details"))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(space::LG as i8))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = space::XS;
                        if let Some(row) = row {
                            ui.add(
                                Label::new(
                                    RichText::new(&row.name)
                                        .size(text_size::TITLE)
                                        .color(rgb(m3.on_surface)),
                                )
                                .wrap(),
                            );
                            ui.add(
                                Label::new(
                                    RichText::new(&row.subtitle)
                                        .size(text_size::SMALL)
                                        .color(rgb(m3.on_surface_variant)),
                                )
                                .wrap(),
                            );
                            ui.add_space(space::SM);
                            ui.add(
                                Label::new(
                                    RichText::new(&row.description).color(rgb(m3.on_surface)),
                                )
                                .wrap(),
                            );
                            (look.details)(ui, palette, row);
                        }
                        ui.add_space(space::LG);
                        let installed = row.is_some_and(|row| row.installed);
                        if installed {
                            if button(ui, palette, ButtonKind::Outlined, None, UNINSTALL).clicked()
                            {
                                click = Some(Click::Uninstall);
                            }
                        } else {
                            let pressed = ui
                                .add_enabled_ui(row.is_some(), |ui| {
                                    button(ui, palette, ButtonKind::Filled, None, INSTALL)
                                })
                                .inner
                                .clicked();
                            if pressed {
                                click = Some(Click::Install);
                            }
                        }
                    });
            });
    });
    inner
        .response
        .widget_info(|| WidgetInfo::labeled(Role::Group, true, DETAILS_LABEL));
    click
}

/// A detail field: its name in small muted type over its wrapped value.
///
/// # Arguments
///
/// * `ui` - the details pane's ui.
/// * `palette` - the colours.
/// * `name` - the field's name.
/// * `value` - the field's value; a blank value draws nothing.
pub fn detail_field(ui: &mut egui::Ui, palette: &Palette, name: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    detail_heading(ui, palette, name);
    ui.add(Label::new(RichText::new(value).color(rgb(palette.m3.on_surface))).wrap());
}

/// A detail section's name, in small muted type after a gap.
///
/// # Arguments
///
/// * `ui` - the details pane's ui.
/// * `palette` - the colours.
/// * `name` - the section's name.
pub fn detail_heading(ui: &mut egui::Ui, palette: &Palette, name: &str) {
    ui.add_space(space::SM);
    ui.label(
        RichText::new(name)
            .size(text_size::SMALL)
            .color(rgb(palette.m3.on_surface_variant)),
    );
}

/// The footer bar: the shown count and search, the refresh note and last
/// install result ([`StoreSnapshot::footer`]) and Refresh.
fn footer(
    ui: &mut egui::Ui,
    palette: &Palette,
    snap: &StoreSnapshot,
    width: f32,
    click: &mut Option<Click>,
) {
    let muted = rgb(palette.m3.on_surface_variant);
    ui.allocate_ui_with_layout(
        Vec2::new(width, FOOTER_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_height(FOOTER_HEIGHT);
            // The count describes the result list, so it carries the panes'
            // keyboard hint (a tooltip over a whole pane would follow the
            // pointer everywhere in it).
            let count = ui.label(
                RichText::new(footer_left(snap))
                    .size(text_size::SMALL)
                    .color(muted),
            );
            plain_tooltip(count, palette, PANES_TOOLTIP);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, palette, ButtonKind::Text, None, REFRESH).clicked() {
                    *click = Some(Click::Refresh);
                }
                if let Some(text) = &snap.footer {
                    ui.add(
                        Label::new(RichText::new(text).size(text_size::SMALL).color(muted))
                            .truncate(),
                    );
                }
            });
        },
    );
}

/// The footer's left text: `"{shown} of {total}"`, and the search when
/// there is one.
///
/// # Arguments
///
/// * `snap` - the snapshot.
///
/// # Returns
///
/// For example `3 of 12 - search "git"`.
#[must_use]
pub fn footer_left(snap: &StoreSnapshot) -> String {
    let mut text = format!("{} of {}", snap.shown, snap.total);
    if !snap.query.is_empty() {
        text.push_str(&format!(" - search \"{}\"", snap.query));
    }
    text
}
