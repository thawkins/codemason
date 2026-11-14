//! Placement and painting of the sidebar context menus (spec SESSIONMENU).
//!
//! The menu state and its key/mouse handling are frontend-neutral and live in
//! [`crate::session_menu`]; this module places the open menu (and submenu)
//! next to the sidebar, records the painted areas for mouse hits and paints.
//!
//! Depends on `ratatui` and [`crate::adapters`] (`Area` <-> `Rect`).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::adapters::{area_to_rect, rect_to_area};
use crate::app::App;
use crate::session_menu::SessionMenu;

/// Width and height of `menu`, clamped to `frame`.
fn menu_size(menu: &SessionMenu, frame: Rect) -> (u16, u16) {
    let label_width = menu
        .items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    // Border, space, label, gap, accelerator, space, border.
    let width = u16::try_from(label_width + 7)
        .unwrap_or(u16::MAX)
        .min(frame.width);
    let height = u16::try_from(menu.line_count() + 2)
        .unwrap_or(u16::MAX)
        .min(frame.height);
    (width, height)
}

/// Where the menu goes: just below its row, or just above it when it would
/// run off the bottom of `frame`; shifted left to stay on screen.
fn menu_area(menu: &SessionMenu, sidebar: Rect, frame: Rect) -> Rect {
    let (width, height) = menu_size(menu, frame);
    let right = frame.x + frame.width;
    let x = (sidebar.x + sidebar.width.saturating_sub(4)).min(right.saturating_sub(width));
    let bottom = frame.y + frame.height;
    let y = if menu.row_y + 1 + height <= bottom {
        menu.row_y + 1
    } else {
        menu.row_y.saturating_sub(height).max(frame.y)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Where a submenu goes: to the right of `parent`, its first item level
/// with the parent item, or to the left of `parent` when there is no room
/// on the right; shifted up to stay on screen.
fn submenu_area(sub: &SessionMenu, parent: Rect, frame: Rect) -> Rect {
    let (width, height) = menu_size(sub, frame);
    let right = frame.x + frame.width;
    let x = if parent.x + parent.width + width <= right {
        parent.x + parent.width
    } else {
        parent.x.saturating_sub(width)
    };
    let bottom = frame.y + frame.height;
    let y = sub
        .row_y
        .saturating_sub(1)
        .min(bottom.saturating_sub(height))
        .max(frame.y);
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Paints one menu level into `area`.
fn paint(frame: &mut Frame, menu: &SessionMenu, area: Rect) {
    let inner_width = usize::from(area.width.saturating_sub(2));
    let normal = Style::default().fg(Color::White);
    let selected = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let mut lines = Vec::with_capacity(menu.line_count());
    for (index, item) in menu.items.iter().enumerate() {
        if index > 0 && menu.items[index - 1].group != item.group {
            lines.push(Line::from(Span::styled(
                "-".repeat(inner_width),
                Style::default().fg(Color::DarkGray),
            )));
        }
        let hint = if item.action.opens_submenu() {
            '>'
        } else {
            item.accel.map_or(' ', |c| c.to_ascii_uppercase())
        };
        let gap = inner_width.saturating_sub(item.label.chars().count() + 3);
        let text = format!(" {}{}{hint} ", item.label, " ".repeat(gap));
        let style = match (index == menu.cursor, item.enabled, item.danger) {
            (true, _, _) => selected,
            (false, false, _) => normal.fg(Color::DarkGray),
            (false, true, true) => normal.fg(Color::Red),
            (false, true, false) => normal,
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    let lines: Vec<Line> = lines.into_iter().skip(menu.scroll).collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Paints the open menu and submenu, if any, on top of the sidebar and chat.
///
/// # Arguments
///
/// * `frame` - the frame being drawn.
/// * `app` - holds the menu; painted areas are recorded for mouse hits.
pub fn render(frame: &mut Frame, app: &mut App) {
    let sidebar = area_to_rect(app.sidebar.area);
    let full = frame.area();
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return;
    };
    let area = menu_area(menu, sidebar, full);
    menu.area = rect_to_area(area);
    menu.keep_cursor_visible(usize::from(area.height.saturating_sub(2)));
    paint(frame, menu, area);
    if let Some(sub) = menu.submenu.as_mut() {
        let sub_area = submenu_area(sub, area, full);
        sub.area = rect_to_area(sub_area);
        sub.keep_cursor_visible(usize::from(sub_area.height.saturating_sub(2)));
        paint(frame, sub, sub_area);
    }
}
