//! Painting of the left-hand session sidebar (spec MULTISESSION, milestone
//! M5; spec `projects`).
//!
//! The sidebar's state, key and mouse handling live in the frontend-neutral
//! [`crate::sessions_sidebar_state`] module, re-exported here so existing
//! `sessions_sidebar::` paths keep resolving. This module only splits the
//! frame and paints the rows.
//!
//! Depends on `ratatui` and [`crate::adapters`] (`Area` <-> `Rect`).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::adapters::rect_to_area;
use crate::app::{App, Attention};
pub use crate::sessions_sidebar_state::*;

/// Background of the open session's row when the cursor is elsewhere.
const OPEN_SESSION_BG: Color = Color::Indexed(236);

/// Splits the frame into the sidebar and the chat area.
///
/// # Arguments
///
/// * `app` - for the sidebar preference and width.
/// * `area` - the whole frame.
///
/// # Returns
///
/// `(Some(sidebar), chat)` when the sidebar is shown, `(None, area)` when it
/// is turned off or the terminal is narrower than [`MIN_TERMINAL_WIDTH`].
///
/// The height test matters because `area` is the body below the status band,
/// not the whole frame: on a terminal short enough that the band (the status
/// bar, plus the teammate strip when a team is shown) leaves no room, the
/// sidebar would paint a bordered box of zero or fewer inner rows and swallow
/// every list row it holds, so it is dropped and the chat takes the body.
pub fn split_area(app: &App, area: Rect) -> (Option<Rect>, Rect) {
    if !app.sidebar.visible || area.width < MIN_TERMINAL_WIDTH || area.height <= 2 {
        return (None, area);
    }
    let width = app.sidebar.width.clamp(MIN_WIDTH, area.width / 2);
    let sidebar = Rect { width, ..area };
    let chat = Rect {
        x: area.x + width,
        width: area.width - width,
        ..area
    };
    (Some(sidebar), chat)
}

/// Marker and colour for an attention state (FR-04, ASCII only).
fn attention_marker(attention: Attention) -> (char, Color) {
    match attention {
        Attention::Waiting => ('!', Color::Yellow),
        Attention::Running => ('*', Color::Green),
        Attention::Error => ('x', Color::Red),
        Attention::Unread => ('o', Color::Cyan),
        Attention::Idle => (' ', Color::Reset),
    }
}

/// An action row: `text` in `colour`, with its shortcut `hint` dimmed at the
/// right edge when there is room for both.
fn with_hint(text: &str, colour: Color, hint: &str, width: usize) -> Line<'static> {
    let text_width = text.chars().count();
    let hint_width = hint.chars().count();
    let mut spans = vec![Span::styled(fit(text, width), Style::default().fg(colour))];
    if !hint.is_empty() && text_width + 1 + hint_width < width {
        spans.push(Span::raw(" ".repeat(width - text_width - hint_width - 1)));
        spans.push(Span::styled(
            hint.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

/// The chevron a section heading carries: `v` open, `>` collapsed.
fn collapsed_mark(collapsed: bool) -> char {
    if collapsed { '>' } else { 'v' }
}

/// Paints the sidebar into `area`.
///
/// # Arguments
///
/// * `frame` - the frame being drawn.
/// * `app` - sidebar state and sessions; the painted area is recorded.
/// * `area` - the sidebar rectangle from [`split_area`].
pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    refresh_if_dirty(app);
    app.sidebar.area = rect_to_area(area);
    let title = if app.sidebar.filter_editing || !app.sidebar.filter.is_empty() {
        let caret = if app.sidebar.filter_editing { "_" } else { "" };
        format!(" SESSIONS /{}{caret} ", app.sidebar.filter)
    } else {
        let total: usize = app.sidebar.groups.iter().map(|g| g.sessions.len()).sum();
        format!(" SESSIONS ({total}) ")
    };
    let border = if app.sidebar.focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = build_rows(app);
    let height = usize::from(inner.height);
    let state = &mut app.sidebar;
    // Rebuilt rows can put a spacer under the cursor (unpinning the only
    // pinned session, say); move to the next selectable row.
    state.cursor = selectable(&rows, state.cursor.min(rows.len().saturating_sub(1)), true);
    if state.cursor < state.scroll {
        state.scroll = state.cursor;
    } else if height > 0 && state.cursor >= state.scroll + height {
        state.scroll = state.cursor + 1 - height;
    }
    state.scroll = state.scroll.min(rows.len().saturating_sub(height.max(1)));
    let (scroll, cursor, focused) = (state.scroll, state.cursor, state.focused);

    let now = chrono::Utc::now();
    let width = usize::from(inner.width);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(height)
        .map(|(index, row)| {
            let mut line = row_line(app, row, width, now);
            let open_session = matches!(
                row,
                SidebarRow::Session { id, .. } if app.session.session_id.as_deref() == Some(id)
            );
            if focused && index == cursor {
                // Pad to the full width so the bar spans the whole row.
                let pad = width.saturating_sub(line.width());
                if pad > 0 {
                    line.spans.push(Span::raw(" ".repeat(pad)));
                }
                line = line.patch_style(Style::default().bg(Color::DarkGray));
            } else if open_session {
                // The open session keeps a soft full-width bar.
                let pad = width.saturating_sub(line.width());
                if pad > 0 {
                    line.spans.push(Span::raw(" ".repeat(pad)));
                }
                line = line.patch_style(Style::default().bg(OPEN_SESSION_BG));
            }
            line
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Builds the painted line for one row.
fn row_line(
    app: &App,
    row: &SidebarRow,
    width: usize,
    now: chrono::DateTime<chrono::Utc>,
) -> Line<'static> {
    match row {
        SidebarRow::NewSession => with_hint(" + New session", Color::Green, "Ctrl+N", width),
        SidebarRow::Spacer => Line::from(""),
        SidebarRow::Search => {
            let state = &app.sidebar;
            let text = if state.filter_editing || !state.filter.is_empty() {
                let caret = if state.filter_editing { "_" } else { "" };
                format!(" / {}{caret}", state.filter)
            } else {
                " / Search".to_string()
            };
            let hint = if state.filter_editing || !state.filter.is_empty() {
                ""
            } else {
                "/"
            };
            with_hint(&text, Color::Gray, hint, width)
        }
        SidebarRow::Header {
            key,
            label,
            count,
            collapsed,
            remote,
            description,
        } => {
            // A quiet heading so the session titles carry the list: the name
            // in grey (yellow for `Pinned`), a dimmer chevron (`v` open,
            // `>` collapsed), the remote host kind and the hidden-session
            // count when collapsed: `codemason v [github] 2`. The project's
            // `Properties` description follows the chevron when one is set.
            let name_colour = if key == PINNED_KEY {
                Color::Yellow
            } else {
                Color::Gray
            };
            let name = fit(label, width.saturating_sub(2));
            let arrow = format!(" {}", collapsed_mark(*collapsed));
            // The description outranks the remote kind and the collapsed
            // count: a short sidebar keeps it and drops those, so a project
            // stays identifiable where it is described.
            let room = width.saturating_sub(name.chars().count() + arrow.chars().count());
            let note = description
                .as_deref()
                .filter(|_| room >= HEADER_NOTE_MIN)
                .map(|d| format!(" ({})", fit(d, room.saturating_sub(3))))
                .unwrap_or_default();
            let note_width = note.chars().count();
            let mut suffix = String::new();
            if *collapsed && *count > 0 {
                let count_text = format!(" {count}");
                if suffix.chars().count() + count_text.chars().count() + note_width <= room {
                    suffix.push_str(&count_text);
                }
            }
            if let Some(kind) = remote {
                let kind_text = format!(" [{kind}]");
                if suffix.chars().count() + kind_text.chars().count() + note_width <= room {
                    suffix.push_str(&kind_text);
                }
            }
            Line::from(vec![
                Span::styled(name, Style::default().fg(name_colour)),
                Span::styled(arrow, Style::default().fg(Color::DarkGray)),
                Span::styled(note, Style::default().fg(Color::DarkGray)),
                Span::styled(suffix, Style::default().fg(Color::DarkGray)),
            ])
        }
        SidebarRow::Session {
            id,
            title,
            updated_at,
            branch,
            owns_worktree,
            requests,
        } => {
            let attention = attention_of(app, id);
            let (marker, colour) = attention_marker(attention);
            // Branch marker and request count, e.g. `Y3`; the count is left
            // out when it is zero or not known yet.
            let branch_part = branch.as_ref().map(|_| match requests.filter(|n| *n > 0) {
                Some(n) => format!("{BRANCH_MARK}{n}"),
                None => BRANCH_MARK.to_string(),
            });
            // `#N`: the session has had N pull/merge requests; blue while
            // one is bound, grey when none is any more, and `#!N` when
            // Auto-fix waits on the user (FR-22).
            let total = app.sidebar.request_totals.get(id).copied().unwrap_or(0);
            let request_part = match (app.sidebar.bound.get(id), total) {
                (Some(true), n) => Some((format!("#!{}", n.max(1)), Color::Yellow)),
                (Some(false), n) => Some((format!("#{}", n.max(1)), Color::Blue)),
                (None, 0) => None,
                (None, n) => Some((format!("#{n}"), Color::DarkGray)),
            };
            // Right edge: `[age ][#][branch ]state`. Indent of two before the
            // title and one space after it.
            let branch_width = branch_part.as_ref().map_or(0, |b| b.len() + 1);
            let request_width = request_part.as_ref().map_or(0, |(p, _)| p.len() + 1);
            let fixed = 2 + 1 + branch_width + request_width + 1;
            let mut age = relative_age(updated_at, now);
            // When space is short the age goes first, then the title is cut.
            if width.saturating_sub(fixed + age.len() + 1) < MIN_TITLE_WIDTH {
                age.clear();
            }
            let age_width = if age.is_empty() { 0 } else { age.len() + 1 };
            let title_width = width.saturating_sub(fixed + age_width);
            let title = fit(title, title_width);
            let pad = title_width.saturating_sub(title.chars().count());
            let focused = app.session.session_id.as_deref() == Some(id.as_str());
            // Focused: cyan bold. Needing a look (unread, waiting): white
            // bold. Others: white, brighter than the headings above them.
            let title_style = if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if matches!(attention, Attention::Unread | Attention::Waiting) {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            // A selected session is marked with `*` in place of the indent.
            let lead = if app.sidebar.selected.contains(id) {
                Span::styled("* ", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("  ")
            };
            let mut spans = vec![
                lead,
                Span::styled(title, title_style),
                Span::raw(" ".repeat(pad + 1)),
            ];
            if !age.is_empty() {
                spans.push(Span::styled(age, Style::default().fg(Color::DarkGray)));
                spans.push(Span::raw(" "));
            }
            if let Some((part, colour)) = request_part {
                spans.push(Span::styled(part, Style::default().fg(colour)));
                spans.push(Span::raw(" "));
            }
            if let Some(part) = branch_part {
                // A session with its own worktree stands out from one that
                // only records a branch.
                let branch_colour = if *owns_worktree {
                    Color::Magenta
                } else {
                    Color::DarkGray
                };
                spans.push(Span::styled(part, Style::default().fg(branch_colour)));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                marker.to_string(),
                Style::default().fg(colour),
            ));
            Line::from(spans)
        }
    }
}
