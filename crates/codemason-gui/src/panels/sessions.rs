//! The sessions sidebar (the Sessions pane), over app-core's
//! `sessions_sidebar_state`.
//!
//! [`SessionsView`] lists the rows app-core builds
//! ([`build_rows`]): `New session` and `Search` (each with its icon and its
//! key on the right), the `Pinned` section and one section per project
//! (each header led by a painted solid triangle, pointing right when
//! collapsed and down when open), each session with its relative age,
//! request count, branch marker and attention marker. The
//! open session is highlighted, and while the sidebar has the keyboard the
//! cursor row is too.
//!
//! Every action is app-core's: a click calls
//! [`click_row`] (switches to the session, opens a new one, starts the
//! filter or toggles a section; `Ctrl`+click toggles the selection) and a
//! right-click calls [`open_row_menu`] (the session or project menu, drawn
//! by `crate::dialogs::session_menu`). Keys reach
//! `sessions_sidebar_state::handle_key` through the key router: `Alt+S`
//! shows, focuses and hides the sidebar, and while it is focused the arrows,
//! `Enter`, `m`, `Shift+F10` and the other sidebar keys act on the cursor
//! row, exactly as in the TUI.
//!
//! The session count is in the dock tab's title ([`tab_title`]), not in the
//! panel.
//!
//! The list is virtualised with `ScrollArea::show_rows` (one text row per
//! sidebar row), so a long session list costs only the visible rows.
//!
//! Dependencies: `codemason-app-core` (sidebar state and actions), `chrono`
//! (relative ages), `egui` and [`crate::palette`].

use codemason_app_core::app::{App, Attention};
use codemason_app_core::geometry::Area;
use codemason_app_core::sessions_sidebar_state::{
    BRANCH_MARK, PINNED_KEY, SidebarRow, attention_of, build_rows, click_row, open_row_menu,
    refresh_if_dirty, relative_age, selectable,
};
use egui::{Atom, Atoms, Color32, IdSalt, IntoAtoms, Rect, RichText, Role, ScrollArea, WidgetInfo};

use crate::icons::{Icon, icon_label};
use crate::palette::Palette;
use crate::styled_text::drawable;

/// Accessible label of the session list.
pub const SESSIONS_LABEL: &str = "Sessions";

/// Scroll state the sidebar keeps between frames.
#[derive(Debug, Default)]
pub struct SessionsView {
    /// The cursor row drawn last frame, to scroll it into view once it moves.
    last_cursor: Option<usize>,
}

/// Marker for an attention state (FR-04, ASCII only), as the TUI draws it.
fn attention_marker(attention: Attention) -> char {
    match attention {
        Attention::Waiting => '!',
        Attention::Running => '*',
        Attention::Error => 'x',
        Attention::Unread => 'o',
        Attention::Idle => ' ',
    }
}

/// The id of a section header's disclosure-triangle slot.
fn disclosure_id() -> IdSalt {
    IdSalt::new("sessions_disclosure")
}

/// The colour of the section header keyed `key`: the Pinned section stands
/// out, project sections are muted.
fn header_colour(key: &str, palette: &Palette) -> Color32 {
    if key == PINNED_KEY {
        palette.warning
    } else {
        palette.muted
    }
}

/// Paint a solid disclosure triangle in `slot`: pointing right while the
/// section is `collapsed`, down while it is open.
fn paint_disclosure(ui: &egui::Ui, slot: Rect, collapsed: bool, colour: Color32) {
    let r = Rect::from_center_size(slot.center(), slot.size() * 0.5);
    let points = if collapsed {
        vec![r.left_top(), r.right_center(), r.left_bottom()]
    } else {
        vec![r.left_top(), r.right_top(), r.center_bottom()]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        colour,
        egui::Stroke::NONE,
    ));
}

/// An action row: `icon`, `label`, then `shortcut` pushed to the right edge,
/// as a menu item draws its accelerator.
fn action_row(
    ui: &egui::Ui,
    icon: Icon,
    label: String,
    shortcut: &str,
    colour: egui::Color32,
    palette: &Palette,
) -> Atoms<'static> {
    let size = egui::TextStyle::Button.resolve(ui.style()).size;
    (
        icon_label(ui.ctx(), icon, size, colour),
        RichText::new(label).color(colour),
        Atom::grow(),
        RichText::new(shortcut.to_string()).color(palette.muted),
    )
        .into_atoms()
}

/// The content of one row.
fn row_text(ui: &egui::Ui, app: &App, row: &SidebarRow, palette: &Palette) -> Atoms<'static> {
    let font = egui::TextStyle::Button.resolve(ui.style());
    // Titles, names and descriptions are user text: draw what the font
    // cannot as ASCII stand-ins.
    let shown = |text: &str| drawable(ui.ctx(), &font, text);
    match row {
        SidebarRow::NewSession => action_row(
            ui,
            Icon::Add,
            "New session".to_string(),
            "Ctrl+N",
            palette.success,
            palette,
        ),
        SidebarRow::Spacer => RichText::new(" ").into_atoms(),
        SidebarRow::Search => {
            let state = &app.sidebar;
            if state.filter_editing || !state.filter.is_empty() {
                let caret = if state.filter_editing { "_" } else { "" };
                let filter = format!("{}{caret}", shown(&state.filter));
                action_row(ui, Icon::Search, filter, "", palette.text, palette)
            } else {
                action_row(
                    ui,
                    Icon::Search,
                    "Search".to_string(),
                    "/",
                    palette.muted,
                    palette,
                )
            }
        }
        SidebarRow::Header {
            key,
            label,
            count,
            collapsed,
            remote,
            description,
        } => {
            // The disclosure triangle is painted into the leading slot
            // (`paint_disclosure`), not drawn as a character.
            let mut text = shown(label);
            if let Some(description) = description {
                text.push_str(&format!(" ({})", shown(description)));
            }
            if *collapsed && *count > 0 {
                text.push_str(&format!(" {count}"));
            }
            if let Some(kind) = remote {
                text.push_str(&format!(" [{kind}]"));
            }
            let size = egui::TextStyle::Button.resolve(ui.style()).size;
            (
                Atom::custom(disclosure_id(), egui::Vec2::splat(size)),
                RichText::new(text).color(header_colour(key, palette)),
            )
                .into_atoms()
        }
        SidebarRow::Session {
            id,
            title,
            updated_at,
            branch,
            requests,
            ..
        } => session_text(
            app,
            id,
            title,
            updated_at,
            branch.as_deref(),
            *requests,
            palette,
        )
        .into_atoms(),
    }
}

/// A session row: selection mark, title, age, request count, branch marker
/// and attention marker, as in the TUI.
fn session_text(
    app: &App,
    id: &str,
    title: &str,
    updated_at: &str,
    branch: Option<&str>,
    requests: Option<i64>,
    palette: &Palette,
) -> RichText {
    let attention = attention_of(app, id);
    let marker = attention_marker(attention);
    let lead = if app.sidebar.selected.contains(id) {
        "* "
    } else {
        "  "
    };
    let mut text = format!(
        "{lead}{title}  {}",
        relative_age(updated_at, chrono::Utc::now())
    );
    let total = app.sidebar.request_totals.get(id).copied().unwrap_or(0);
    match (app.sidebar.bound.get(id), total) {
        (Some(true), n) => text.push_str(&format!(" #!{}", n.max(1))),
        (Some(false), n) => text.push_str(&format!(" #{}", n.max(1))),
        (None, 0) => {}
        (None, n) => text.push_str(&format!(" #{n}")),
    }
    if branch.is_some() {
        match requests.filter(|n| *n > 0) {
            Some(n) => text.push_str(&format!(" {BRANCH_MARK}{n}")),
            None => text.push_str(&format!(" {BRANCH_MARK}")),
        }
    }
    if marker != ' ' {
        text.push_str(&format!(" {marker}"));
    }
    let open = app.session.session_id.as_deref() == Some(id);
    let rich = RichText::new(text);
    if open {
        rich.color(palette.accent).strong()
    } else if matches!(attention, Attention::Unread | Attention::Waiting) {
        rich.color(palette.text).strong()
    } else {
        rich.color(palette.text)
    }
}

/// The Sessions pane's dock tab title: the pane name and the session count,
/// e.g. `Sessions (12)`. The panel itself draws no heading.
///
/// While the sidebar is shown, its data is refreshed first if it is dirty,
/// so the count is current even when another tab is in front and the panel
/// body (which also refreshes) is not drawn.
///
/// # Arguments
///
/// * `app` - the App whose sidebar groups are counted.
///
/// # Returns
///
/// The tab title.
pub fn tab_title(app: &mut App) -> String {
    if app.sidebar.visible {
        refresh_if_dirty(app);
    }
    let total: usize = app.sidebar.groups.iter().map(|g| g.sessions.len()).sum();
    format!("{SESSIONS_LABEL} ({total})")
}

impl SessionsView {
    /// Draw the sidebar into `ui` and run the clicks on it.
    ///
    /// # Arguments
    ///
    /// * `ui` - the Sessions pane's body.
    /// * `app` - the App whose sidebar is drawn; clicks change it through
    ///   app-core's sidebar actions.
    /// * `palette` - the colours to draw with.
    pub fn show(&mut self, ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
        refresh_if_dirty(app);
        let rows = build_rows(app);
        // Rebuilt rows can put a spacer under the cursor; move off it, as
        // the TUI does.
        app.sidebar.cursor = selectable(
            &rows,
            app.sidebar.cursor.min(rows.len().saturating_sub(1)),
            true,
        );
        let row_height = ui.text_style_height(&egui::TextStyle::Button) + 4.0;
        // app-core pages PageUp/PageDown by the painted height in rows, with
        // the border rows of the terminal's box.
        let visible_rows = (ui.available_height() / row_height).floor().max(1.0);
        app.sidebar.area = Area {
            x: 0,
            y: 0,
            width: 0,
            height: u16::try_from(visible_rows as usize)
                .unwrap_or(u16::MAX)
                .saturating_add(2),
        };
        let cursor = app.sidebar.cursor;
        let focused = app.sidebar.focused;
        let mut scroll = ScrollArea::vertical()
            .id_salt("sessions_rows")
            .auto_shrink([false, false]);
        if focused && self.last_cursor != Some(cursor) {
            scroll = scroll_to_row(scroll, ui, cursor, row_height, visible_rows);
        }
        self.last_cursor = focused.then_some(cursor);
        let mut clicked = None;
        let mut menu = None;
        let output = scroll.show_rows(ui, row_height, rows.len(), |ui, range| {
            for index in range {
                let Some(row) = rows.get(index) else {
                    continue;
                };
                if matches!(row, SidebarRow::Spacer) {
                    // A blank line: it takes its row and cannot be clicked.
                    ui.allocate_space(egui::vec2(ui.available_width(), row_height));
                    continue;
                }
                let open = matches!(
                    row,
                    SidebarRow::Session { id, .. } if app.session.session_id.as_deref() == Some(id)
                );
                let highlighted = (focused && index == cursor) || open;
                // Left-aligned like every other list: `add_sized` would
                // centre the label in the full-width row.
                let response =
                    egui::Button::selectable(highlighted, row_text(ui, app, row, palette))
                        .frame_when_inactive(false)
                        .min_size(egui::vec2(ui.available_width(), row_height))
                        // One line per row: `show_rows` and the cursor
                        // scrolling assume a fixed row height.
                        .truncate()
                        .atom_ui(ui);
                if let SidebarRow::Header { key, collapsed, .. } = row {
                    if let Some(slot) = response.rect(disclosure_id()) {
                        paint_disclosure(ui, slot, *collapsed, header_colour(key, palette));
                    }
                    // The triangle is only painted: tell assistive
                    // technology whether the section is open.
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_expanded(!*collapsed);
                    });
                }
                if response.clicked() {
                    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                    clicked = Some((index, ctrl));
                }
                if response.secondary_clicked() {
                    menu = Some(index);
                }
            }
        });
        let list = ui.interact(
            output.inner_rect,
            ui.scope_id().with("sessions_list"),
            egui::Sense::hover(),
        );
        list.widget_info(|| WidgetInfo::labeled(Role::List, true, SESSIONS_LABEL));
        if let Some((index, ctrl)) = clicked {
            click_row(app, index, ctrl);
            ui.ctx().request_repaint();
        }
        if let Some(index) = menu {
            open_row_menu(app, index);
            ui.ctx().request_repaint();
        }
    }
}

/// `scroll` set to bring row `index` into view, given this frame's viewport
/// of `visible_rows` rows of `row_height`.
fn scroll_to_row(
    scroll: ScrollArea,
    ui: &egui::Ui,
    index: usize,
    row_height: f32,
    visible_rows: f32,
) -> ScrollArea {
    let spacing = ui.spacing().item_spacing.y;
    let stride = row_height + spacing;
    let top = index as f32 * stride;
    let id = ui.make_persistent_id("sessions_rows");
    let offset = egui::scroll_area::State::load(ui.ctx(), id).map_or(0.0, |s| s.clamped_offset().y);
    let view = visible_rows * stride;
    if top < offset {
        scroll.vertical_scroll_offset(top)
    } else if top + stride > offset + view {
        scroll.vertical_scroll_offset(top + stride - view)
    } else {
        scroll
    }
}
