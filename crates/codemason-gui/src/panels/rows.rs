//! Shared drawing for the line-based side panels (Profile, Tasks, Memory,
//! Active agents, Teams).
//!
//! Each of those panels shows what the TUI paints as a list of coloured
//! monospace lines. A panel builds its lines as [`Line`]s and draws them
//! with [`show_lines`], which virtualises the list (`ScrollArea::show_rows`:
//! only the rows in view are laid out), highlights the cursor row, keeps it
//! in view when it moves, follows the App's scroll offset when a key
//! (Ctrl+PageUp/PageDown) moved it, and reports a click on a clickable row.
//! [`label_panel`] then gives the drawn list its accessible role and label.
//!
//! Characters the font cannot draw are drawn as ASCII stand-ins
//! ([`drawable`]).
//!
//! Dependencies: `egui` and [`crate::styled_text`].

use egui::text::LayoutJob;
use egui::{Color32, FontId, Role, ScrollArea, TextFormat, WidgetInfo};

use crate::styled_text::{FONT_SIZE, drawable};

/// One drawn line: coloured monospace sections, as a TUI line's spans.
#[derive(Clone, Debug, Default)]
pub struct Line {
    sections: Vec<(String, Color32)>,
    hover: Option<String>,
}

impl Line {
    /// An empty line.
    ///
    /// # Returns
    ///
    /// A line with no sections; it draws as a blank row.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `text` drawn in `color`.
    ///
    /// # Arguments
    ///
    /// * `text` - the section's text.
    /// * `color` - its colour.
    ///
    /// # Returns
    ///
    /// The line with the section added.
    #[must_use]
    pub fn push(mut self, text: impl Into<String>, color: Color32) -> Self {
        self.sections.push((text.into(), color));
        self
    }

    /// Show `text` when the row is hovered, in place of the row's own text:
    /// the full values of columns the row cuts to a fixed width.
    ///
    /// # Arguments
    ///
    /// * `text` - the hover text.
    ///
    /// # Returns
    ///
    /// The line with its hover text set.
    #[must_use]
    pub fn hover(mut self, text: impl Into<String>) -> Self {
        self.hover = Some(text.into());
        self
    }

    /// The line's text, without colours.
    ///
    /// # Returns
    ///
    /// Every section's text, in order.
    #[must_use]
    pub fn text(&self) -> String {
        self.sections
            .iter()
            .map(|(text, _)| text.as_str())
            .collect()
    }

    /// The line laid out for egui, on `background` when it is highlighted.
    fn job(&self, ctx: &egui::Context, background: Option<Color32>) -> LayoutJob {
        let font = FontId::monospace(FONT_SIZE);
        let mut job = LayoutJob::default();
        for (text, color) in &self.sections {
            let text: String = text
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            let mut format = TextFormat::simple(font.clone(), *color);
            if let Some(background) = background {
                format.background = background;
            }
            job.append(&drawable(ctx, &font, &text), 0.0, format);
        }
        if job.sections.is_empty() {
            // A blank row still takes its height.
            job.append(" ", 0.0, TextFormat::simple(font, Color32::TRANSPARENT));
        }
        job
    }
}

/// How a list follows a scroll offset the App keeps (in lines), changed by
/// the keys app-core scrolls side panels with (Ctrl+PageUp/PageDown).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    /// The App keeps no offset for this list.
    None,
    /// Lines from the bottom: a larger offset shows earlier lines (the TUI's
    /// Profile and Tasks panels).
    FromBottom(u16),
    /// Lines from the top: a larger offset shows later lines (the TUI's
    /// Memory panel).
    FromTop(u16),
}

/// What [`show_lines`] drew.
#[derive(Clone, Copy, Debug)]
pub struct Drawn {
    /// The list's rectangle.
    pub rect: egui::Rect,
    /// The clickable row the user clicked this frame, if any.
    pub clicked: Option<usize>,
}

/// The height of one monospace row.
fn row_height(ui: &egui::Ui) -> f32 {
    ui.ctx()
        .fonts_mut(|fonts| fonts.row_height(&FontId::monospace(FONT_SIZE)))
}

/// Draw `count` lines, virtualised, in a scroll area named `salt`.
///
/// # Arguments
///
/// * `ui` - the pane's body.
/// * `salt` - the scroll area's id salt, unique per panel.
/// * `count` - how many lines the list has.
/// * `line` - builds line `index`; called for the rows in view only.
/// * `cursor` - the highlighted row, if any; it is scrolled into view when
///   it changes.
/// * `clickable` - whether row `index` takes clicks.
/// * `follow` - the App's scroll offset for this list, if it keeps one.
///
/// # Returns
///
/// The list's rectangle and the clicked row.
pub fn show_lines(
    ui: &mut egui::Ui,
    salt: &str,
    count: usize,
    line: impl Fn(usize) -> Line,
    cursor: Option<usize>,
    clickable: impl Fn(usize) -> bool,
    follow: Follow,
) -> Drawn {
    let height = row_height(ui);
    let stride = height + ui.spacing().item_spacing.y;
    let id = ui.make_persistent_id(salt);
    let memo = id.with("memo");
    let (last_cursor, last_offset) = ui
        .ctx()
        .data(|d| d.get_temp::<(Option<usize>, Option<u16>)>(memo))
        .unwrap_or((None, None));
    let offset = egui::scroll_area::State::load(ui.ctx(), id).map_or(0.0, |s| s.clamped_offset().y);
    let view = ui.available_height().max(stride);
    let mut target = None;
    let (follow_offset, sign) = match follow {
        Follow::None => (None, 0.0),
        Follow::FromBottom(lines) => (Some(lines), -1.0),
        Follow::FromTop(lines) => (Some(lines), 1.0),
    };
    if let (Some(now), Some(before)) = (follow_offset, last_offset)
        && now != before
    {
        let delta = f32::from(now) - f32::from(before);
        target = Some((sign * delta).mul_add(stride, offset).max(0.0));
    }
    if let Some(row) = cursor
        && last_cursor != cursor
    {
        let top = row as f32 * stride;
        if top < offset {
            target = Some(top);
        } else if top + stride > offset + view {
            target = Some(top + stride - view);
        }
    }
    ui.ctx()
        .data_mut(|d| d.insert_temp(memo, (cursor, follow_offset)));
    // A list scrolled from the bottom shows its tail, as the TUI does at
    // offset 0, and follows it as lines arrive.
    let mut scroll = ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .stick_to_bottom(matches!(follow, Follow::FromBottom(_)));
    if let Some(target) = target {
        scroll = scroll.vertical_scroll_offset(target);
    }
    let selection = ui.visuals().selection.bg_fill;
    let mut clicked = None;
    let output = scroll.show_rows(ui, height, count, |ui, range| {
        for index in range {
            let background = (cursor == Some(index)).then_some(selection);
            let drawn = line(index);
            let job = drawn.job(ui.ctx(), background);
            // A line wider than the pane is cut at its edge, as the TUI
            // clips it (hovering shows it whole), so a row's click target is
            // always on screen.
            let mut label = egui::Label::new(job).truncate();
            if clickable(index) {
                label = label.sense(egui::Sense::click());
            }
            if drawn.hover.is_some() {
                label = label.show_tooltip_when_elided(false);
            }
            let mut response = ui.add(label);
            if let Some(hover) = drawn.hover {
                response = response.on_hover_text(hover);
            }
            if response.clicked() {
                clicked = Some(index);
            }
        }
    });
    Drawn {
        rect: output.inner_rect,
        clicked,
    }
}

/// The height a key legend drawn with `ui.weak(text)` under a list takes,
/// wrapped to the pane's width, plus the gap above it: the room the list
/// leaves so no legend line is pushed below the pane.
///
/// # Arguments
///
/// * `ui` - the pane's body, whose available width the legend wraps to.
/// * `text` - the legend.
///
/// # Returns
///
/// The legend's height in points.
pub fn legend_height(ui: &egui::Ui, text: &str) -> f32 {
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        ui.available_width(),
        egui::TextStyle::Body,
    );
    galley.size().y + ui.spacing().item_spacing.y
}

/// Give the list drawn in `rect` its accessible `role` and `label`.
///
/// # Arguments
///
/// * `ui` - the pane's body.
/// * `rect` - the list's rectangle.
/// * `salt` - an id salt unique per panel.
/// * `role` - the list's role, e.g. `Role::List`.
/// * `label` - its accessible label, e.g. `"Tasks"`.
pub fn label_panel(ui: &mut egui::Ui, rect: egui::Rect, salt: &str, role: Role, label: &str) {
    let list = ui.interact(rect, ui.scope_id().with(salt), egui::Sense::hover());
    list.widget_info(|| WidgetInfo::labeled(role, true, label));
}
