//! Tests for `codemason_gui::fonts`: the font stack registers the fallback
//! and bold families, the bundled faces cover the glyphs the core draws
//! (status icons, block elements, box drawing), `drawable` keeps
//! those glyphs once the stack is installed, and bold spans use the bold
//! face. Every check runs with system fonts off, so nothing depends on the
//! fonts installed on the host.

use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle};
use codemason_app_core::theme_mode::ThemeMode;
use codemason_gui::fonts::{
    BOLD_FACE, BOLD_FAMILY, BOLD_MONO_FAMILY, SYMBOL_FACE, UI_BOLD_FACE, UI_FACE, font_stack,
    install,
};
use codemason_gui::palette::palette_for;
use codemason_gui::styled_text::{Bold, FONT_SIZE, drawable, styled_to_layout_job};
use craft_ui::fonts::SystemFonts;
use egui::{FontFamily, FontId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Glyphs the core emits that the bundled faces must draw: the running
/// diamond and success dot, the full and half blocks and a quadrant, and
/// box-drawing lines.
const COVERED: [char; 8] = [
    '\u{25C6}', '\u{25CF}', '\u{2588}', '\u{2580}', '\u{2584}', '\u{259F}', '\u{2500}', '\u{256C}',
];

/// The status icon no bundled proportional face has (only system fonts draw
/// it), with the ASCII stand-in `drawable` uses when no installed font has
/// it either: the info sign.
const SYSTEM_ONLY: [(char, char); 1] = [('\u{2139}', 'i')];

/// Status icons the bundled Inter face draws in proportional text: the error
/// cross and the warning sign.
const UI_FACE_ICONS: [char; 2] = ['\u{2717}', '\u{26A0}'];

/// The families the window draws text in.
fn families() -> [FontFamily; 4] {
    [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name(BOLD_FAMILY.into()),
        FontFamily::Name(BOLD_MONO_FAMILY.into()),
    ]
}

/// A context after one pass, with the window's fonts (system fonts off)
/// when `with_stack`, else egui's defaults.
fn context(with_stack: bool) -> egui::Context {
    let ctx = egui::Context::default();
    if with_stack {
        install(&ctx, SystemFonts::Off);
    }
    // Fonts become active at the start of the next pass.
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    ctx
}

#[test]
fn test_fonts_stack_registers_fallback_families() {
    let stack = font_stack(SystemFonts::Off);
    // The proportional family falls back to egui's Hack face, which has
    // the box-drawing, block and geometric-shape ranges.
    let proportional = stack.chain(&FontFamily::Proportional);
    assert!(
        proportional.iter().any(|name| name == SYMBOL_FACE),
        "{proportional:?}"
    );
    // Proportional text is led by the bundled UI face.
    assert_eq!(proportional.first().map(String::as_str), Some(UI_FACE));
    // Bold prose starts with the UI face's own bold weight, and falls back to
    // the bold code face for the block glyphs it lacks; bold code starts with
    // the bold code face.
    let bold = stack.chain(&FontFamily::Name(BOLD_FAMILY.into()));
    assert_eq!(bold.first().map(String::as_str), Some(UI_BOLD_FACE));
    assert!(bold.iter().any(|name| name == BOLD_FACE), "{bold:?}");
    let bold_mono = stack.chain(&FontFamily::Name(BOLD_MONO_FAMILY.into()));
    assert_eq!(bold_mono.first().map(String::as_str), Some(BOLD_FACE));
}

#[test]
fn test_fonts_bundled_faces_cover_status_and_block_glyphs() {
    let stack = font_stack(SystemFonts::Off);
    let mut fonts = stack.fonts(false);
    for family in families() {
        let font = FontId::new(FONT_SIZE, family.clone());
        for c in COVERED {
            assert!(
                fonts.has_glyph(&font, c),
                "{family:?} lacks U+{:04X}",
                u32::from(c)
            );
        }
        assert!(fonts.has_glyphs(&font, "Bold text 123"), "{family:?}");
    }
    // This is in no bundled face: only system fonts have it.
    let proportional = FontId::proportional(FONT_SIZE);
    for (c, _) in SYSTEM_ONLY {
        assert!(!fonts.has_glyph(&proportional, c), "U+{:04X}", u32::from(c));
    }
    // The bundled UI face draws these itself.
    for c in UI_FACE_ICONS {
        assert!(fonts.has_glyph(&proportional, c), "U+{:04X}", u32::from(c));
    }
}

#[test]
fn test_fonts_drawable_keeps_glyphs_the_stack_covers() {
    let ctx = context(true);
    let font = FontId::proportional(FONT_SIZE);
    let covered = "\u{2588}\u{2580}\u{2584} \u{25C6} running \u{25CF} done \u{2500}\u{2500}";
    assert_eq!(drawable(&ctx, &font, covered), covered);
    // Only the glyphs no configured face has are still replaced.
    for (c, stand_in) in SYSTEM_ONLY {
        assert_eq!(
            drawable(&ctx, &font, &format!("{c} s")),
            format!("{stand_in} s")
        );
    }
    assert_eq!(drawable(&ctx, &font, "\u{E5B6}"), "?");
    assert_eq!(Bold::of(&ctx), Bold::Face);
    assert_eq!(Bold::of(&context(false)), Bold::Regular);
}

#[test]
fn test_fonts_bold_spans_use_the_bold_face() {
    let lines = vec![StyledLine {
        spans: vec![
            StyledSpan::styled("Heading", TextStyle::new().bold()),
            StyledSpan::styled(
                " code",
                TextStyle {
                    monospace: true,
                    ..TextStyle::new().bold()
                },
            ),
            StyledSpan::raw(" body"),
        ],
    }];
    let palette = palette_for(ThemeMode::Default);
    let job = styled_to_layout_job(&lines, &palette, 400.0, Bold::Face);
    let families: Vec<FontFamily> = job
        .sections
        .iter()
        .map(|s| s.format.font_id.family.clone())
        .collect();
    assert_eq!(
        families,
        [
            FontFamily::Name(BOLD_FAMILY.into()),
            FontFamily::Name(BOLD_MONO_FAMILY.into()),
            FontFamily::Proportional,
        ]
    );
    // Without the bold faces installed, bold draws at regular weight.
    let job = styled_to_layout_job(&lines, &palette, 400.0, Bold::Regular);
    assert_eq!(
        job.sections[0].format.font_id.family,
        FontFamily::Proportional
    );
    assert_eq!(job.sections[1].format.font_id.family, FontFamily::Monospace);
}

#[test]
fn test_fonts_window_draws_block_and_status_glyphs_without_tofu() {
    // No `allow_missing_glyphs`: kittest fails the frame on any tofu.
    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 120.0))
        .build_ui_state(
            |ui, ready: &mut bool| {
                if *ready {
                    ui.label("\u{2588}\u{2580}\u{2584}\u{259F} \u{25C6} running");
                    ui.label(
                        egui::RichText::new("bold \u{2588}")
                            .family(FontFamily::Name(BOLD_FAMILY.into())),
                    );
                    ui.monospace("\u{250C}\u{2500}\u{2510} \u{25CF}");
                }
            },
            false,
        );
    install(&harness.ctx, SystemFonts::Off);
    harness.run();
    *harness.state_mut() = true;
    harness.run();
    assert!(
        harness
            .query_by_label("\u{2588}\u{2580}\u{2584}\u{259F} \u{25C6} running")
            .is_some()
    );
}
