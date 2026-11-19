//! Mermaid diagrams in the transcript.
//!
//! A ```` ```mermaid ```` block in a reply is drawn as the diagram: the
//! source is rendered to SVG by `mermaid-rs-renderer` (pure Rust, no
//! browser) in the theme's Material colours, and egui's SVG loader
//! rasterises it at the screen's scale. The SVG is cached by its source and
//! colours, so a diagram is rendered once, not every frame.
//!
//! The diagram is shown at its own size, scaled down to the transcript's
//! width; a click opens it in the full-size image view. A Show source
//! button swaps it for the code block (and Show diagram swaps back). While
//! the reply is still streaming, and when the source does not parse, the
//! code block is shown instead; a parse error is named above it.
//!
//! Dependencies: `egui`, `mermaid-rs-renderer`, and this crate's code block,
//! image view, Material buttons and palette.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use egui::Id;
use mermaid_rs_renderer::{RenderOptions, Theme, render_with_options};

use crate::dialogs::image_view::ImageRef;
use crate::material::button::{ButtonKind, button};
use crate::metrics::{radius, text_size};
use crate::palette::{Palette, rgb};
use crate::panels::code_block::{CodeAction, code_block_ui};

/// The button that shows a diagram's source.
pub const SHOW_SOURCE: &str = "Show source";

/// The button that shows the diagram again.
pub const SHOW_DIAGRAM: &str = "Show diagram";

/// The tallest a diagram is drawn in the transcript, in points; a click
/// opens it at full size.
pub const MAX_INLINE_HEIGHT: f32 = 1200.0;

/// Accessible name and full-size view headline of a diagram.
pub const DIAGRAM_NAME: &str = "Mermaid diagram";

/// What a diagram block asked for this frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MermaidOutcome {
    /// Show the diagram at full size.
    pub enlarge: Option<ImageRef>,
    /// What the code block (shown instead of the diagram) asked for.
    pub code: Option<CodeAction>,
}

/// Whether a code fence's language is Mermaid.
#[must_use]
pub fn is_mermaid(lang: Option<&str>) -> bool {
    lang.is_some_and(|lang| lang.trim().eq_ignore_ascii_case("mermaid"))
}

/// `rgb` as `#rrggbb`, for the renderer's theme.
fn hex(colour: codemason_app_core::theme_tokens::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.0, colour.1, colour.2)
}

/// The renderer's theme in `palette`'s Material colours: nodes on
/// `surface_container_high` with an `outline` border, lines and labels in
/// the `on_surface` family, on the transcript's `surface`.
#[must_use]
pub fn mermaid_theme(palette: &Palette) -> Theme {
    let m3 = &palette.m3;
    let mut theme = if palette.dark {
        Theme::dark()
    } else {
        Theme::modern()
    };
    theme.primary_color = hex(m3.surface_container_high);
    theme.primary_text_color = hex(m3.on_surface);
    theme.primary_border_color = hex(m3.outline);
    theme.line_color = hex(m3.on_surface_variant);
    theme.secondary_color = hex(m3.secondary_container);
    theme.tertiary_color = hex(m3.tertiary_container);
    theme.edge_label_background = hex(m3.surface);
    theme.cluster_background = hex(m3.surface_container_low);
    theme.cluster_border = hex(m3.outline_variant);
    theme.background = hex(m3.surface);
    theme.sequence_actor_fill = hex(m3.surface_container_high);
    theme.sequence_actor_border = hex(m3.outline);
    theme.sequence_actor_line = hex(m3.outline_variant);
    theme.sequence_note_fill = hex(m3.tertiary_container);
    theme.sequence_note_border = hex(m3.outline_variant);
    theme.sequence_activation_fill = hex(m3.secondary_container);
    theme.sequence_activation_border = hex(m3.outline);
    theme.text_color = hex(m3.on_surface);
    theme
}

/// A rendered diagram: its SVG, or why it could not be rendered.
type Rendered = Arc<Result<Arc<[u8]>, String>>;

/// The cache key of `source` drawn in `theme`: the source and every
/// theme input (each colour, the font and the light or dark base), so any
/// theme change draws the diagram again.
#[must_use]
pub fn key(source: &str, theme: &Theme) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    format!("{theme:?}").hash(&mut hasher);
    hasher.finish()
}

/// `source` rendered in `theme`, from the cache or rendered now.
fn rendered(ctx: &egui::Context, source: &str, theme: Theme, key: u64) -> Rendered {
    let id = Id::unique(("codemason_mermaid_svg", key));
    if let Some(done) = ctx.data(|d| d.get_temp::<Rendered>(id)) {
        return done;
    }
    let options = RenderOptions {
        theme,
        ..RenderOptions::default()
    };
    let done: Rendered = Arc::new(
        render_with_options(source, options)
            .map(|svg| Arc::from(svg.into_bytes()))
            .map_err(|err| err.to_string()),
    );
    ctx.data_mut(|d| d.insert_temp(id, Arc::clone(&done)));
    done
}

/// Draw a Mermaid block: the diagram, or its source.
///
/// # Arguments
///
/// * `ui` - where to draw; the diagram is at most the available width.
/// * `id` - a stable id for the block (its source toggle is kept under it).
/// * `source` - the diagram's Mermaid source.
/// * `palette` - the colours.
/// * `streaming` - whether the reply is still being written; its source is
///   shown until it is done.
///
/// # Returns
///
/// What the block asked for: the diagram at full size, or the code
/// block's action.
pub fn mermaid_ui(
    ui: &mut egui::Ui,
    id: Id,
    source: &str,
    palette: &Palette,
    streaming: bool,
) -> MermaidOutcome {
    let mut outcome = MermaidOutcome::default();
    let source_id = id.with("mermaid_source");
    let show_source = ui.data(|d| d.get_temp::<bool>(source_id)).unwrap_or(false);
    let code =
        |ui: &mut egui::Ui| code_block_ui(ui, id.with("code"), Some("mermaid"), source, palette);
    if streaming {
        outcome.code = code(ui);
        return outcome;
    }
    let theme = mermaid_theme(palette);
    let key = key(source, &theme);
    let svg = match rendered(ui.ctx(), source, theme, key).as_ref() {
        Ok(svg) => Arc::clone(svg),
        Err(err) => {
            ui.label(
                egui::RichText::new(format!("{DIAGRAM_NAME} could not be drawn: {err}"))
                    .size(text_size::SMALL)
                    .color(rgb(palette.m3.error)),
            );
            outcome.code = code(ui);
            return outcome;
        }
    };
    let toggle = if show_source {
        SHOW_DIAGRAM
    } else {
        SHOW_SOURCE
    };
    if show_source {
        outcome.code = code(ui);
    } else {
        let uri = format!("bytes://mermaid/{key:016x}.svg");
        let image = egui::Image::from_bytes(uri.clone(), egui::load::Bytes::Shared(svg))
            .fit_to_original_size(1.0)
            .max_size(egui::vec2(ui.available_width(), MAX_INLINE_HEIGHT))
            .corner_radius(radius::SM)
            .alt_text(DIAGRAM_NAME)
            .sense(egui::Sense::CLICK);
        if ui.add(image).on_hover_text("Click to enlarge").clicked() {
            outcome.enlarge = Some(ImageRef {
                uri,
                name: DIAGRAM_NAME.to_string(),
            });
        }
    }
    if button(ui, palette, ButtonKind::Text, None, toggle).clicked() {
        ui.data_mut(|d| d.insert_temp(source_id, !show_source));
    }
    outcome
}
