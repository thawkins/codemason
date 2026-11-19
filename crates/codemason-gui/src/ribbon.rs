//! The word-processor style command frame: a title strip with a quick
//! access toolbar, and a ribbon of tabbed, grouped commands.
//!
//! Every command is one the window already has: a slash command
//! ([`MenuAction::Slash`]), a key press such as Esc ([`MenuAction::Key`]), a
//! keymap command such as a panel toggle ([`RibbonAction::Panel`]), or a file
//! dialog ([`RibbonAction::Files`]). The ribbon only collects them in one
//! place; [`crate::shell`] runs the [`RibbonAction`] a click returns, the
//! same way it runs the native menu's.
//!
//! The frame is drawn for themes with their own Material roles (the
//! `document-*` themes); other themes keep the plain `File` menu bar.
//!
//! The tab strip is laid out with craft-ui's [`craft_ui::tabs`]: tabs keep
//! their natural width ([`fit_whole`]), and each is a [`Tab`] with the
//! accessible tab role. Buttons use [`crate::material`]'s state layers.
//!
//! When the app draws the window frame ([`crate::title_bar`]), the title
//! strip is the window's title bar, with the caption buttons, on every
//! theme; then only the `document-*` themes add the quick access toolbar.
//!
//! Dependencies: `egui`, `craft-ui` (tabs), `craft-window` (the title bar),
//! `codemason-app-core` (key presses), and this crate's material
//! components, icons, menus and file dialogs.

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use craft_ui::tabs::{Tab, fit_whole};
use craft_window::Button;
use craft_window::policy::Frame;
use egui::{CornerRadius, Rect, Response, Role, Sense, Stroke, Vec2, WidgetInfo};

use crate::icons::Icon;
use crate::material::icon_button::{IconButtonKind, icon_button};
use crate::material::{
    content_colour, icon_galley, label_large, motion_time, paint_focus_ring, paint_state_layer,
    standard_easing, text_galley,
};
use crate::metrics::{space, text_size};
use crate::palette::{Palette, rgb};
use crate::platform::files::FileIntent;
use crate::platform::menu::MenuAction;
use crate::title_bar::{app_drawn, caption_buttons};

/// Height of the title strip, in points.
pub const TITLE_STRIP_HEIGHT: f32 = 40.0;

/// Height of the ribbon's tab strip, in points.
pub const TAB_HEIGHT: f32 = 36.0;

/// Height of the band of grouped commands under the tabs, in points.
pub const BAND_HEIGHT: f32 = 88.0;

/// Vertical padding of the band, in points.
const BAND_MARGIN: f32 = 4.0;

/// Width of a ribbon command button, in points.
const BUTTON_WIDTH: f32 = 72.0;

/// Height of a ribbon command button, in points.
const BUTTON_HEIGHT: f32 = 60.0;

/// Size of a ribbon command's icon, in points.
const BUTTON_ICON: f32 = 24.0;

/// Horizontal padding of a tab label, in points.
const TAB_PADDING: f32 = 16.0;

/// The tab the ribbon opens on: Home.
pub const HOME_TAB: usize = 1;

/// What a ribbon or quick access command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonAction {
    /// A key press or slash command, run as the native menu runs it.
    Menu(MenuAction),
    /// A keymap command by id (e.g. `panel.log`), sent as its key press.
    Panel(&'static str),
    /// A file dialog, whose answer the shell applies.
    Files(FileIntent),
}

/// One command button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RibbonItem {
    /// The button's label, also its accessible name.
    pub label: &'static str,
    /// The button's icon.
    pub icon: Icon,
    /// What a click runs.
    pub action: RibbonAction,
}

/// A labelled group of commands in a tab's band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RibbonGroup {
    /// The group's caption, drawn under its buttons.
    pub label: &'static str,
    /// The group's commands, left to right.
    pub items: &'static [RibbonItem],
}

/// One ribbon tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RibbonTab {
    /// The tab's label.
    pub label: &'static str,
    /// The tab's groups, left to right.
    pub groups: &'static [RibbonGroup],
}

/// A ribbon item running a slash command.
const fn slash(label: &'static str, icon: Icon, line: &'static str) -> RibbonItem {
    RibbonItem {
        label,
        icon,
        action: RibbonAction::Menu(MenuAction::Slash(line)),
    }
}

/// A ribbon item toggling a panel through its keymap command.
const fn panel(label: &'static str, icon: Icon, command: &'static str) -> RibbonItem {
    RibbonItem {
        label,
        icon,
        action: RibbonAction::Panel(command),
    }
}

/// A ribbon item opening a file dialog.
const fn files(label: &'static str, icon: Icon, intent: FileIntent) -> RibbonItem {
    RibbonItem {
        label,
        icon,
        action: RibbonAction::Files(intent),
    }
}

/// Stop the running turn: the Esc key press.
const STOP: RibbonItem = RibbonItem {
    label: "Stop",
    icon: Icon::Stop,
    action: RibbonAction::Menu(MenuAction::Key(KeyInput {
        key: Key::Esc,
        mods: Modifiers::NONE,
    })),
};

/// The quick access toolbar in the title strip.
pub const QUICK_ACCESS: &[RibbonItem] = &[
    slash("New session", Icon::Add, "/session new"),
    slash("Undo", Icon::Undo, "/undo"),
    slash("Copy transcript", Icon::Copy, "/clip"),
    STOP,
];

/// The ribbon's tabs.
pub const TABS: &[RibbonTab] = &[
    RibbonTab {
        label: "File",
        groups: &[
            RibbonGroup {
                label: "Project",
                items: &[
                    files("Open project", Icon::FolderOpen, FileIntent::OpenProject),
                    files("New folder", Icon::NewFolder, FileIntent::NewProjectDir),
                ],
            },
            RibbonGroup {
                label: "Application",
                items: &[
                    slash("About", Icon::Info, "/about"),
                    slash("Quit", Icon::Quit, "/quit"),
                ],
            },
        ],
    },
    RibbonTab {
        label: "Home",
        groups: &[
            RibbonGroup {
                label: "Session",
                items: &[
                    slash("New session", Icon::Add, "/session new"),
                    slash("Sessions", Icon::Sessions, "/session list"),
                ],
            },
            RibbonGroup {
                label: "Edit",
                items: &[
                    slash("Undo", Icon::Undo, "/undo"),
                    slash("Redo", Icon::Redo, "/redo"),
                    slash("Copy transcript", Icon::Copy, "/clip"),
                ],
            },
            RibbonGroup {
                label: "Turn",
                items: &[STOP, slash("Compact", Icon::Compact, "/compact")],
            },
        ],
    },
    RibbonTab {
        label: "Insert",
        groups: &[RibbonGroup {
            label: "Attach",
            items: &[
                files("Images", Icon::Image, FileIntent::Attach),
                slash("Template", Icon::Template, "/template"),
            ],
        }],
    },
    RibbonTab {
        label: "Agent",
        groups: &[
            RibbonGroup {
                label: "Setup",
                items: &[
                    slash("Model", Icon::Model, "/model"),
                    slash("Agent", Icon::Agent, "/agent"),
                ],
            },
            RibbonGroup {
                label: "Run",
                items: &[slash("Loop", Icon::Loop, "/loop")],
            },
        ],
    },
    RibbonTab {
        label: "View",
        groups: &[
            RibbonGroup {
                label: "Panels",
                items: &[
                    panel("Log", Icon::Log, "panel.log"),
                    panel("Profile", Icon::Profile, "panel.profile"),
                    panel("Tasks", Icon::Tasks, "panel.tasks"),
                    panel("Memory", Icon::Memory, "panel.memory"),
                ],
            },
            RibbonGroup {
                label: "Look",
                items: &[slash("Theme", Icon::Theme, "/theme")],
            },
        ],
    },
    RibbonTab {
        label: "Review",
        groups: &[RibbonGroup {
            label: "Usage",
            items: &[
                slash("Cost", Icon::Cost, "/cost"),
                slash("Stats", Icon::Stats, "/stats"),
                slash("Doctor", Icon::Doctor, "/doctor"),
            ],
        }],
    },
];

/// What the ribbon remembers between frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RibbonState {
    /// The index into [`TABS`] of the open tab.
    pub tab: usize,
    /// Whether only the tab strip is shown.
    pub collapsed: bool,
}

impl Default for RibbonState {
    fn default() -> Self {
        Self {
            tab: HOME_TAB,
            collapsed: false,
        }
    }
}

/// Draw the title strip: the window title, the quick access toolbar at the
/// right, and, when the app draws the window frame, the caption buttons and
/// the drag area that makes the strip the window's title bar.
///
/// # Arguments
///
/// * `ui` - the strip's ui.
/// * `palette` - the colours.
/// * `title` - the title, e.g. `codemason - <project>`.
/// * `quick_access` - whether to show the quick access toolbar (the
///   `document-*` themes).
/// * `enabled` - whether the quick access toolbar can be used (not while an
///   app-core modal owns the window); the caption buttons always can.
/// * `frame` - who draws the window frame ([`crate::title_bar`]).
///
/// # Returns
///
/// The quick access command clicked this frame, if any.
pub fn title_strip_ui(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: &str,
    quick_access: bool,
    enabled: bool,
    frame: &Frame,
) -> Option<RibbonAction> {
    let mut chosen = None;
    let strip = Rect::from_min_size(
        ui.available_rect_before_wrap().min,
        Vec2::new(ui.available_width(), TITLE_STRIP_HEIGHT),
    );
    // The empty part of the strip moves the window; the controls drawn after
    // it keep their own clicks.
    if app_drawn(frame) {
        craft_window::title_bar::empty_area(ui, strip);
    }
    let (left, right): (&[Button], &[Button]) = match frame {
        Frame::Integrated(captions) => (&captions.layout.left, &captions.layout.right),
        Frame::Native | Frame::Inset(_) => (&[], &[]),
    };
    ui.set_height(TITLE_STRIP_HEIGHT);
    ui.horizontal_centered(|ui| {
        if let Frame::Inset(inset) = frame {
            ui.add_space(inset.leading);
        }
        // Outermost last: the leftmost button is the last of the list.
        let left: Vec<Button> = left.iter().rev().copied().collect();
        caption_buttons(ui, palette, &left);
        ui.label(
            egui::RichText::new(title)
                .size(text_size::BODY)
                .color(rgb(palette.m3.on_surface_variant)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let right: Vec<Button> = right.iter().rev().copied().collect();
            caption_buttons(ui, palette, &right);
            if !right.is_empty() && quick_access {
                ui.add_space(space::LG);
            }
            let items = if quick_access { QUICK_ACCESS } else { &[] };
            ui.add_enabled_ui(enabled, |ui| {
                for item in items.iter().rev() {
                    let response = icon_button(
                        ui,
                        palette,
                        IconButtonKind::Standard,
                        item.icon,
                        item.label,
                        None,
                    );
                    if response.clicked() {
                        chosen = Some(item.action);
                    }
                }
            });
        });
    });
    chosen
}

/// Draw the ribbon: the tab strip, and unless collapsed, the open tab's
/// band of grouped commands.
///
/// # Arguments
///
/// * `ui` - the ribbon's ui.
/// * `palette` - the colours.
/// * `state` - the open tab and collapsed flag; a tab click opens that tab
///   (and expands a collapsed ribbon), the chevron at the right collapses
///   or expands it.
///
/// # Returns
///
/// The command clicked this frame, if any.
pub fn ribbon_ui(
    ui: &mut egui::Ui,
    palette: &Palette,
    state: &mut RibbonState,
) -> Option<RibbonAction> {
    tab_strip(ui, palette, state);
    // The band slides open and shut: its height follows the openness, and
    // what does not fit yet is clipped.
    let ctx = ui.ctx().clone();
    let openness = ctx.animate_bool_with_time_and_easing(
        ui.scope_id().with("ribbon_band_open"),
        !state.collapsed,
        motion_time(&ctx),
        standard_easing,
    );
    if openness <= 0.0 {
        return None;
    }
    let tab = TABS.get(state.tab).or_else(|| TABS.first())?;
    let full = BAND_MARGIN.mul_add(2.0, BAND_HEIGHT);
    let top = ui.cursor().min;
    let width = ui.available_width();
    let (shown, _) = ui.allocate_exact_size(Vec2::new(width, full * openness), Sense::hover());
    let mut band_ui = ui.new_child(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(top, Vec2::new(width, full))),
    );
    band_ui.set_clip_rect(shown.intersect(ui.clip_rect()));
    band(&mut band_ui, palette, tab)
}

/// The tab strip with the collapse chevron at its right end.
fn tab_strip(ui: &mut egui::Ui, palette: &Palette, state: &mut RibbonState) {
    let m3 = &palette.m3;
    let size = Vec2::new(ui.available_width(), TAB_HEIGHT);
    let (strip, _) = ui.allocate_exact_size(size, Sense::hover());
    let font = label_large();
    let natural: Vec<f32> = TABS
        .iter()
        .map(|tab| {
            text_galley(ui, tab.label, font.clone(), rgb(m3.on_surface))
                .size()
                .x
        })
        .map(|width| TAB_PADDING.mul_add(2.0, width))
        .collect();
    let chevron = crate::material::icon_button::SIZE;
    let fit = fit_whole(&natural, state.tab, strip.width() - chevron, 0.0);
    let mut x = strip.left();
    let mut underline = None;
    for (index, width) in fit.shown {
        let Some(tab) = TABS.get(index) else { continue };
        let rect = Rect::from_min_size(egui::pos2(x, strip.top()), Vec2::new(width, TAB_HEIGHT));
        x += width;
        let on = state.tab == index && !state.collapsed;
        let id = ui.scope_id().with(("ribbon_tab", index));
        let response = Tab::new(id, tab.label, on)
            .focus_stroke(Stroke::new(2.0, rgb(m3.secondary)))
            .show_at(ui, rect, |ui, response| {
                let text_width = paint_tab(ui, palette, response, tab.label, on);
                if on {
                    underline = Some((rect.center().x, text_width));
                }
            });
        if response.clicked() {
            state.tab = index;
            state.collapsed = false;
        }
    }
    // The open tab's underline glides to the tab that opens.
    if let Some((centre, width)) = underline {
        let ctx = ui.ctx().clone();
        let time = motion_time(&ctx);
        let id = ui.scope_id().with("ribbon_underline");
        let centre = ctx.animate_value_with_time(id.with("x"), centre, time);
        let width = ctx.animate_value_with_time(id.with("w"), width, time);
        let bar = Rect::from_center_size(
            egui::pos2(centre, strip.bottom() - 1.5),
            Vec2::new(width, 3.0),
        );
        ui.painter().rect_filled(bar, 1.5, rgb(m3.primary));
    }
    let chevron_rect = Rect::from_min_size(
        egui::pos2(strip.right() - chevron, strip.center().y - chevron / 2.0),
        Vec2::splat(chevron),
    );
    let (icon, label) = if state.collapsed {
        (Icon::ChevronDown, "Expand the ribbon")
    } else {
        (Icon::ChevronUp, "Collapse the ribbon")
    };
    let toggled = ui
        .scope_builder(egui::UiBuilder::new().max_rect(chevron_rect), |ui| {
            icon_button(ui, palette, IconButtonKind::Standard, icon, label, None).clicked()
        })
        .inner;
    if toggled {
        state.collapsed = !state.collapsed;
    }
}

/// Paint one tab: its state layer and label (in `primary` when it is open).
///
/// # Returns
///
/// The label's width, which the open tab's underline takes.
fn paint_tab(ui: &egui::Ui, palette: &Palette, response: &Response, label: &str, on: bool) -> f32 {
    let m3 = &palette.m3;
    let rect = response.rect;
    let colour = content_colour(
        ui,
        palette,
        if on {
            rgb(m3.primary)
        } else {
            rgb(m3.on_surface_variant)
        },
    );
    paint_state_layer(ui, response, CornerRadius::ZERO, colour);
    let text = text_galley(ui, label, label_large(), colour);
    let text_width = text.size().x;
    ui.painter()
        .galley(rect.center() - text.size() / 2.0, text, colour);
    text_width
}

/// The open tab's band: each group's buttons over its caption, groups
/// separated by `outline_variant` dividers.
fn band(ui: &mut egui::Ui, palette: &Palette, tab: &RibbonTab) -> Option<RibbonAction> {
    let m3 = &palette.m3;
    let mut chosen = None;
    egui::Frame::new()
        .fill(rgb(m3.surface_container))
        .corner_radius(CornerRadius::same(crate::metrics::radius::MD))
        .inner_margin(egui::Margin::symmetric(8, BAND_MARGIN as i8))
        .show(ui, |ui| {
            ui.set_height(BAND_HEIGHT);
            ui.horizontal(|ui| {
                for (index, group) in tab.groups.iter().enumerate() {
                    if index > 0 {
                        let rect = ui
                            .allocate_exact_size(Vec2::new(space::SM, BAND_HEIGHT), Sense::hover())
                            .0;
                        ui.painter().vline(
                            rect.center().x,
                            rect.y_range().shrink(space::SM),
                            Stroke::new(1.0, rgb(m3.outline_variant)),
                        );
                    }
                    let spacing = ui.spacing().item_spacing.x;
                    let count = group.items.len() as f32;
                    let width = BUTTON_WIDTH.mul_add(count, spacing * (count - 1.0).max(0.0));
                    let layout = egui::Layout::top_down(egui::Align::Center);
                    ui.allocate_ui_with_layout(Vec2::new(width, BAND_HEIGHT), layout, |ui| {
                        ui.horizontal(|ui| {
                            for item in group.items {
                                if ribbon_button(ui, palette, item).clicked() {
                                    chosen = Some(item.action);
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(group.label)
                                .size(text_size::SMALL)
                                .color(rgb(m3.on_surface_variant)),
                        );
                    });
                }
            });
        });
    chosen
}

/// One ribbon command: its icon over its label.
fn ribbon_button(ui: &mut egui::Ui, palette: &Palette, item: &RibbonItem) -> Response {
    let m3 = &palette.m3;
    let size = Vec2::new(BUTTON_WIDTH, BUTTON_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), item.label));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let corner = CornerRadius::same(crate::metrics::radius::SM);
    let content = content_colour(ui, palette, rgb(m3.on_surface));
    paint_state_layer(ui, &response, corner, content);
    let icon_colour = content_colour(ui, palette, rgb(m3.primary));
    let glyph = icon_galley(ui, item.icon, BUTTON_ICON, icon_colour);
    let icon_pos = egui::pos2(
        rect.center().x - glyph.size().x / 2.0,
        rect.top() + space::SM,
    );
    let icon_bottom = icon_pos.y + glyph.size().y;
    ui.painter().galley(icon_pos, glyph, icon_colour);
    let text = text_galley(
        ui,
        item.label,
        egui::FontId::proportional(text_size::SMALL),
        content,
    );
    let text_pos = egui::pos2(
        rect.center().x - text.size().x / 2.0,
        icon_bottom + space::XS,
    );
    ui.painter().galley(text_pos, text, content);
    paint_focus_ring(ui, &response, corner, palette);
    response
}
