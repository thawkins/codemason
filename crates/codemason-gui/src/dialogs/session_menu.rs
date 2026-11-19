//! The sidebar's session menu, over `App::sidebar.menu` (and its open
//! `submenu`).
//!
//! app-core runs the menu (`codemason_app_core::session_menu`): Up/Down,
//! Home and End move the highlight, Enter runs the highlighted item (or
//! opens its submenu), Right opens a submenu, Left and Esc close one level,
//! and an item's accelerator letter runs it. Every action calls the same App
//! method as the sidebar's shortcut. The keys reach the App through the
//! router, as in the TUI.
//!
//! The menu is drawn as a popup: a Material menu surface opened at the
//! pointer when a row is right-clicked, or under the highlighted row when
//! the keyboard opened it, with the open submenu beside the item that opened
//! it. The rows show the highlight, the accelerators and the disabled
//! (greyed out) items, with a divider between groups. Clicking an item moves
//! the highlight to it and sends Enter, so app-core runs it; clicking an
//! item of the menu while a submenu is open first closes the submenu
//! (Left), as a click on the menu does in the TUI. A click anywhere outside
//! the menu closes it (Esc for each open level) without acting.
//!
//! The menu is not one of the App's input locks, but it takes every key
//! while open, so the prompt treats it as one
//! ([`super::locks_prompt`]).
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::session_menu::SessionMenu;
use egui::{Color32, CornerRadius, Id, Pos2, Rect, Role, Sense, Vec2, WidgetInfo};

use super::{plain, send_key};
use crate::icons::Icon;
use crate::material::menu::{DENSE_ITEM_HEIGHT, menu_frame};
use crate::material::{PRESS_OPACITY, content_colour, icon_galley, paint_state_layer};
use crate::metrics::{space, text_size};
use crate::palette::{Palette, palette_for, rgb};

/// Accessible label of the menu.
pub const SESSION_MENU_TITLE: &str = "Session menu";

/// Accessible label of the open submenu.
pub const SUBMENU_LABEL: &str = "Submenu";

/// Narrowest menu, in points.
const MIN_WIDTH: f32 = 200.0;

/// Horizontal padding inside an item, in points.
const PADDING: f32 = 12.0;

/// Smallest gap between a label and its accelerator, in points.
const ACCEL_GAP: f32 = 32.0;

/// Size of the chevron on an item that opens a submenu, in points.
const CHEVRON_SIZE: f32 = 18.0;

/// Gap between the menu and its submenu, in points.
const SUBMENU_GAP: f32 = 2.0;

/// Where the open menu is anchored (its top-left corner).
fn anchor_id() -> Id {
    Id::unique("codemason_session_menu_anchor")
}

/// The bottom-left corner of the sidebar's highlighted row, for a menu the
/// keyboard opens.
fn row_anchor_id() -> Id {
    Id::unique("codemason_session_menu_row_anchor")
}

/// Record that the menu was opened by a right-click at `pos`.
pub(crate) fn note_pointer(ctx: &egui::Context, pos: Pos2) {
    ctx.data_mut(|d| d.insert_temp(anchor_id(), pos));
}

/// Record the rect of the sidebar's highlighted row this frame.
pub(crate) fn note_cursor_row(ctx: &egui::Context, rect: Rect) {
    ctx.data_mut(|d| d.insert_temp(row_anchor_id(), rect.left_bottom()));
}

/// Forget the anchor once the menu has closed, so the next opening picks a
/// new one.
pub(crate) fn forget_anchor(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Pos2>(anchor_id()));
}

/// The menu's anchor: the right-click position, else the highlighted row
/// (kept from then on, so the menu does not follow the row's highlight),
/// else the middle of the window.
fn anchor(ctx: &egui::Context) -> Pos2 {
    ctx.data_mut(|d| {
        if let Some(pos) = d.get_temp::<Pos2>(anchor_id()) {
            return pos;
        }
        let pos = d
            .get_temp::<Pos2>(row_anchor_id())
            .unwrap_or_else(|| ctx.content_rect().center());
        d.insert_temp(anchor_id(), pos);
        pos
    })
}

/// Draw `menu`'s items on a menu surface labelled `label`.
///
/// # Returns
///
/// The index of a clicked item, and each item's rect.
fn surface(
    ui: &mut egui::Ui,
    palette: &Palette,
    menu: &SessionMenu,
    label: &str,
) -> (Option<usize>, Vec<Rect>) {
    let mut clicked = None;
    let mut rects = Vec::with_capacity(menu.items.len());
    let width = natural_width(ui, menu);
    let frame = menu_frame(palette).show(ui, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.vertical(|ui| {
            for (i, item) in menu.items.iter().enumerate() {
                if i > 0 && menu.items[i - 1].group != item.group {
                    let y = ui.cursor().top() + space::SM / 2.0;
                    let x = ui.max_rect().x_range();
                    ui.painter()
                        .hline(x, y, (1.0, rgb(palette.m3.outline_variant)));
                    ui.add_space(space::SM);
                }
                let row = ui
                    .add_enabled_ui(item.enabled, |ui| row(ui, palette, item, i == menu.cursor))
                    .inner;
                if row.clicked() {
                    clicked = Some(i);
                }
                rects.push(row.rect);
            }
        });
    });
    frame
        .response
        .widget_info(|| WidgetInfo::labeled(Role::Menu, true, label));
    (clicked, rects)
}

/// The menu's width: its widest label and accelerator with padding, and at
/// least [`MIN_WIDTH`].
fn natural_width(ui: &egui::Ui, menu: &SessionMenu) -> f32 {
    let font = egui::FontId::proportional(text_size::BODY);
    let measure = |text: String| {
        ui.painter()
            .layout_no_wrap(text, font.clone(), Color32::PLACEHOLDER)
            .size()
            .x
    };
    menu.items
        .iter()
        .map(|item| {
            let accel = if item.action.opens_submenu() {
                CHEVRON_SIZE + ACCEL_GAP
            } else {
                item.accel.map_or(0.0, |c| {
                    measure(c.to_ascii_uppercase().to_string()) + ACCEL_GAP
                })
            };
            PADDING.mul_add(2.0, measure(item.label.clone()) + accel)
        })
        .fold(MIN_WIDTH, f32::max)
}

/// Draw one item: its label, its accelerator at the trailing edge, the
/// error colour for a destructive item, and the highlight on the cursor.
fn row(
    ui: &mut egui::Ui,
    palette: &Palette,
    item: &codemason_app_core::session_menu::MenuItem,
    highlighted: bool,
) -> egui::Response {
    let m3 = &palette.m3;
    let base = if item.danger { m3.error } else { m3.on_surface };
    let content = content_colour(ui, palette, rgb(base));
    let variant = content_colour(ui, palette, rgb(m3.on_surface_variant));
    let font = egui::FontId::proportional(text_size::BODY);
    let text = ui
        .painter()
        .layout_no_wrap(item.label.clone(), font.clone(), content);
    // A cascading item shows a chevron where others show their accelerator.
    let trailing = if item.action.opens_submenu() {
        Some(icon_galley(ui, Icon::ChevronRight, CHEVRON_SIZE, variant))
    } else {
        item.accel.map(|c| {
            ui.painter()
                .layout_no_wrap(c.to_ascii_uppercase().to_string(), font.clone(), variant)
        })
    };
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, DENSE_ITEM_HEIGHT), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(Role::MenuItem, ui.is_enabled(), highlighted, &item.label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    if highlighted {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            rgb(m3.on_surface).gamma_multiply(PRESS_OPACITY),
        );
    } else {
        paint_state_layer(ui, &response, CornerRadius::ZERO, content);
    }
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter()
        .galley(egui::pos2(rect.left() + PADDING, y), text, content);
    if let Some(accel) = trailing {
        let pos = egui::pos2(
            rect.right() - PADDING - accel.size().x,
            rect.center().y - accel.size().y / 2.0,
        );
        ui.painter().galley(pos, accel, variant);
    }
    response
}

/// Draw the session menu while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `sidebar.menu` is drawn.
///
/// # Returns
///
/// `true` while the menu is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let palette = palette_for(app, ctx);
    let Some(menu) = app.sidebar.menu.as_ref() else {
        return false;
    };
    // Not `Modal::default_area`: its screen anchor would override the
    // position.
    let area = egui::Area::new(Id::unique("codemason_session_menu"))
        .kind(egui::UiKind::Modal)
        .sense(Sense::hover())
        .order(egui::Order::Foreground)
        .interactable(true)
        .fixed_pos(anchor(ctx))
        .constrain(true);
    let mut top: Option<usize> = None;
    let mut sub: Option<usize> = None;
    let shown = egui::Modal::new(Id::unique("codemason_session_menu"))
        .area(area)
        .frame(egui::Frame::NONE)
        .backdrop_color(Color32::TRANSPARENT)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = SUBMENU_GAP;
                let (clicked, rects) = surface(ui, &palette, menu, SESSION_MENU_TITLE);
                top = clicked;
                if let Some(submenu) = menu.submenu.as_deref() {
                    ui.vertical(|ui| {
                        // Line the submenu's first item up with the item
                        // that opened it.
                        if let (Some(opener), Some(first)) = (rects.get(menu.cursor), rects.first())
                        {
                            ui.add_space(opener.top() - first.top());
                        }
                        sub = surface(ui, &palette, submenu, SUBMENU_LABEL).0;
                    });
                }
            });
        });
    let outside = shown.backdrop_response.clicked() || shown.backdrop_response.secondary_clicked();
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return true;
    };
    if let Some(i) = sub {
        if let Some(submenu) = menu.submenu.as_deref_mut() {
            submenu.cursor = i;
            send_key(ctx, plain(Key::Enter));
        }
    } else if let Some(i) = top {
        menu.cursor = i;
        if menu.submenu.is_some() {
            send_key(ctx, plain(Key::Left));
        }
        send_key(ctx, plain(Key::Enter));
    } else if outside {
        if menu.submenu.is_some() {
            send_key(ctx, plain(Key::Esc));
        }
        send_key(ctx, plain(Key::Esc));
    }
    true
}
