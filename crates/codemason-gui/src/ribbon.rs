//! The word-processor style command frame: a title strip with a quick
//! access toolbar, and a ribbon of tabbed, grouped commands.
//!
//! Every command is one the window already has: a slash command
//! ([`MenuAction::Slash`]), a key press such as Esc ([`MenuAction::Key`]), a
//! panel toggle sent as its keymap command ([`RibbonAction::Panel`]), or a file
//! dialog ([`RibbonAction::Files`]). The ribbon only collects them in one
//! place; [`crate::shell`] runs the [`RibbonAction`] a click returns, the
//! same way it runs the native menu's.
//!
//! The frame is drawn on every theme, in that theme's Material roles
//! ([`crate::palette::Palette::m3`]).
//!
//! The tab strip is laid out with craft-ui's [`craft_ui::tabs`]: tabs keep
//! their natural width ([`fit_whole`]), and each is a [`Tab`] with the
//! accessible tab role. Buttons use [`crate::material`]'s state layers; a
//! panel toggle is filled in `secondary_container` while its panel is open.
//!
//! The band spans the window's width on every tab, so switching tabs never
//! changes its size, and each button is as wide as its label needs. In a
//! window too narrow for a tab's groups the band scrolls sideways.
//!
//! When the app draws the window frame ([`crate::title_bar`]), the title
//! strip is also the window's title bar, with the caption buttons.
//!
//! Whether the ribbon is collapsed is kept in `gui-ribbon.json` in the
//! codemason config directory ([`load_ribbon`], [`save_ribbon`]).
//!
//! Dependencies: `egui`, `craft-ui` (tabs), `craft-window` (the title bar),
//! `serde`/`serde_json` and `codemason-config` (the saved state),
//! `codemason-app-core` (key presses), and this crate's material
//! components, icons, menus and file dialogs.

use std::path::{Path, PathBuf};

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use craft_ui::tabs::{Tab, fit_whole};
use craft_window::Button;
use craft_window::policy::Frame;
use egui::{CornerRadius, Rect, Response, Role, Sense, Stroke, Vec2, WidgetInfo};
use serde::{Deserialize, Serialize};

use crate::dock::PaneId;
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

/// Height of the band of grouped commands under the tabs, in points: a
/// button over its group's caption.
pub const BAND_HEIGHT: f32 = 72.0;

/// Vertical padding of the band, in points.
const BAND_MARGIN: f32 = 4.0;

/// Narrowest a ribbon command button gets, in points; wider labels widen it.
const BUTTON_MIN_WIDTH: f32 = 56.0;

/// Height of a ribbon command button, in points: its icon over its label.
const BUTTON_HEIGHT: f32 = 52.0;

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
    /// Toggle a panel, sent as its keymap command's key press
    /// ([`crate::dock::panel_command`]); the button shows whether the panel is open.
    Panel(PaneId),
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
const fn panel(label: &'static str, icon: Icon, pane: PaneId) -> RibbonItem {
    RibbonItem {
        label,
        icon,
        action: RibbonAction::Panel(pane),
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
    slash("Resume", Icon::Resume, "/resume"),
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
                items: &[slash("Quit", Icon::Quit, "/quit")],
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
                    slash("Provider", Icon::Provider, "/provider"),
                    slash("Model", Icon::Model, "/model"),
                    slash("Agent", Icon::Agent, "/agent"),
                ],
            },
            RibbonGroup {
                label: "Run",
                items: &[
                    slash("Loop", Icon::Loop, "/loop"),
                    slash("Resume", Icon::Resume, "/resume"),
                ],
            },
            RibbonGroup {
                label: "Addons",
                items: &[
                    slash("Plugins", Icon::Plugins, "/plugins claude"),
                    slash("Connectors", Icon::Connectors, "/connectors claude"),
                    slash("MCP Servers", Icon::Mcp, "/mcp discover"),
                ],
            },
        ],
    },
    RibbonTab {
        label: "View",
        groups: &[
            RibbonGroup {
                label: "Panels",
                items: &[
                    panel("Log", Icon::Log, PaneId::Log),
                    panel("Profile", Icon::Profile, PaneId::Profile),
                    panel("Tasks", Icon::Tasks, PaneId::Tasks),
                    panel("Memory", Icon::Memory, PaneId::Memory),
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
    RibbonTab {
        label: "Help",
        groups: &[RibbonGroup {
            label: "Help",
            items: &[RibbonItem {
                label: "About",
                icon: Icon::Info,
                action: RibbonAction::Menu(MenuAction::About),
            }],
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
        // The buttons at the right are laid out first, so a long title is
        // shortened with an ellipsis in the room left, never pushing them
        // out of the window.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let right: Vec<Button> = right.iter().rev().copied().collect();
            caption_buttons(ui, palette, &right);
            if !right.is_empty() {
                ui.add_space(space::LG);
            }
            ui.add_enabled_ui(enabled, |ui| {
                for item in QUICK_ACCESS.iter().rev() {
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
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(title)
                            .size(text_size::BODY)
                            .color(rgb(palette.m3.on_surface_variant)),
                    )
                    .truncate(),
                );
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
/// * `shown` - whether a panel is open, which its toggle button shows.
///
/// # Returns
///
/// The command clicked this frame, if any.
pub fn ribbon_ui(
    ui: &mut egui::Ui,
    palette: &Palette,
    state: &mut RibbonState,
    shown: &dyn Fn(PaneId) -> bool,
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
    let (visible, _) = ui.allocate_exact_size(Vec2::new(width, full * openness), Sense::hover());
    let mut band_ui = ui.new_child(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(top, Vec2::new(width, full))),
    );
    band_ui.set_clip_rect(visible.intersect(ui.clip_rect()));
    band(&mut band_ui, palette, tab, shown)
}

/// The tab strip with the collapse chevron at its right end.
fn tab_strip(ui: &mut egui::Ui, palette: &Palette, state: &mut RibbonState) {
    let m3 = &palette.m3;
    let size = Vec2::new(ui.available_width(), TAB_HEIGHT);
    let (strip, _) = ui.allocate_exact_size(size, Sense::hover());
    let font = label_large(ui.ctx());
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
    let text = text_galley(ui, label, label_large(ui.ctx()), colour);
    let text_width = text.size().x;
    ui.painter()
        .galley(rect.center() - text.size() / 2.0, text, colour);
    text_width
}

/// The open tab's band, as wide as the window: each group's buttons over
/// its caption, groups separated by `outline_variant` dividers.
fn band(
    ui: &mut egui::Ui,
    palette: &Palette,
    tab: &RibbonTab,
    shown: &dyn Fn(PaneId) -> bool,
) -> Option<RibbonAction> {
    let m3 = &palette.m3;
    let mut chosen = None;
    egui::Frame::new()
        .fill(rgb(m3.surface_container))
        .corner_radius(CornerRadius::same(crate::metrics::radius::MD))
        .inner_margin(egui::Margin::symmetric(8, BAND_MARGIN as i8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_height(BAND_HEIGHT);
            // In a window too narrow for the groups the band scrolls
            // sideways (a plain mouse wheel scrolls it too), so no command
            // is cut off; at full width nothing scrolls.
            ui.style_mut().always_scroll_the_only_direction = true;
            egui::ScrollArea::horizontal()
                .id_salt("ribbon_band_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (index, group) in tab.groups.iter().enumerate() {
                            if index > 0 {
                                divider(ui, palette);
                            }
                            if let Some(action) = band_group(ui, palette, group, shown) {
                                chosen = Some(action);
                            }
                        }
                    });
                });
        });
    chosen
}

/// A divider between two groups, inset from the band's edges.
fn divider(ui: &mut egui::Ui, palette: &Palette) {
    let width = space::SM.mul_add(2.0, 1.0);
    let rect = ui
        .allocate_exact_size(Vec2::new(width, BAND_HEIGHT), Sense::hover())
        .0;
    ui.painter().vline(
        rect.center().x,
        rect.y_range().shrink(space::SM),
        Stroke::new(1.0, rgb(palette.m3.outline_variant)),
    );
}

/// One group: its buttons in a row, each as wide as its label needs, and
/// its caption centred under them.
fn band_group(
    ui: &mut egui::Ui,
    palette: &Palette,
    group: &RibbonGroup,
    shown: &dyn Fn(PaneId) -> bool,
) -> Option<RibbonAction> {
    let m3 = &palette.m3;
    let font = egui::FontId::proportional(text_size::SMALL);
    let label_colour = content_colour(ui, palette, rgb(m3.on_surface));
    let widths: Vec<f32> = group
        .items
        .iter()
        .map(|item| {
            let text = text_galley(ui, item.label, font.clone(), label_colour)
                .size()
                .x;
            space::SM.mul_add(2.0, text).max(BUTTON_MIN_WIDTH)
        })
        .collect();
    let caption_colour = content_colour(ui, palette, rgb(m3.on_surface_variant));
    let caption = text_galley(ui, group.label, font, caption_colour);
    let gaps = widths.len().saturating_sub(1) as f32;
    let row: f32 = space::XS.mul_add(gaps, widths.iter().sum::<f32>());
    let width = row.max(space::SM.mul_add(2.0, caption.size().x));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, BAND_HEIGHT), Sense::hover());
    let mut chosen = None;
    let mut x = rect.center().x - row / 2.0;
    for (item, item_width) in group.items.iter().zip(widths) {
        let button = Rect::from_min_size(
            egui::pos2(x, rect.top()),
            Vec2::new(item_width, BUTTON_HEIGHT),
        );
        x += item_width + space::XS;
        let on = match item.action {
            RibbonAction::Panel(pane) => Some(shown(pane)),
            RibbonAction::Menu(_) | RibbonAction::Files(_) => None,
        };
        if ribbon_button(ui, palette, item, button, on).clicked() {
            chosen = Some(item.action);
        }
    }
    let caption_pos = egui::pos2(
        rect.center().x - caption.size().x / 2.0,
        rect.bottom() - caption.size().y,
    );
    ui.painter().galley(caption_pos, caption, caption_colour);
    chosen
}

/// One ribbon command in `rect`: its icon over its label. A toggle (`on` is
/// `Some`) is filled in `secondary_container` while on.
fn ribbon_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    item: &RibbonItem,
    rect: Rect,
    on: Option<bool>,
) -> Response {
    let m3 = &palette.m3;
    let id = ui.scope_id().with(("ribbon_button", item.label));
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| match on {
        Some(on) => WidgetInfo::selected(Role::Button, ui.is_enabled(), on, item.label),
        None => WidgetInfo::labeled(Role::Button, ui.is_enabled(), item.label),
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let corner = CornerRadius::same(crate::metrics::radius::SM);
    let selected = on == Some(true);
    let (content, icon_colour) = if selected {
        ui.painter()
            .rect_filled(rect, corner, rgb(m3.secondary_container));
        let colour = rgb(m3.on_secondary_container);
        (colour, colour)
    } else {
        (rgb(m3.on_surface), rgb(m3.primary))
    };
    let content = content_colour(ui, palette, content);
    let icon_colour = content_colour(ui, palette, icon_colour);
    paint_state_layer(ui, &response, corner, content);
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

/// File name of the persisted ribbon state inside the codemason config
/// directory.
const RIBBON_FILE: &str = "gui-ribbon.json";

/// What `gui-ribbon.json` holds: whether the ribbon is collapsed.
#[derive(Debug, Serialize, Deserialize)]
struct RibbonFile {
    collapsed: bool,
}

/// The ribbon state file: `<config_dir>/codemason/gui-ribbon.json`.
///
/// # Returns
///
/// `None` when the platform config directory cannot be determined.
#[must_use]
pub fn ribbon_path() -> Option<PathBuf> {
    codemason_config::config::Config::global_config_dir().map(|dir| dir.join(RIBBON_FILE))
}

/// Load the ribbon state saved at `path`.
///
/// # Returns
///
/// The saved collapsed flag, on the Home tab; the default state (expanded)
/// when the file is missing, and with a warning when it cannot be read.
#[must_use]
pub fn load_ribbon(path: &Path) -> RibbonState {
    let collapsed = match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<RibbonFile>(&text) {
            Ok(file) => file.collapsed,
            Err(err) => {
                tracing::warn!(path = %path.display(), %err, "invalid ribbon state; using default");
                false
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "cannot read ribbon state; using default");
            false
        }
    };
    RibbonState {
        collapsed,
        ..RibbonState::default()
    }
}

/// Save `state`'s collapsed flag to `path`, creating its directory.
///
/// # Errors
///
/// Returns the I/O or serialization error.
pub fn save_ribbon(path: &Path, state: &RibbonState) -> anyhow::Result<()> {
    use anyhow::Context as _;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(&RibbonFile {
        collapsed: state.collapsed,
    })
    .context("serializing ribbon state")?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))
}
