//! Material Design 3 list items, in a dense size for desktop lists.
//!
//! [`list_item`] draws one row: a headline, an optional supporting line
//! under it and optional trailing text at the right edge. A selected item
//! is filled with `secondary_container`; hovering or pressing one draws the
//! state layer.
//!
//! [`dense_item`] draws a compact legend row for breakdowns: an optional
//! disclosure triangle, an optional colour swatch, a headline and trailing
//! value columns aligned at the right edge. An expandable row is clickable.
//!
//! [`dense_item_option`] draws the same row as one option of a list box: it
//! is clickable and reports whether it is the selected one.
//!
//! [`avatar_list_item`] draws a taller, three-line row for browsing
//! entries: a letter avatar, the headline with a muted subtitle, a one-line
//! supporting text, a meta line the caller fills (chips, badges), and a
//! trailing slot (an action button). The selected item is filled with
//! `secondary_container` and carries a leading `primary` accent bar.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{
    Align, Align2, Color32, CornerRadius, Label, Layout, Rect, Response, RichText, Role, Sense,
    Shape, Stroke, UiBuilder, Vec2, WidgetInfo,
};

use super::feedback::{SWATCH_RADIUS, SWATCH_SIZE, swatch_rect};
use super::{
    content_colour, elided_galley, label_large, paint_state_layer, state_opacity, text_galley,
};
use crate::metrics::{radius, space, text_size};
use crate::palette::{Palette, rgb};

/// Height of a one-line item, in points.
pub const ONE_LINE_HEIGHT: f32 = 40.0;

/// Height of a two-line item (headline and supporting text), in points.
pub const TWO_LINE_HEIGHT: f32 = 56.0;

/// Horizontal padding inside an item, in points.
const PADDING: f32 = 16.0;

/// Smallest gap between the text and the trailing text, in points.
const TRAILING_GAP: f32 = 16.0;

/// Height of a [`dense_item`], in points.
pub const DENSE_HEIGHT: f32 = 22.0;

/// Width of each trailing value column of a [`dense_item`], in points.
pub const DENSE_COLUMN: f32 = 52.0;

/// Width of the disclosure triangle's slot in a [`dense_item`], in points.
const DISCLOSURE_SLOT: f32 = 14.0;

/// Gap between a [`dense_item`]'s leading parts, in points.
const DENSE_GAP: f32 = 6.0;

/// Whether a [`dense_item`] opens to show rows under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disclosure {
    /// The row has nothing under it; it is not clickable.
    None,
    /// The row has rows under it, hidden; a click shows them.
    Collapsed,
    /// The row has rows under it, shown; a click hides them.
    Expanded,
}

/// Draw a list item.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the item fills its available width.
/// * `palette` - the colours.
/// * `headline` - the item's text, also its accessible name.
/// * `supporting` - an optional second line under the headline.
/// * `trailing` - optional trailing text, e.g. a size or a price.
/// * `selected` - whether the item is the selected one.
///
/// # Returns
///
/// The item's response; `clicked()` and `double_clicked()` report clicks.
pub fn list_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    headline: &str,
    supporting: Option<&str>,
    trailing: Option<&str>,
    selected: bool,
) -> Response {
    list_item_with(
        ui,
        palette,
        headline,
        supporting,
        trailing,
        selected,
        |_| (),
    )
    .0
}

/// Draw a list item with a trailing slot for a widget (a badge, an action
/// button) beside the text.
///
/// The text column stops before the slot, so a row with a wide slot cuts
/// its headline with an ellipsis instead of running under it. The slot's
/// widgets are drawn after the item, so a click on one is the slot's, not
/// the item's.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the item fills its available width.
/// * `palette` - the colours.
/// * `headline` - the item's text, also its accessible name.
/// * `supporting` - an optional second line under the headline.
/// * `trailing` - optional trailing text, left of the slot.
/// * `selected` - whether the item is the selected one.
/// * `slot` - draws the trailing slot; its value is returned.
///
/// # Returns
///
/// The item's response (`clicked()` and `double_clicked()` report clicks on
/// the item), and what `slot` returned.
pub fn list_item_with<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    headline: &str,
    supporting: Option<&str>,
    trailing: Option<&str>,
    selected: bool,
    slot: impl FnOnce(&mut egui::Ui) -> R,
) -> (Response, R) {
    let m3 = &palette.m3;
    let height = if supporting.is_some() {
        TWO_LINE_HEIGHT
    } else {
        ONE_LINE_HEIGHT
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(Role::ListItem, ui.is_enabled(), selected, headline));
    // egui passes `selected` on for tabs and options only; a list item
    // states it itself, so assistive technology hears the selected row.
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_selected(selected);
        // The painted second line and trailing text, for screen readers.
        let description: Vec<&str> = [supporting, trailing].into_iter().flatten().collect();
        if !description.is_empty() {
            node.set_description(description.join(", "));
        }
    });
    // The fill and the state layer go under the text and the slot; they are
    // painted last, once the slot's width is known.
    let background = ui.painter().add(Shape::Noop);
    // The slot is laid out first, for its width, in a right-to-left ui at
    // the row's right edge.
    let mut slot_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                egui::pos2(rect.center().x, rect.top()),
                egui::pos2(rect.right() - PADDING, rect.bottom()),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    let trailed = slot(&mut slot_ui);
    let slot_rect = slot_ui.min_rect();
    // An empty slot leaves its ui's min rect at the right edge, zero-sized.
    let slot_width = if slot_rect.size() == Vec2::ZERO {
        0.0
    } else {
        (rect.right() - PADDING - slot_rect.left()).max(0.0) + TRAILING_GAP
    };
    if !ui.is_rect_visible(rect) {
        return (response, trailed);
    }
    let content = content_colour(
        ui,
        palette,
        rgb(if selected {
            m3.on_secondary_container
        } else {
            m3.on_surface
        }),
    );
    let variant = content_colour(ui, palette, rgb(m3.on_surface_variant));
    let trail =
        trailing.map(|t| text_galley(ui, t, egui::FontId::proportional(text_size::SMALL), variant));
    let trail_width = trail.as_ref().map_or(0.0, |t| t.size().x + TRAILING_GAP);
    let text_right = rect.right() - PADDING - slot_width - trail_width;
    let clip = rect.with_max_x(text_right.max(rect.left()));
    // Text wider than the row is cut with an ellipsis, and hovering shows
    // it whole.
    let text_width = clip.width() - PADDING;
    let head = elided_galley(
        ui,
        headline,
        egui::FontId::proportional(text_size::BODY),
        content,
        text_width,
    );
    let sub = supporting.map(|s| {
        elided_galley(
            ui,
            s,
            egui::FontId::proportional(text_size::SMALL),
            variant,
            text_width,
        )
    });
    let cut = head.elided || sub.as_ref().is_some_and(|s| s.elided);
    let block = head.size().y + sub.as_ref().map_or(0.0, |s| s.size().y);
    let mut y = rect.center().y - block / 2.0;
    let x = rect.left() + PADDING;
    let painter = ui.painter().with_clip_rect(clip);
    let head_height = head.size().y;
    painter.galley(egui::pos2(x, y), head, content);
    y += head_height;
    if let Some(sub) = sub {
        painter.galley(egui::pos2(x, y), sub, variant);
    }
    if let Some(trail) = trail {
        let pos = egui::pos2(
            rect.right() - PADDING - slot_width - trail.size().x,
            rect.center().y - trail.size().y / 2.0,
        );
        ui.painter().galley(pos, trail, variant);
    }
    let corner = CornerRadius::same(radius::SM);
    let mut shapes = Vec::new();
    if selected {
        shapes.push(Shape::rect_filled(
            rect,
            corner,
            rgb(m3.secondary_container),
        ));
    }
    let opacity = state_opacity(&response);
    if opacity > 0.0 && ui.is_enabled() {
        shapes.push(Shape::rect_filled(
            rect,
            corner,
            content.gamma_multiply(opacity),
        ));
    }
    ui.painter().set(background, Shape::Vec(shapes));
    let response = if cut {
        let full = supporting.map_or_else(|| headline.to_owned(), |s| format!("{headline}\n{s}"));
        response.on_hover_text(full)
    } else {
        response
    };
    (response, trailed)
}

/// Draw a dense legend row.
///
/// The row reads, left to right: `indent`, the disclosure triangle (its
/// slot is kept for [`Disclosure::None`] too, so headlines line up), the
/// `swatch` square when given, the headline (cut at the columns) and the
/// `trailing` values, each right-aligned in a [`DENSE_COLUMN`]-wide column
/// at the right edge. The accessible name is the headline followed by the
/// values.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the row fills its available width.
/// * `palette` - the colours.
/// * `indent` - space before the row's content, in points.
/// * `disclosure` - whether the row opens; an openable row is clickable.
/// * `swatch` - the colour of the leading swatch, if any.
/// * `headline` - the row's text.
/// * `trailing` - the value columns, left to right.
///
/// # Returns
///
/// The row's response; `clicked()` reports a click on an openable row.
pub fn dense_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    indent: f32,
    disclosure: Disclosure,
    swatch: Option<Color32>,
    headline: &str,
    trailing: &[&str],
) -> Response {
    dense_row(
        ui, palette, indent, disclosure, swatch, headline, trailing, None,
    )
}

/// Draw a dense row as one option of a list box: [`dense_item`]'s row, made
/// clickable and marked when it is the selected option (filled with
/// `secondary_container`, as a selected [`list_item`] is).
///
/// The row reports [`Role::ListBoxOption`], so a list of them reads as the
/// options of the list box the caller labels.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the row fills its available width.
/// * `palette` - the colours.
/// * `indent` - space before the row's content, in points.
/// * `headline` - the row's text, also its accessible name.
/// * `selected` - whether the row is the option the list box has selected.
///
/// # Returns
///
/// The row's response; `clicked()` reports a click on it.
pub fn dense_item_option(
    ui: &mut egui::Ui,
    palette: &Palette,
    indent: f32,
    headline: &str,
    selected: bool,
) -> Response {
    dense_row(
        ui,
        palette,
        indent,
        Disclosure::None,
        None,
        headline,
        &[],
        Some(selected),
    )
}

/// Draw a dense row: [`dense_item`]'s layout, as a plain row when `option`
/// is `None` and as a list-box option carrying its selection otherwise.
fn dense_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    indent: f32,
    disclosure: Disclosure,
    swatch: Option<Color32>,
    headline: &str,
    trailing: &[&str],
    option: Option<bool>,
) -> Response {
    let m3 = &palette.m3;
    let sense = if option.is_some() || disclosure != Disclosure::None {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), DENSE_HEIGHT), sense);
    let name: String = std::iter::once(headline)
        .chain(trailing.iter().copied())
        .collect::<Vec<_>>()
        .join("  ");
    match option {
        Some(selected) => response.widget_info(|| {
            WidgetInfo::selected(Role::ListBoxOption, ui.is_enabled(), selected, &name)
        }),
        None => {
            response.widget_info(|| WidgetInfo::labeled(Role::ListItem, ui.is_enabled(), &name))
        }
    }
    if disclosure != Disclosure::None {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_expanded(disclosure == Disclosure::Expanded);
        });
    }
    if let Some(selected) = option {
        // egui passes `selected` on for tabs and options only; a list-box
        // option states it itself, so assistive technology hears it.
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_selected(selected);
        });
    }
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let content = content_colour(ui, palette, rgb(m3.on_surface));
    let variant = content_colour(ui, palette, rgb(m3.on_surface_variant));
    if option == Some(true) {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(radius::XS),
            rgb(m3.secondary_container),
        );
    }
    if option.is_some() || disclosure != Disclosure::None {
        paint_state_layer(ui, &response, CornerRadius::same(radius::XS), content);
    }
    let centre_y = rect.center().y;
    let mut x = rect.left() + indent;
    let painter = ui.painter();
    let size = 4.0;
    let tip = egui::pos2(x + DISCLOSURE_SLOT / 2.0, centre_y);
    let points = match disclosure {
        Disclosure::None => Vec::new(),
        Disclosure::Collapsed => vec![
            tip + Vec2::new(-size / 2.0, -size),
            tip + Vec2::new(size, 0.0),
            tip + Vec2::new(-size / 2.0, size),
        ],
        Disclosure::Expanded => vec![
            tip + Vec2::new(-size, -size / 2.0),
            tip + Vec2::new(size, -size / 2.0),
            tip + Vec2::new(0.0, size),
        ],
    };
    if !points.is_empty() {
        painter.add(Shape::convex_polygon(points, variant, Stroke::NONE));
    }
    x += DISCLOSURE_SLOT;
    if let Some(colour) = swatch {
        let square = swatch_rect(egui::pos2(x, centre_y - SWATCH_SIZE / 2.0));
        painter.rect_filled(square, CornerRadius::same(SWATCH_RADIUS), colour);
        x += SWATCH_SIZE + DENSE_GAP;
    }
    let columns_left = DENSE_COLUMN.mul_add(-(trailing.len() as f32), rect.right());
    let font = egui::FontId::proportional(text_size::SMALL + 1.0);
    let clip = rect.with_max_x((columns_left - DENSE_GAP).max(x));
    // A long name is cut with an ellipsis, and hovering shows it whole.
    let head = elided_galley(ui, headline, font.clone(), content, clip.right() - x);
    let cut = head.elided;
    painter.with_clip_rect(clip).galley(
        egui::pos2(x, centre_y - head.size().y / 2.0),
        head,
        content,
    );
    for (index, value) in trailing.iter().enumerate() {
        let galley = text_galley(ui, value, font.clone(), variant);
        let right = DENSE_COLUMN.mul_add((index + 1) as f32, columns_left);
        let pos = egui::pos2(right - galley.size().x, centre_y - galley.size().y / 2.0);
        painter.galley(pos, galley, variant);
    }
    if cut {
        return response.on_hover_text(headline);
    }
    response
}

/// Height of an avatar list item, in points: the 4 pt top and bottom insets
/// around the headline, the supporting line and a meta line of 20 pt dense
/// chips (about 63 pt of content), with a little room to spare.
pub const AVATAR_ITEM_HEIGHT: f32 = 72.0;

/// Diameter of an avatar list item's letter avatar, in points.
const AVATAR_SIZE: f32 = 40.0;

/// Width of the selected avatar item's leading accent bar, in points.
const ACCENT_WIDTH: f32 = 3.0;

/// Smallest gap between an avatar item's text and its trailing slot, in
/// points.
const AVATAR_TRAILING_GAP: f32 = 12.0;

/// Gap between an avatar item's text lines, in points.
const LINE_GAP: f32 = 2.0;

/// The text of an [`avatar_list_item`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarItem<'a> {
    /// The item's name, also its accessible name; its first letter is the
    /// avatar.
    pub headline: &'a str,
    /// Muted text after the headline (e.g. a version or a category),
    /// truncated to fit.
    pub subtitle: &'a str,
    /// The second line (e.g. a description), truncated to one line.
    pub supporting: &'a str,
    /// Whether the item is the selected one.
    pub selected: bool,
}

/// Draw a three-line list item with a letter avatar and a trailing slot.
///
/// The row is [`AVATAR_ITEM_HEIGHT`] points high and fills the available
/// width. Its first line is the headline and subtitle, the second the
/// supporting text, the third whatever `meta` draws (in a horizontal ui,
/// clipped to the text column). `trailing` draws at the right edge,
/// vertically centred, in a right-to-left ui. A click on the meta line
/// (a chip or badge in it) counts as a click on the item; a click on the
/// trailing slot does not.
///
/// # Arguments
///
/// * `ui` - the ui to draw in.
/// * `palette` - the colours.
/// * `item` - the text and selected state.
/// * `meta` - draws the third line.
/// * `trailing` - draws the trailing slot; its value is returned.
///
/// # Returns
///
/// The item's response (`clicked()` and `double_clicked()` report clicks on
/// the item, the meta line included), and what `trailing` returned.
pub fn avatar_list_item<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    item: &AvatarItem<'_>,
    meta: impl FnOnce(&mut egui::Ui),
    trailing: impl FnOnce(&mut egui::Ui) -> R,
) -> (Response, R) {
    let m3 = &palette.m3;
    let size = Vec2::new(ui.available_width(), AVATAR_ITEM_HEIGHT);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            Role::ListItem,
            ui.is_enabled(),
            item.selected,
            item.headline,
        )
    });
    // egui passes `selected` on for tabs and options only; the item states
    // it itself, so assistive technology hears the selected row.
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_selected(item.selected);
        let description: Vec<&str> = [item.subtitle, item.supporting]
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect();
        if !description.is_empty() {
            node.set_description(description.join(", "));
        }
    });
    // The fill and state layer go under everything; they are painted last,
    // once the meta line's clicks are known.
    let background = ui.painter().add(Shape::Noop);
    let content = rgb(if item.selected {
        m3.on_secondary_container
    } else {
        m3.on_surface
    });
    let variant = rgb(m3.on_surface_variant);

    let centre = egui::pos2(rect.left() + PADDING + AVATAR_SIZE / 2.0, rect.center().y);
    let (avatar_fill, avatar_text) = if item.selected {
        (rgb(m3.primary_container), rgb(m3.on_primary_container))
    } else {
        (rgb(m3.secondary_container), rgb(m3.on_secondary_container))
    };
    ui.painter()
        .circle_filled(centre, AVATAR_SIZE / 2.0, avatar_fill);
    let letter: String = item
        .headline
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .collect();
    ui.painter().text(
        centre,
        Align2::CENTER_CENTER,
        letter,
        label_large(ui.ctx()),
        avatar_text,
    );

    // The trailing slot is laid out first, for its width, but its widgets
    // go on top of the item's, so its clicks stay its own.
    let mut slot = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                egui::pos2(rect.center().x, rect.top()),
                egui::pos2(rect.right() - PADDING, rect.bottom()),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    let trailed = trailing(&mut slot);
    let text_left = rect.left() + PADDING + AVATAR_SIZE + PADDING;
    let text_right = (slot.min_rect().left() - AVATAR_TRAILING_GAP).max(text_left);
    let text_rect = Rect::from_min_max(
        egui::pos2(text_left, rect.top() + space::XS),
        egui::pos2(text_right, rect.bottom() - space::XS),
    );
    let mut text = ui.new_child(
        UiBuilder::new()
            .max_rect(text_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    text.set_clip_rect(text_rect.intersect(ui.clip_rect()));
    text.style_mut().interaction.selectable_labels = false;
    text.spacing_mut().item_spacing = Vec2::new(space::SM, LINE_GAP);
    text.horizontal(|ui| {
        // The headline identifies the row: it keeps the width first, cut
        // with an ellipsis (hover shows it whole) when the trailing slot is
        // wide.
        ui.add(Label::new(RichText::new(item.headline).strong().color(content)).truncate());
        ui.add(
            Label::new(
                RichText::new(item.subtitle)
                    .size(text_size::SMALL)
                    .color(variant),
            )
            .truncate(),
        );
    });
    text.add(Label::new(RichText::new(item.supporting).color(variant)).truncate());
    let meta_rect = text.horizontal(meta).response.rect;
    // Registered after the meta line's widgets, so it takes their clicks:
    // a chip in a row selects the row.
    let meta_hit = ui.interact(
        meta_rect.intersect(text_rect),
        response.id.with("avatar_item_meta"),
        Sense::click(),
    );
    response = response.union(meta_hit);
    if ui.is_rect_visible(rect) {
        let mut shapes = Vec::new();
        if item.selected {
            shapes.push(Shape::rect_filled(
                rect,
                CornerRadius::ZERO,
                rgb(m3.secondary_container),
            ));
            shapes.push(Shape::rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(ACCENT_WIDTH, AVATAR_ITEM_HEIGHT)),
                CornerRadius::ZERO,
                rgb(m3.primary),
            ));
        }
        let opacity = state_opacity(&response);
        if opacity > 0.0 && ui.is_enabled() {
            shapes.push(Shape::rect_filled(
                rect,
                CornerRadius::ZERO,
                content.gamma_multiply(opacity),
            ));
        }
        ui.painter().set(background, Shape::Vec(shapes));
    }
    (response, trailed)
}
