//! The sessions sidebar (the Sessions pane), over app-core's
//! `sessions_sidebar_state`.
//!
//! [`SessionsView`] lists the rows app-core builds
//! ([`build_rows`]): `New session` and `Search` (each with its icon and its
//! key on the right), the `Pinned` section and one section per project
//! (each a small muted heading followed by a painted solid triangle,
//! pointing right when collapsed and down when open), and each session as
//! its title, an attention dot while it needs a look, then two fixed
//! columns on the right: a pull-request icon with the session's request
//! count (always shown, `0` when it has none; see [`PrBadge`]) and the icon
//! of the execution backend it runs on ([`backend_name`]: local computer,
//! Docker or Podman container, remote system). Hovering a session shows
//! its age, backend, branch and requests. The open session is highlighted,
//! and while the sidebar has the keyboard the cursor row is too; sessions in
//! the multi-selection carry an accent bar on their left edge.
//!
//! The row icons are painted (like the disclosure triangle) rather than
//! drawn from the icon font, whose bundled subset has no pull-request,
//! container or cloud glyphs.
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
//! Dependencies: `codemason-app-core` (sidebar state and actions),
//! `codemason-config` (backend kinds), `chrono` (relative ages), `egui` and
//! [`crate::palette`].

use codemason_app_core::app::{App, Attention};
use codemason_app_core::geometry::Area;
use codemason_app_core::sessions_sidebar_state::{
    PINNED_KEY, SidebarRow, attention_of, backend_of, build_rows, click_row, open_row_menu,
    refresh_if_dirty, relative_age, selectable,
};
use codemason_config::ExecutionBackendKind;
use egui::{
    Atom, Atoms, Color32, IdSalt, IntoAtoms, Painter, Pos2, Rect, RichText, Role, ScrollArea,
    Stroke, WidgetInfo, pos2, vec2,
};

use crate::dialogs::session_menu;
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

/// Pull/merge request state of a session, as its row's badge shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    /// The session has had no request.
    None,
    /// The session has had requests, none bound now.
    Past,
    /// The session is bound to a request (Auto-fix may be on or off).
    Active,
    /// The session is bound to a request and Auto-fix waits on the user.
    Paused,
}

/// The pull-request badge of a session row: always drawn, with a count of
/// `0` when the session has had no request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrBadge {
    /// Requests the session has had (at least 1 while one is bound).
    pub count: usize,
    /// Whether one is bound now, and whether Auto-fix waits on the user.
    pub state: PrState,
}

/// The pull-request badge of session `id`, from the sidebar's cached
/// request totals and bound requests.
///
/// # Arguments
///
/// * `app` - the App whose cached sidebar data is read.
/// * `id` - the session id.
///
/// # Returns
///
/// The badge; a session with no request has `count` 0 and [`PrState::None`].
#[must_use]
pub fn pr_badge(app: &App, id: &str) -> PrBadge {
    let total = app.sidebar.request_totals.get(id).copied().unwrap_or(0);
    match (app.sidebar.bound.get(id), total) {
        (Some(true), n) => PrBadge {
            count: n.max(1),
            state: PrState::Paused,
        },
        (Some(false), n) => PrBadge {
            count: n.max(1),
            state: PrState::Active,
        },
        (None, 0) => PrBadge {
            count: 0,
            state: PrState::None,
        },
        (None, n) => PrBadge {
            count: n,
            state: PrState::Past,
        },
    }
}

/// The name of an execution backend, as a session row's tooltip and
/// accessible description give it.
///
/// # Arguments
///
/// * `kind` - the backend kind.
///
/// # Returns
///
/// e.g. `Local computer` or `Podman container`.
#[must_use]
pub const fn backend_name(kind: ExecutionBackendKind) -> &'static str {
    match kind {
        ExecutionBackendKind::Local => "Local computer",
        ExecutionBackendKind::Docker => "Docker container",
        ExecutionBackendKind::Podman => "Podman container",
        ExecutionBackendKind::Remote => "Remote system",
    }
}

/// The words for an attention state in a session's tooltip, or `None` when
/// the session is idle.
fn attention_words(attention: Attention) -> Option<&'static str> {
    match attention {
        Attention::Waiting => Some("Waiting for you"),
        Attention::Running => Some("Running"),
        Attention::Error => Some("Failed"),
        Attention::Unread => Some("Unread"),
        Attention::Idle => None,
    }
}

/// The colour of a session's attention dot, or `None` when it is idle.
fn attention_colour(attention: Attention, palette: &Palette) -> Option<Color32> {
    match attention {
        Attention::Waiting => Some(palette.warning),
        Attention::Running => Some(palette.accent),
        Attention::Error => Some(palette.error),
        Attention::Unread => Some(palette.info),
        Attention::Idle => None,
    }
}

/// What a session row says beyond its title: age, attention, backend,
/// branch and requests, one per line. It is the row's tooltip and, joined,
/// its accessible description.
///
/// # Arguments
///
/// * `app` - the App whose cached sidebar data is read.
/// * `id` - the session id.
/// * `updated_at` - the session's RFC 3339 last-activity time.
/// * `branch` - the session's branch, if it has one.
/// * `requests` - requests whose source is `branch`, when known.
///
/// # Returns
///
/// The lines, e.g. `["Updated 3h ago", "Backend: Podman container",
/// "Pull requests: 0"]`.
#[must_use]
pub fn session_details(
    app: &App,
    id: &str,
    updated_at: &str,
    branch: Option<&str>,
    requests: Option<i64>,
) -> Vec<String> {
    let age = relative_age(updated_at, chrono::Utc::now());
    let mut lines = vec![if age == "now" {
        "Updated just now".to_string()
    } else {
        format!("Updated {age} ago")
    }];
    if let Some(words) = attention_words(attention_of(app, id)) {
        lines.push(words.to_string());
    }
    lines.push(format!("Backend: {}", backend_name(backend_of(app, id))));
    if let Some(branch) = branch {
        match requests.filter(|n| *n > 0) {
            Some(n) => lines.push(format!("Branch: {branch} ({n} requests from it)")),
            None => lines.push(format!("Branch: {branch}")),
        }
    }
    let badge = pr_badge(app, id);
    let state = match badge.state {
        PrState::None | PrState::Past => "",
        PrState::Active => " (one bound)",
        PrState::Paused => " (one bound, Auto-fix waiting on you)",
    };
    lines.push(format!("Pull requests: {}{state}", badge.count));
    lines
}

/// The id of a session row's attention-dot slot.
fn attention_id() -> IdSalt {
    IdSalt::new("sessions_attention")
}

/// The id of a session row's pull-request badge slot.
fn pr_id() -> IdSalt {
    IdSalt::new("sessions_pr")
}

/// The id of a session row's backend icon slot.
fn backend_id() -> IdSalt {
    IdSalt::new("sessions_backend")
}

/// The colour of a pull-request badge: muted with no request, accent for
/// past ones, success while one is bound and warning while Auto-fix waits.
fn pr_colour(state: PrState, palette: &Palette) -> Color32 {
    match state {
        PrState::None => palette.muted,
        PrState::Past => palette.accent,
        PrState::Active => palette.success,
        PrState::Paused => palette.warning,
    }
}

/// The stroke row icons are painted with, for an icon `size` high.
fn icon_stroke(size: f32, colour: Color32) -> Stroke {
    Stroke::new((size / 12.0).max(1.0), colour)
}

/// The square, `scale` of the shorter side of `slot`, centred in it.
fn icon_square(slot: Rect, scale: f32) -> Rect {
    Rect::from_center_size(
        slot.center(),
        egui::Vec2::splat(slot.height().min(slot.width()) * scale),
    )
}

/// Paint a pull-request icon (two commits on a line, and a branch curling
/// back into it) in the square `q`. Points are fractions of `q`.
fn paint_pr_icon(painter: &Painter, q: Rect, colour: Color32) {
    let at = |x: f32, y: f32| q.lerp_inside(vec2(x, y));
    let stroke = icon_stroke(q.width(), colour);
    let radius = q.width() * 0.13;
    for centre in [at(0.25, 0.17), at(0.25, 0.83), at(0.75, 0.83)] {
        painter.circle_stroke(centre, radius, stroke);
    }
    painter.line_segment([at(0.25, 0.3), at(0.25, 0.7)], stroke);
    // The branch: up from its commit, then left into an arrow head.
    painter.add(egui::Shape::line(
        vec![
            at(0.75, 0.7),
            at(0.75, 0.27),
            at(0.65, 0.17),
            at(0.53, 0.17),
        ],
        stroke,
    ));
    painter.line_segment([at(0.53, 0.17), at(0.65, 0.05)], stroke);
    painter.line_segment([at(0.53, 0.17), at(0.65, 0.29)], stroke);
}

/// Points, as fractions of a square, on the arc of the circle at
/// (`cx`, `cy`) of `radius` from angle `from` to `to` (degrees, clockwise on
/// screen from the +x axis).
fn arc(cx: f32, cy: f32, radius: f32, from: f32, to: f32) -> impl Iterator<Item = egui::Vec2> {
    (0..=8u8).map(move |i| {
        let t = (to - from).mul_add(f32::from(i) / 8.0, from).to_radians();
        vec2(radius.mul_add(t.cos(), cx), radius.mul_add(t.sin(), cy))
    })
}

/// Paint the icon of execution backend `kind` in the square `q`: a monitor
/// for the local computer, a shipping container for Docker, a pod of three
/// containers for Podman and a cloud for a remote system. Points are
/// fractions of `q`.
fn paint_backend_icon(painter: &Painter, q: Rect, kind: ExecutionBackendKind, colour: Color32) {
    let at = |x: f32, y: f32| q.lerp_inside(vec2(x, y));
    let s = q.width();
    let stroke = icon_stroke(s, colour);
    let outline = |min: Pos2, max: Pos2, rounding: f32| {
        painter.rect_stroke(
            Rect::from_min_max(min, max),
            s * rounding,
            stroke,
            egui::StrokeKind::Middle,
        );
    };
    match kind {
        ExecutionBackendKind::Local => {
            outline(at(0.0, 0.1), at(1.0, 0.68), 0.08);
            painter.line_segment([at(0.5, 0.68), at(0.5, 0.92)], stroke);
            painter.line_segment([at(0.25, 0.92), at(0.75, 0.92)], stroke);
        }
        ExecutionBackendKind::Docker => {
            outline(at(0.0, 0.2), at(1.0, 0.8), 0.06);
            for x in [0.3, 0.5, 0.7] {
                painter.line_segment([at(x, 0.32), at(x, 0.68)], stroke);
            }
        }
        ExecutionBackendKind::Podman => {
            outline(at(0.05, 0.55), at(0.45, 0.95), 0.05);
            outline(at(0.55, 0.55), at(0.95, 0.95), 0.05);
            outline(at(0.3, 0.07), at(0.7, 0.47), 0.05);
        }
        ExecutionBackendKind::Remote => {
            let points: Vec<Pos2> = arc(0.24, 0.63, 0.17, 90.0, 250.0)
                .chain(arc(0.52, 0.48, 0.27, 195.0, 335.0))
                .chain(arc(0.78, 0.6, 0.2, 250.0, 450.0))
                .map(|v| q.lerp_inside(v))
                .collect();
            painter.add(egui::Shape::closed_line(points, stroke));
        }
    }
}

/// Paint the right-hand columns of a session row: the pull-request icon
/// and count in `pr_slot` and the backend icon in `backend_slot`.
fn paint_session_badges(
    ui: &egui::Ui,
    pr_slot: Rect,
    backend_slot: Rect,
    badge: PrBadge,
    backend: ExecutionBackendKind,
    palette: &Palette,
) {
    let painter = ui.painter();
    let colour = pr_colour(badge.state, palette);
    let icon = Rect::from_min_size(pr_slot.min, egui::Vec2::splat(pr_slot.height()));
    paint_pr_icon(painter, icon_square(icon, 0.85), colour);
    let font = egui::TextStyle::Small.resolve(ui.style());
    painter.text(
        pos2(icon.right() + 2.0, pr_slot.center().y),
        egui::Align2::LEFT_CENTER,
        badge.count.to_string(),
        font,
        colour,
    );
    paint_backend_icon(
        painter,
        icon_square(backend_slot, 0.9),
        backend,
        palette.muted,
    );
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
    let r = Rect::from_center_size(slot.center(), slot.size() * 0.4);
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
            palette.text,
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
            let size = egui::TextStyle::Small.resolve(ui.style()).size;
            (
                RichText::new(text)
                    .small()
                    .color(header_colour(key, palette)),
                Atom::custom(disclosure_id(), egui::Vec2::splat(size)),
            )
                .into_atoms()
        }
        SidebarRow::Session { id, title, .. } => session_atoms(ui, app, id, &shown(title), palette),
    }
}

/// A session row: its title (and attention dot), then the fixed-width
/// pull-request and backend columns, painted by [`paint_session_badges`].
fn session_atoms(
    ui: &egui::Ui,
    app: &App,
    id: &str,
    title: &str,
    palette: &Palette,
) -> Atoms<'static> {
    let size = egui::TextStyle::Button.resolve(ui.style()).size;
    let attention = attention_of(app, id);
    let open = app.session.session_id.as_deref() == Some(id);
    let mut text = RichText::new(title.to_string()).color(palette.text);
    if open || matches!(attention, Attention::Unread | Attention::Waiting) {
        text = text.strong();
    }
    // Room for the icon and a two-digit count, so every row's columns line
    // up whatever its count.
    let small = egui::TextStyle::Small.resolve(ui.style());
    let digits = ui
        .painter()
        .layout_no_wrap("88".to_string(), small, palette.text)
        .size()
        .x;
    let mut atoms = text.into_atoms();
    if attention_colour(attention, palette).is_some() {
        atoms.push_right(Atom::custom(attention_id(), vec2(size * 0.7, size)));
    }
    atoms.push_right(Atom::grow());
    atoms.push_right(Atom::custom(pr_id(), vec2(size + 2.0 + digits, size)));
    atoms.push_right(Atom::custom(backend_id(), vec2(size, size)));
    atoms
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
        // Roomy rows, as a navigation list draws them.
        let row_height = ui.text_style_height(&egui::TextStyle::Button) + 10.0;
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
        if app.sidebar.menu.is_none() {
            session_menu::forget_anchor(ui.ctx());
        }
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
                // The open or cursor row sits on a rounded selection fill;
                // the others draw a frame only while hovered.
                let mut button =
                    egui::Button::selectable(highlighted, row_text(ui, app, row, palette))
                        .frame_when_inactive(highlighted)
                        .corner_radius(6.0);
                if highlighted {
                    button = button.fill(palette.selection);
                }
                let response = button
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
                if let SidebarRow::Session {
                    id,
                    updated_at,
                    branch,
                    requests,
                    ..
                } = row
                {
                    paint_session_row(ui, app, &response, id, palette);
                    let details =
                        session_details(app, id, updated_at, branch.as_deref(), *requests);
                    // The badges are only painted: give assistive
                    // technology what they say.
                    let description = details.join(", ");
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_description(description);
                    });
                    // The tooltip is shown as a side effect; the returned
                    // response is the same row.
                    response.response.clone().on_hover_text(details.join("\n"));
                }
                if response.clicked() {
                    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                    clicked = Some((index, ctrl));
                }
                if index == cursor {
                    session_menu::note_cursor_row(ui.ctx(), response.rect);
                }
                if response.secondary_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        session_menu::note_pointer(ui.ctx(), pos);
                    }
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

/// Paint what a session row's atoms leave room for: the selection bar on
/// its left edge, the attention dot and the pull-request and backend
/// columns.
fn paint_session_row(
    ui: &egui::Ui,
    app: &App,
    response: &egui::WidgetAtomResponse,
    id: &str,
    palette: &Palette,
) {
    if app.sidebar.selected.contains(id) {
        let rect = response.rect;
        let bar = Rect::from_min_max(
            rect.left_top() + vec2(0.0, 4.0),
            pos2(rect.left() + 3.0, rect.bottom() - 4.0),
        );
        ui.painter().rect_filled(bar, 1.5, palette.accent);
    }
    if let (Some(slot), Some(colour)) = (
        response.rect(attention_id()),
        attention_colour(attention_of(app, id), palette),
    ) {
        ui.painter()
            .circle_filled(slot.center(), slot.height() * 0.16, colour);
    }
    if let (Some(pr_slot), Some(backend_slot)) =
        (response.rect(pr_id()), response.rect(backend_id()))
    {
        paint_session_badges(
            ui,
            pr_slot,
            backend_slot,
            pr_badge(app, id),
            backend_of(app, id),
            palette,
        );
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
