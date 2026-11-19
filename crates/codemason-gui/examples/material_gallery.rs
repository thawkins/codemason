//! A window showing every Material Design 3 component in
//! `codemason_gui::material`, for reviewing them by eye and for screenshots.
//!
//! Run: `cargo run -p codemason-gui --example material_gallery -- <theme>`,
//! where `<theme>` is a built-in theme name (default `document-light`).
//!
//! Dependencies: `eframe`, `egui`, `craft-ui` (fonts) and
//! `codemason-app-core` (the theme catalog).

use codemason_app_core::theme_tokens::ThemeCatalog;
use codemason_gui::icons::Icon;
use codemason_gui::material::button::{ButtonKind, button};
use codemason_gui::material::card::{CardKind, card_frame};
use codemason_gui::material::chip::{ChipKind, chip};
use codemason_gui::material::dialog::{dialog_actions, dialog_body, dialog_frame, dialog_headline};
use codemason_gui::material::feedback::{linear_progress, plain_tooltip, slider, snackbar};
use codemason_gui::material::icon_button::{IconButtonKind, icon_button};
use codemason_gui::material::menu::{menu_frame, menu_item};
use codemason_gui::material::segmented::segmented_buttons;
use codemason_gui::material::sheet::{drawer_item, sheet_frame, sheet_header};
use codemason_gui::material::tabs::primary_tabs;
use codemason_gui::material::text_field::outlined_text_field;
use codemason_gui::palette::{Palette, rgb};
use codemason_gui::theme::visuals_for;

/// The gallery's state: the values its components edit.
struct Gallery {
    palette: Palette,
    tab: usize,
    segment: usize,
    filter: bool,
    toggle: bool,
    zoom: f32,
    name: String,
    empty: String,
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let palette = self.palette;
        egui::Panel::left("drawer")
            .exact_size(240.0)
            .frame(sheet_frame(&palette))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Sessions").size(14.0));
                drawer_item(ui, &palette, Some(Icon::Agent), "Current session", true);
                drawer_item(ui, &palette, Some(Icon::Read), "Release notes", false);
                drawer_item(ui, &palette, Some(Icon::Command), "Build fixes", false);
            });
        egui::Panel::right("sheet")
            .exact_size(260.0)
            .frame(sheet_frame(&palette))
            .show(ui, |ui| {
                sheet_header(ui, &palette, "Tasks");
                card_frame(CardKind::Outlined, &palette).show(ui, |ui| {
                    ui.label("Outlined card");
                });
                ui.add_space(8.0);
                menu_frame(&palette).show(ui, |ui| {
                    menu_item(ui, &palette, Some(Icon::Copy), "Copy", Some("Ctrl+C"));
                    menu_item(ui, &palette, Some(Icon::Search), "Find", Some("Ctrl+F"));
                    menu_item(ui, &palette, None, "Select all", None);
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(rgb(palette.m3.surface_container_low)))
            .show(ui, |ui| {
                primary_tabs(
                    ui,
                    &palette,
                    &["Home", "Insert", "Agent", "View"],
                    &mut self.tab,
                );
                egui::Frame::new().inner_margin(16).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        button(ui, &palette, ButtonKind::Filled, Some(Icon::Check), "Send");
                        button(ui, &palette, ButtonKind::Tonal, None, "Queue");
                        button(ui, &palette, ButtonKind::Outlined, None, "Stop");
                        button(ui, &palette, ButtonKind::Text, None, "Cancel");
                        ui.add_enabled_ui(false, |ui| {
                            button(ui, &palette, ButtonKind::Filled, None, "Disabled");
                        });
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        icon_button(
                            ui,
                            &palette,
                            IconButtonKind::Standard,
                            Icon::Copy,
                            "Copy",
                            None,
                        );
                        icon_button(
                            ui,
                            &palette,
                            IconButtonKind::Filled,
                            Icon::Edit,
                            "Edit",
                            None,
                        );
                        if icon_button(
                            ui,
                            &palette,
                            IconButtonKind::Filled,
                            Icon::Info,
                            "Details",
                            Some(self.toggle),
                        )
                        .clicked()
                        {
                            self.toggle = !self.toggle;
                        }
                        ui.add_space(12.0);
                        segmented_buttons(ui, &palette, &["Unified", "Split"], &mut self.segment);
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        chip(
                            ui,
                            &palette,
                            ChipKind::Assist,
                            Some(Icon::Search),
                            "Search code",
                        );
                        if chip(
                            ui,
                            &palette,
                            ChipKind::Filter {
                                selected: self.filter,
                            },
                            None,
                            "Errors only",
                        )
                        .clicked()
                        {
                            self.filter = !self.filter;
                        }
                        chip(
                            ui,
                            &palette,
                            ChipKind::Filter { selected: false },
                            None,
                            "Warnings",
                        );
                    });
                    ui.add_space(16.0);
                    ui.set_max_width(480.0);
                    outlined_text_field(
                        ui,
                        &palette,
                        "Session name",
                        &mut self.name,
                        Some("Shown in the sessions drawer"),
                        false,
                    );
                    ui.add_space(12.0);
                    outlined_text_field(ui, &palette, "Model", &mut self.empty, None, false);
                    ui.add_space(12.0);
                    slider(ui, &palette, &mut self.zoom, 50.0..=200.0, "Zoom");
                    linear_progress(ui, &palette, Some(0.6));
                    ui.add_space(16.0);
                    let card = card_frame(CardKind::Elevated, &palette).show(ui, |ui| {
                        ui.label("Elevated card with a tooltip");
                    });
                    plain_tooltip(card.response, &palette, "Plain tooltip");
                    ui.add_space(16.0);
                    dialog_frame(&palette).show(ui, |ui| {
                        dialog_headline(ui, &palette, "Discard changes?");
                        dialog_body(ui, &palette, "The edits to this session will be lost.");
                        dialog_actions(ui, |ui| {
                            button(ui, &palette, ButtonKind::Text, None, "Discard");
                            button(ui, &palette, ButtonKind::Text, None, "Keep editing");
                        });
                    });
                });
            });
        snackbar(
            ui.ctx(),
            &palette,
            egui::Id::unique("gallery_snackbar"),
            "Session saved",
            Some("Undo"),
        );
    }
}

fn main() -> eframe::Result {
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "document-light".to_string());
    let catalog = ThemeCatalog::builtin();
    let Some(tokens) = catalog.get(&name) else {
        eprintln!("unknown theme `{name}`");
        std::process::exit(2);
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Material gallery",
        options,
        Box::new(move |cc| {
            codemason_gui::fonts::install(&cc.egui_ctx, craft_ui::fonts::SystemFonts::Off);
            cc.egui_ctx.set_theme(if tokens.dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            let visuals = visuals_for(&tokens);
            cc.egui_ctx
                .all_styles_mut(|style| style.visuals = visuals.clone());
            Ok(Box::new(Gallery {
                palette: Palette::from_tokens(&tokens),
                tab: 0,
                segment: 0,
                filter: true,
                toggle: true,
                zoom: 100.0,
                name: "Release prep".to_string(),
                empty: String::new(),
            }))
        }),
    )
}
