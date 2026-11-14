//! Painting of the sidebar's project properties dialog.
//!
//! The dialog state and its key/mouse/paste handling are frontend-neutral and
//! live in [`crate::project_properties`]; this module centres the dialog,
//! records its area for mouse hits and paints it.
//!
//! Depends on `ratatui` and [`crate::adapters`] (`Rect` -> `Area`).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::adapters::rect_to_area;
use crate::app::App;
use crate::project_properties::{Field, LIST_ROWS, ProjectProperties};

/// Width of the dialog, including its border.
const WIDTH: u16 = 66;

/// Paints the dialog, centred, when it is open.
pub fn render(frame: &mut Frame, app: &mut App) {
    let Some(dialog) = app.sidebar.properties.as_mut() else {
        return;
    };
    let screen = frame.area();
    let width = WIDTH.min(screen.width);
    let height = 14u16.min(screen.height);
    let area = Rect {
        x: screen.x + (screen.width - width) / 2,
        y: screen.y + (screen.height - height) / 2,
        width,
        height,
    };
    dialog.area = rect_to_area(area);
    let inner = usize::from(width.saturating_sub(4));
    let label = |text: &str, field: Field| {
        let style = if dialog.focus == field {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        Span::styled(format!(" {text:<12}"), style)
    };
    let value_width = inner.saturating_sub(13);
    let boxed = |text: &str, focused: bool| {
        let shown = crate::sessions_sidebar::fit(text, value_width.saturating_sub(2));
        let style = if focused {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };
        Span::styled(
            format!("[{shown:<w$}]", w = value_width.saturating_sub(2)),
            style,
        )
    };
    let description = if dialog.focus == Field::Description {
        caret_window(
            &dialog.description,
            dialog.caret,
            value_width.saturating_sub(2),
        )
    } else {
        dialog.description.clone()
    };
    let dim = Style::default().fg(Color::DarkGray);
    let shown_as = if dialog.description.trim().is_empty() {
        format!("shown as: {}", dialog.project_name)
    } else {
        format!(
            "shown as: {} ({})",
            dialog.project_name,
            dialog.description.trim()
        )
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            label("Description", Field::Description),
            boxed(&description, dialog.focus == Field::Description),
        ]),
        Line::from(Span::styled(
            format!(
                "{:13}{}",
                "",
                crate::sessions_sidebar::fit(&shown_as, value_width)
            ),
            dim,
        )),
        Line::from(""),
        Line::from(vec![
            label("Backend", Field::Backend),
            boxed(
                &format!("{} v", dialog.backends[dialog.backend].label),
                dialog.focus == Field::Backend,
            ),
        ]),
        Line::from(Span::styled(
            format!("{:13}new sessions only; existing ones keep theirs", ""),
            dim,
        )),
        Line::from(""),
        Line::from(vec![
            label("Model", Field::Model),
            boxed(
                &format!("{} v", dialog.models[dialog.model].label),
                dialog.focus == Field::Model,
            ),
        ]),
        Line::from(""),
    ];
    match &dialog.error {
        Some(error) => lines.push(Line::from(Span::styled(
            format!(" {}", crate::sessions_sidebar::fit(error, inner)),
            Style::default().fg(Color::Red),
        ))),
        None => lines.push(Line::from("")),
    }
    lines.push(Line::from(Span::styled(
        " Tab move  Left/Right change  Space list  Enter save  Esc cancel",
        dim,
    )));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" Properties: {} ", dialog.project_name));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    if let Some(cursor) = dialog.list {
        paint_list(frame, dialog, cursor, area);
    }
}

/// `text` with a `_` caret at `caret`, cut to the `width` characters
/// around the caret so the caret and the text next to it stay visible
/// while the rest scrolls out of view.
fn caret_window(text: &str, caret: usize, width: usize) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    let caret = caret.min(chars.len());
    chars.insert(caret, '_');
    let start = (caret + 1).saturating_sub(width);
    chars.into_iter().skip(start).take(width).collect()
}

/// Paints the open choice list below its field.
fn paint_list(frame: &mut Frame, dialog: &ProjectProperties, cursor: usize, dialog_area: Rect) {
    let Some((choices, chosen)) = dialog.focused_choices() else {
        return;
    };
    let row = match dialog.focus {
        Field::Backend => 5,
        Field::Description | Field::Model => 8,
    };
    let rows = choices.len().min(LIST_ROWS);
    let screen = frame.area();
    let height = u16::try_from(rows + 2).unwrap_or(u16::MAX);
    let width = dialog_area.width.saturating_sub(14);
    let area = Rect {
        x: dialog_area.x + 13,
        y: (dialog_area.y + row).min(screen.y + screen.height.saturating_sub(height)),
        width,
        height: height.min(screen.height),
    };
    let top = cursor.saturating_sub(rows.saturating_sub(1));
    let text_width = usize::from(width.saturating_sub(2));
    let lines: Vec<Line> = choices
        .iter()
        .enumerate()
        .skip(top)
        .take(rows)
        .map(|(i, choice)| {
            let mark = if i == chosen { '*' } else { ' ' };
            let text = format!(
                "{mark} {}",
                crate::sessions_sidebar::fit(&choice.label, text_width.saturating_sub(2))
            );
            let style = if i == cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if choice.enabled {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(format!("{text:<text_width$}"), style))
        })
        .collect();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
        area,
    );
}
