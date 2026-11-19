//! Shared setup for the `test_material_*` files: the `document-light`
//! palette, and a harness builder with the window's font stack installed
//! (the components draw labels in its bold family).

use codemason_app_core::theme_tokens::ThemeCatalog;
use codemason_gui::palette::Palette;
use egui_kittest::HarnessBuilder;

/// The `document-light` palette.
pub fn palette() -> Palette {
    Palette::from_tokens(
        &ThemeCatalog::builtin()
            .get("document-light")
            .expect("built-in"),
    )
}

/// A harness builder whose context has the window's fonts (system fonts
/// off, so every machine draws the same).
#[allow(dead_code)] // Not every file that includes this module builds a harness.
pub fn builder<State>() -> HarnessBuilder<State> {
    Harness::<State>::builder().with_context_setup(|ctx| {
        codemason_gui::fonts::install(ctx, craft_ui::fonts::SystemFonts::Off);
    })
}

use egui_kittest::Harness;
