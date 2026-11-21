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
//! Resting the pointer on a row with requests opens the request popup
//! beside it (in place of the tooltip): each request's title, then its
//! state, repository and number, and its diff size (`+120 -7`, GitHub only)
//! once the history refresh has read it; an eye marks the request Auto-fix
//! watches, and a click opens it in the browser ([`App::open_url`]). The
//! popup stays open while the pointer moves onto it.
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
//! The list is virtualised with `ScrollArea::show_viewport` over a table of
//! row heights (one text row per sidebar row), so a long session list costs
//! only the visible rows. `show_rows` is not used because a section's blank
//! row is drawn shorter than a session row, and `show_rows` assumes every row
//! is the same height.
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
use codemason_app_core::styled::ThemeColor;
use codemason_config::ExecutionBackendKind;
use codemason_storage::{DiffStats, SessionRequest};
use egui::{
    Atom, Atoms, Color32, Id, IdSalt, IntoAtoms, Painter, Pos2, Rect, RichText, Role, ScrollArea,
    Stroke, Vec2, WidgetInfo, pos2, vec2,
};

use crate::dialogs::session_menu;
use crate::icons::{Icon, icon_label};
use crate::palette::Palette;
use crate::styled_text::drawable;

/// Accessible label of the session list.
pub const SESSIONS_LABEL: &str = "Sessions";

/// How much larger a section heading is drawn than a session row's title.
///
/// The titles are drawn in `TextStyle::Button`; the heading is this factor of
/// that, so it stays a fifth larger whatever the window's font size is.
const HEADING_SCALE: f32 = 1.2;

/// The height of a section's blank row above its heading, as a fraction of a
/// full row: the gap above a project heading is 40% smaller than a blank row
/// used to make it.
const SECTION_GAP_SCALE: f32 = 0.6;

/// The size a section heading is drawn at, given its session titles' size.
///
/// # Arguments
///
/// * `title_size` - the size the session rows' titles are drawn at.
///
/// # Returns
///
/// [`HEADING_SCALE`] of `title_size`.
#[doc(hidden)]
#[must_use]
pub fn heading_size(title_size: f32) -> f32 {
    title_size * HEADING_SCALE
}

/// The height of a section's blank row above its heading.
///
/// # Arguments
///
/// * `row_height` - the height of a full session row.
///
/// # Returns
///
/// [`SECTION_GAP_SCALE`] of `row_height`.
#[doc(hidden)]
#[must_use]
pub fn section_gap_height(row_height: f32) -> f32 {
    row_height * SECTION_GAP_SCALE
}

/// Scroll state the sidebar keeps between frames.
#[derive(Debug, Default)]
pub struct SessionsView {
    /// The cursor row drawn last frame, to scroll it into view once it moves.
    last_cursor: Option<usize>,
    /// The session whose request popup is open, and the row it hangs from.
    popup: Option<(String, Rect)>,
    /// Where the request popup was drawn last frame.
    popup_rect: Option<Rect>,
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
            // A heading is a fifth larger than the titles it heads, so a
            // project's name no longer reads as smaller than its own
            // sessions (egui's `TextStyle::Small`, at 9 pt, sat below both
            // this window's smallest caption and the 13 pt session titles).
            let size = heading_size(font.size);
            (
                RichText::new(text)
                    .size(size)
                    .color(header_colour(key, palette)),
                Atom::custom(disclosure_id(), egui::Vec2::splat(size)),
            )
                .into_atoms()
        }
        SidebarRow::Session { id, title, .. } => session_atoms(ui, app, id, &shown(title), palette),
        SidebarRow::SubAgent {
            agent,
            task,
            depth,
            suspended,
            ..
        } => subagent_atoms(ui, &shown(agent), &shown(task), *depth, *suspended, palette),
    }
}

/// A running sub-agent row, indented under the session that spawned it:
/// the agent icon and name, then the task's first line, dimmed.
fn subagent_atoms(
    ui: &egui::Ui,
    agent: &str,
    task: &str,
    depth: usize,
    suspended: bool,
    palette: &Palette,
) -> Atoms<'static> {
    let size = egui::TextStyle::Button.resolve(ui.style()).size;
    let colour = if suspended {
        palette.muted
    } else {
        palette.text
    };
    let state = if suspended { " (paused)" } else { "" };
    (
        Atom::custom(
            row_indent_id(),
            vec2(size.mul_add(depth as f32, SESSION_INDENT), size),
        ),
        icon_label(ui.ctx(), Icon::Agent, size, colour),
        RichText::new(format!("{agent}{state}")).color(colour),
        RichText::new(format!(" {task}")).color(palette.muted),
    )
        .into_atoms()
}

/// How far session rows sit in from their project heading, in points, so
/// sessions and projects are told apart at a glance.
const SESSION_INDENT: f32 = 20.0;

/// Id of the blank slot that indents a session or sub-agent row.
fn row_indent_id() -> IdSalt {
    IdSalt::new("sessions_row_indent")
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
    atoms.push_left(Atom::custom(row_indent_id(), vec2(SESSION_INDENT, size)));
    if attention_colour(attention, palette).is_some() {
        atoms.push_right(Atom::custom(attention_id(), vec2(size * 0.7, size)));
    }
    atoms.push_right(Atom::grow());
    atoms.push_right(Atom::custom(pr_id(), vec2(size + 2.0 + digits, size)));
    atoms.push_right(Atom::custom(backend_id(), vec2(size, size)));
    atoms
}

/// The colour of a request in `state` (`open`, `merged`, `closed`).
fn request_colour(state: &str, palette: &Palette) -> Color32 {
    match state {
        "open" => palette.success,
        // The forges show merged in purple: the theme's fifth categorical
        // colour (its magenta).
        "merged" => palette.color(ThemeColor::Categorical(4)),
        "closed" => palette.error,
        _ => palette.muted,
    }
}

/// A request's state as a word: `Open`, `Merged`, `Closed`.
fn state_word(state: &str) -> String {
    let mut chars = state.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// `#12` on GitHub, `!12` on GitLab.
fn request_number(request: &SessionRequest) -> String {
    let sigil = codemason_agent::project::RemoteKind::parse(&request.forge)
        .map_or('#', |kind| kind.noun().sigil);
    format!("{sigil}{}", request.number)
}

/// Paint a pull-request glyph in the square `rect`: a branch on the left
/// (two rings joined by a line) and, on the right, a ring whose line rises
/// and turns back toward the branch, ending in an arrowhead.
fn paint_pr_glyph(painter: &egui::Painter, rect: Rect, colour: Color32) {
    let s = rect.width();
    let stroke = Stroke::new((s * 0.085).max(1.0), colour);
    let ring = s * 0.1;
    let left = s.mul_add(0.22, rect.left());
    let right = s.mul_add(-0.22, rect.right());
    let top = s.mul_add(0.18, rect.top());
    let bottom = s.mul_add(-0.18, rect.bottom());
    painter.circle_stroke(egui::pos2(left, top), ring, stroke);
    painter.circle_stroke(egui::pos2(left, bottom), ring, stroke);
    painter.circle_stroke(egui::pos2(right, bottom), ring, stroke);
    painter.line_segment(
        [
            egui::pos2(left, top + ring),
            egui::pos2(left, bottom - ring),
        ],
        stroke,
    );
    // The right line rises to the top and turns back to the gap between
    // the columns, where its arrowhead points at the branch.
    let tip = egui::pos2(f32::midpoint(left, right), top);
    painter.add(egui::Shape::line(
        vec![
            egui::pos2(right, bottom - ring),
            egui::pos2(right, top),
            tip,
        ],
        stroke,
    ));
    let barb = s * 0.13;
    painter.line_segment([tip, tip + Vec2::new(barb, -barb)], stroke);
    painter.line_segment([tip, tip + Vec2::new(barb, barb)], stroke);
}

/// Id of the request popup's open state: the session it is for and the
/// row it hangs from.
fn popup_id() -> Id {
    Id::unique("codemason_sessions_request_popup")
}

/// Width of the request popup, in points.
const POPUP_WIDTH: f32 = 340.0;

/// How many requests the popup shows before it scrolls.
const POPUP_ROWS: f32 = 7.0;

/// Which session's request popup is open, if any, after this frame's
/// pointer: the popup stays open while the pointer is on it or crossing to
/// it from its row; resting on a row with requests opens that row's (at
/// once while another popup is open, otherwise after the tooltip delay).
fn popup_target(
    ui: &egui::Ui,
    hovered: Option<(String, Rect)>,
    open: Option<(String, Rect)>,
    popup_rect: Option<Rect>,
) -> Option<(String, Rect)> {
    let pointer = ui.ctx().pointer_hover_pos();
    let inside = |rect: Rect| pointer.is_some_and(|p| rect.contains(p));
    if let (Some(row), Some(popup)) = (open.as_ref(), popup_rect) {
        let bridge =
            Rect::from_x_y_ranges(row.1.right()..=popup.left(), row.1.y_range()).expand(4.0);
        // On its own row, follow the row's current rectangle, so a scroll
        // or a sidebar resize moves the popup with it.
        if let Some(current) = hovered.as_ref().filter(|h| h.0 == row.0) {
            return Some(current.clone());
        }
        if inside(popup.expand(4.0)) || inside(bridge) {
            return open;
        }
    }
    let hovered = hovered?;
    let delay = ui.style().interaction.tooltip_delay;
    let rested = ui.input(|i| i.pointer.time_since_last_movement()) >= delay;
    if open.is_some() || rested {
        Some(hovered)
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f32(delay));
        None
    }
}

/// Draw the request popup for `requests` beside `row`, and open the
/// request clicked in it.
///
/// # Returns
///
/// The popup's rectangle.
fn request_popup(
    ctx: &egui::Context,
    app: &mut App,
    session_id: &str,
    requests: &[RequestEntry],
    row: Rect,
    palette: &Palette,
) -> Rect {
    let mut chosen = None;
    let response = egui::Area::new(popup_id().with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(row.right_top() + Vec2::new(4.0, 0.0))
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(POPUP_WIDTH);
                let entry = request_entry_height(ui);
                ScrollArea::vertical()
                    // One scroll state per session, so a long history
                    // scrolled in one popup does not open another
                    // session's popup part-way down.
                    .id_salt(("sessions_request_popup_scroll", session_id))
                    .max_height(entry * POPUP_ROWS)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for entry in requests {
                            if request_entry(ui, entry, palette).clicked() {
                                chosen = Some(entry.request.web_url.clone());
                            }
                        }
                    });
            });
        });
    if let Some(url) = chosen
        && let Err(e) = app.open_url(&url)
    {
        app.status = e;
    }
    response.response.rect
}

/// Height of one request in the popup: two lines and their padding.
fn request_entry_height(ui: &egui::Ui) -> f32 {
    let body = ui.text_style_height(&egui::TextStyle::Body);
    let small = ui.text_style_height(&egui::TextStyle::Small);
    body + small + crate::metrics::space::SM.mul_add(2.0, crate::metrics::space::XS)
}

/// One request as the popup lists it.
#[derive(Debug, Clone)]
struct RequestEntry {
    /// The request.
    request: SessionRequest,
    /// Its diff size, once read.
    stats: Option<DiffStats>,
    /// Whether the Auto-fix watcher polls it.
    watched: bool,
}

/// Paint an eye in the square `rect`: an almond outline and its pupil.
fn paint_eye(painter: &egui::Painter, rect: Rect, colour: Color32) {
    let s = rect.width();
    let stroke = Stroke::new((s * 0.085).max(1.0), colour);
    let centre = rect.center();
    let half = s * 0.42;
    let lift = s * 0.3;
    let lid = |sign: f32| {
        egui::Shape::QuadraticBezier(egui::epaint::QuadraticBezierShape::from_points_stroke(
            [
                centre - Vec2::new(half, 0.0),
                centre + Vec2::new(0.0, sign * lift * 2.0),
                centre + Vec2::new(half, 0.0),
            ],
            false,
            Color32::TRANSPARENT,
            stroke,
        ))
    };
    painter.add(lid(-1.0));
    painter.add(lid(1.0));
    painter.circle_filled(centre, s * 0.14, colour);
}

/// One request in the popup: its title (with an eye at the right while
/// Auto-fix watches it), then its state glyph and word, repository and
/// number, and its diff size (`+120 -7`) when it is known. A click opens it.
fn request_entry(ui: &mut egui::Ui, entry: &RequestEntry, palette: &Palette) -> egui::Response {
    let RequestEntry {
        request,
        stats,
        watched,
    } = entry;
    let stats = *stats;
    use crate::metrics::space;
    let size = Vec2::new(ui.available_width(), request_entry_height(ui));
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let number = request_number(request);
    let mut size_words = stats.map_or_else(String::new, |s| {
        format!(", {} added, {} removed", s.additions, s.deletions)
    });
    if *watched {
        size_words.push_str(", watched by Auto-fix");
    }
    response.widget_info(|| {
        WidgetInfo::labeled(
            Role::Link,
            true,
            format!(
                "{} {number}: {} ({}{size_words})",
                request.repo, request.title, request.state
            ),
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    // Keyboard focus is highlighted like the pointer, so Tab shows which
    // request Enter opens.
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, crate::metrics::radius::XS, palette.selection);
    }
    let inner = rect.shrink2(Vec2::new(space::SM, space::SM));
    let body = ui.text_style_height(&egui::TextStyle::Body);
    // The eye sits at the title line's right end; the title is cut before it.
    let eye_width = if *watched { body + space::SM } else { 0.0 };
    if *watched {
        let eye = Rect::from_min_size(
            egui::pos2(inner.right() - body, inner.top()),
            Vec2::splat(body),
        );
        paint_eye(ui.painter(), eye, palette.accent);
    }
    let title = egui::WidgetText::from(RichText::new(&request.title).color(palette.text))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            inner.width() - eye_width,
            egui::TextStyle::Body,
        );
    let title_height = title.size().y;
    ui.painter().galley(inner.left_top(), title, palette.text);
    let small = ui.text_style_height(&egui::TextStyle::Small);
    let line = Rect::from_min_size(
        inner.left_top() + Vec2::new(0.0, title_height + space::XS),
        Vec2::new(inner.width(), small),
    );
    let colour = request_colour(&request.state, palette);
    paint_pr_glyph(
        ui.painter(),
        Rect::from_min_size(line.left_top(), Vec2::splat(small)),
        colour,
    );
    let mut job = egui::text::LayoutJob::default();
    let font = egui::TextStyle::Small.resolve(ui.style());
    let format = |colour| egui::TextFormat::simple(font.clone(), colour);
    job.append(&state_word(&request.state), 0.0, format(colour));
    job.append(
        &format!("  \u{b7}  {} {number}", request.repo),
        0.0,
        format(palette.muted),
    );
    // The diff size sits at the right end; the rest is cut short before it.
    let size = stats.map(|s| {
        let mut size = egui::text::LayoutJob::default();
        size.append(
            &format!("+{}", s.additions),
            0.0,
            format(palette.diff_added),
        );
        size.append(
            &format!(" -{}", s.deletions),
            0.0,
            format(palette.diff_removed),
        );
        ui.fonts_mut(|f| f.layout_job(size))
    });
    let size_width = size.as_ref().map_or(0.0, |g| g.size().x + space::SM);
    job.wrap =
        egui::text::TextWrapping::truncate_at_width(line.width() - small - space::XS - size_width);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(
        line.left_top() + Vec2::new(small + space::XS, 0.0),
        galley,
        palette.muted,
    );
    if let Some(size) = size {
        let pos = egui::pos2(line.right() - size.size().x, line.top());
        ui.painter().galley(pos, size, palette.muted);
    }
    response
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
        // Roomy rows, as a navigation list draws them. A section's blank row
        // is drawn shorter than a session row, so the rows are laid out one
        // by one instead of by `ScrollArea::show_rows`, which assumes they
        // are all the same height.
        let row_height = ui.text_style_height(&egui::TextStyle::Button) + 10.0;
        let spacing = ui.spacing().item_spacing.y;
        let layout = RowLayout::new(&rows, row_height, spacing);
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
            scroll = scroll_to_row(scroll, ui, &layout, cursor, visible_rows);
        }
        self.last_cursor = focused.then_some(cursor);
        let mut clicked = None;
        let mut menu = None;
        // The session row under the pointer that has requests, and its rect.
        let mut hovered_requests = None;
        let output = scroll.show_viewport(ui, |ui, viewport| {
            ui.set_height(layout.content_height());
            for index in layout.visible(&viewport) {
                let Some(row) = rows.get(index) else {
                    continue;
                };
                let top = ui.max_rect().top() + layout.top(index);
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("sessions_row", index))
                        .max_rect(Rect::from_min_size(
                            pos2(ui.max_rect().left(), top),
                            vec2(ui.available_width(), layout.height(index)),
                        ))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let ui = &mut child;
                if matches!(row, SidebarRow::Spacer) {
                    // A blank row: it is drawn shorter than a session row and
                    // cannot be clicked.
                    continue;
                }
                let open = matches!(
                    row,
                    SidebarRow::Session { id, .. } | SidebarRow::SubAgent { id, .. }
                        if app.session.session_id.as_deref() == Some(id)
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
                    // One line per row: the row heights and the cursor
                    // scrolling assume a fixed height per row.
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
                    // A row with requests opens the request popup instead of
                    // a tooltip; the tooltip is shown as a side effect.
                    if app.sidebar.request_history.contains_key(id) {
                        if response.hovered() {
                            hovered_requests = Some((id.clone(), response.rect));
                        }
                    } else {
                        response.response.clone().on_hover_text(details.join("\n"));
                    }
                }
                if let SidebarRow::SubAgent { agent, task, .. } = row {
                    response.response.clone().on_hover_text(format!(
                        "Sub-agent {agent}: {task}\nOpen to watch it and message it"
                    ));
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
        self.show_request_popup(ui, app, hovered_requests, palette);
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

/// Where each row of the sidebar is drawn, and how tall it is.
///
/// Session rows are all `row_height`, but a section's blank row above its
/// heading is [`SECTION_GAP_SCALE`] of that, so the list cannot use
/// `ScrollArea::show_rows`, which assumes a uniform row height. This table
/// gives each row's top and height, so the rows in view are laid out by
/// position and the content height is exact.
#[derive(Debug)]
struct RowLayout {
    /// The top of each row, relative to the top of the scrolled content.
    tops: Vec<f32>,
    /// The height of each row.
    heights: Vec<f32>,
    /// The gap drawn between two rows.
    spacing: f32,
    /// The content height: every row and the gap under it, without the last
    /// gap (as `ScrollArea::show_rows` lays it out).
    content_height: f32,
}

impl RowLayout {
    /// Lay out `rows`: `row_height` for a session or heading row, and
    /// [`SECTION_GAP_SCALE`] of it for a blank row.
    ///
    /// # Arguments
    ///
    /// * `rows` - the rows to lay out, in order.
    /// * `row_height` - the height of a full row.
    /// * `spacing` - the gap drawn under each row.
    ///
    /// # Returns
    ///
    /// The layout.
    fn new(rows: &[SidebarRow], row_height: f32, spacing: f32) -> Self {
        let gap = section_gap_height(row_height);
        let mut tops = Vec::with_capacity(rows.len());
        let mut heights = Vec::with_capacity(rows.len());
        let mut offset = 0.0;
        for row in rows {
            let height = if matches!(row, SidebarRow::Spacer) {
                gap
            } else {
                row_height
            };
            tops.push(offset);
            heights.push(height);
            offset += height + spacing;
        }
        Self {
            tops,
            heights,
            spacing,
            content_height: (offset - spacing).max(0.0),
        }
    }

    /// The number of rows laid out.
    fn len(&self) -> usize {
        self.heights.len()
    }

    /// The top of row `index`.
    fn top(&self, index: usize) -> f32 {
        self.tops.get(index).copied().unwrap_or(0.0)
    }

    /// The height of row `index`.
    fn height(&self, index: usize) -> f32 {
        self.heights.get(index).copied().unwrap_or(0.0)
    }

    /// The top of the scrolled content.
    ///
    /// # Returns
    ///
    /// Every row and the gap under it, without the last gap.
    fn content_height(&self) -> f32 {
        self.content_height
    }

    /// The rows that intersect `viewport`, which is relative to the top of the
    /// scrolled content.
    ///
    /// # Arguments
    ///
    /// * `viewport` - the visible part of the content.
    ///
    /// # Returns
    ///
    /// The index range of the rows to draw, top to bottom.
    fn visible(&self, viewport: &Rect) -> std::ops::Range<usize> {
        if self.len() == 0 {
            return 0..0;
        }
        let min = viewport.min.y;
        let max = viewport.max.y;
        let first = self
            .tops
            .partition_point(|&top| top < min)
            .saturating_sub(1);
        let last = self
            .tops
            .partition_point(|&top| top < max)
            .max(first + 1)
            .min(self.len());
        first..last
    }
}

impl SessionsView {
    /// Open, keep or close the request popup for this frame's pointer
    /// ([`popup_target`]) and draw it.
    fn show_request_popup(
        &mut self,
        ui: &egui::Ui,
        app: &mut App,
        hovered: Option<(String, Rect)>,
        palette: &Palette,
    ) {
        let ctx = ui.ctx().clone();
        let open = self.popup.take();
        let target = popup_target(ui, hovered, open, self.popup_rect);
        let Some((id, row)) = target else {
            self.popup_rect = None;
            return;
        };
        let Some(requests) = app.sidebar.request_history.get(&id).cloned() else {
            self.popup_rect = None;
            return;
        };
        let entries: Vec<RequestEntry> = requests
            .into_iter()
            .map(|request| {
                let key = (request.host.clone(), request.repo.clone(), request.number);
                RequestEntry {
                    stats: app.sidebar.request_stats.get(&key).copied(),
                    watched: app.sidebar.watched_requests.contains(&key),
                    request,
                }
            })
            .collect();
        self.popup_rect = Some(request_popup(&ctx, app, &id, &entries, row, palette));
        self.popup = Some((id, row));
    }
}

/// `scroll` set to bring row `index` into view, given this frame's viewport

/// `scroll` set to bring row `index` into view, given this frame's viewport
/// of `visible_rows` full rows.
fn scroll_to_row(
    scroll: ScrollArea,
    ui: &egui::Ui,
    layout: &RowLayout,
    index: usize,
    visible_rows: f32,
) -> ScrollArea {
    let stride = layout.height(index) + layout.spacing;
    let top = layout.top(index);
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
