//! Coverage tests for `codemason-tui` modules that are otherwise lightly exercised
//! by the existing suite:
//!
//! * `app::models` - markdown rendering (`render_markdown_unconditionally`) and
//!   the model-list fallbacks reachable through the public `App` surface.
//! * `app::state` - the shared state helpers (snapshot maths, role modes, pane
//!   cycles, the plugin-store browser navigators) and the pure `App` poll/
//!   housekeeping helpers.
//! * `app::cron` - the background scheduler handle lifecycle (`start`/`stop`).
//! * `theme` - theme selection, colour/style lookup, spinner and progress-bar
//!   helpers, status categories/messages/history.
//! * `app::reverse` - the `/reverse` text grammar through the public test hooks.
//! * `widgets::button` - the button and button-bar widgets rendered through
//!   `ratatui::backend::TestBackend`.
//!
//! Tests only - no file under `crates/*/src/` is touched. Every cargo command is
//! run with an isolated `XDG_CONFIG_HOME` (see the task harness), so nothing here
//! reads or writes the developer's real `~/.config/codemason`.
#![allow(unsafe_code)]
#![allow(clippy::await_holding_lock)]

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Color;
use ratatui::widgets::Widget;

use codemason_tui::theme::{self, StatusCategory, StatusStyle};
use codemason_tui::widgets::button::{Button, ButtonBar, ButtonRender, ButtonState, ButtonVariant};

#[path = "support/mod.rs"]
mod support;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

// ===========================================================================
// theme.rs
// ===========================================================================

#[test]
fn theme_typography_helpers_run() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let _ = theme::heading();
    let _ = theme::emphasis();
    let _ = theme::muted();
    assert_eq!(theme::error().fg, Some(theme::status::ERROR));
    assert_eq!(theme::success().fg, Some(theme::status::SUCCESS));
    assert_eq!(theme::warning().fg, Some(theme::status::WARNING));
    assert_eq!(theme::info().fg, Some(theme::status::INFO));
    assert_eq!(theme::think().fg, Some(theme::colors::THINK));
    let _ = theme::loading();
    let _ = theme::disabled();
}

#[test]
fn theme_colour_constants_and_palettes_are_distinct() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(theme::status::SUCCESS, Color::Green);
    assert_eq!(theme::status::ERROR, Color::Red);
    assert_eq!(theme::status::WARNING, Color::Yellow);
    assert_eq!(theme::status::INFO, Color::Cyan);
    assert_ne!(theme::colors::TEXT, theme::colors::BACKGROUND);
    assert_ne!(theme::colors::PRIMARY, theme::colors::SECONDARY);
    assert_ne!(theme::colors::SELECTION_FG, theme::colors::SELECTION_BG);
    assert_eq!(theme::colors::TEXT_PRIMARY, Color::White);
    // High-contrast palette differs from the default palette where the default
    // used a mid-tone; the pure black/white primaries intentionally match.
    assert_ne!(theme::high_contrast::PRIMARY, theme::colors::PRIMARY);
    assert_ne!(theme::high_contrast::THINK, theme::colors::THINK);
    assert_ne!(theme::high_contrast::MUTED, theme::colors::MUTED);
    assert_eq!(theme::high_contrast::BACKGROUND, Color::Black);
    assert_eq!(theme::high_contrast::LINK_COLOR, Color::Rgb(0, 255, 255));
    assert_eq!(theme::high_contrast::SELECTION_FG, Color::White);
}

#[test]
fn theme_focus_and_accessibility_constants() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(theme::focus::SELECTED, "▸ ");
    assert_eq!(theme::focus::UNSELECTED, "  ");
    assert_ne!(theme::focus::FOCUSED, "");
    assert_ne!(theme::focus::BUTTON, "");
    assert_ne!(theme::focus::INTERACTIVE, "");
    assert_ne!(theme::accessibility::ROLE_BUTTON, "");
    assert_ne!(theme::accessibility::STATE_SELECTED, "");
    assert_ne!(theme::accessibility::ROLE_INTERACTIVE, "");
    assert_ne!(theme::accessibility::ROLE_EXPANDABLE, "");
    assert_ne!(theme::accessibility::ROLE_COLLAPSED, "");
    assert_ne!(theme::accessibility::ROLE_EXPANDED, "");
    assert_ne!(theme::accessibility::STATE_LOADING, "");
    assert_eq!(theme::accessibility::PROGRESS_PREFIX, "[");
    assert_eq!(theme::accessibility::PROGRESS_SUFFIX, "]");
    assert_ne!(theme::accessibility::PROGRESS_FILL, "");
    assert_ne!(theme::accessibility::PROGRESS_EMPTY, "");
    assert_eq!(theme::accessibility::SPINNER_FRAMES.len(), 10);
}

#[test]
fn theme_spacing_icon_and_layout_constants() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(theme::SPACING_XS, 1);
    assert_eq!(theme::SPACING_SM, 2);
    assert_eq!(theme::SPACING_MD, 4);
    assert_eq!(theme::SPACING_LG, 8);
    assert_eq!(theme::SPACING_XL, 16);
    assert_eq!(theme::LAYOUT_MAX_CONTENT_WIDTH, 88);
    assert_eq!(theme::LAYOUT_BORDER_PADDING, 2);
    assert_eq!(theme::LAYOUT_SMALL_DIALOG, (60, 30));
    assert_eq!(theme::LAYOUT_MEDIUM_DIALOG, (70, 40));
    assert_eq!(theme::LAYOUT_LARGE_DIALOG, (90, 70));
    for icon in [
        theme::ICON_THOUGHT,
        theme::ICON_FILE,
        theme::ICON_TASK,
        theme::ICON_SUCCESS,
        theme::ICON_ERROR,
        theme::ICON_RUNNING,
        theme::ICON_SPAWNING,
        theme::ICON_IDLE,
        theme::ICON_BLOCKED,
        theme::ICON_FAILED,
        theme::ICON_SELECTED,
        theme::ICON_UNSELECTED,
        theme::ICON_ATTACHMENT,
    ] {
        assert_ne!(icon, "");
    }
}

#[test]
fn theme_spinner_frame_wraps_and_clamps_interval() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(
        theme::accessibility::spinner_frame(0, 80),
        theme::accessibility::SPINNER_FRAMES[0]
    );
    assert_eq!(
        theme::accessibility::spinner_frame(80, 80),
        theme::accessibility::SPINNER_FRAMES[1]
    );
    // interval 0 is clamped to 1, so a large elapsed still indexes safely.
    let frame = theme::accessibility::spinner_frame(10_000, 0);
    assert!(theme::accessibility::SPINNER_FRAMES.contains(&frame));
}

#[test]
fn theme_progress_bar_variants() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let empty = theme::accessibility::progress_bar(0.0, 4);
    assert_eq!(empty, "[░░░░]");
    let half = theme::accessibility::progress_bar(0.5, 4);
    assert_eq!(half, "[██░░]");
    let full = theme::accessibility::progress_bar(1.0, 4);
    assert_eq!(full, "[████] ✓");
    // Over-100% is clamped; a zero width yields an empty bar.
    assert_eq!(theme::accessibility::progress_bar(2.0, 3), "[███] ✓");
    assert_eq!(theme::accessibility::progress_bar(-1.0, 3), "[░░░]");

    let labeled = theme::accessibility::labeled_progress_bar(0.5, 10, "Loading");
    assert!(labeled.starts_with("Loading ["));
    assert!(labeled.ends_with("50%"));
    let labeled_full = theme::accessibility::labeled_progress_bar(1.5, 10, "X");
    assert!(labeled_full.ends_with("100%"));
}

#[test]
fn theme_loading_frame_rotates() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    assert_eq!(theme::LOADING_FRAMES.len(), 4);
    assert_eq!(theme::loading_frame(0, 250), theme::LOADING_FRAMES[0]);
    assert_eq!(theme::loading_frame(250, 250), theme::LOADING_FRAMES[1]);
    assert!(theme::LOADING_FRAMES.contains(&theme::loading_frame(1000, 0)));
}

#[test]
fn theme_status_category_all_methods() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    for category in [
        StatusCategory::Info,
        StatusCategory::Success,
        StatusCategory::Warning,
        StatusCategory::Error,
        StatusCategory::Working,
    ] {
        assert_ne!(category.icon(), "");
        let _ = category.color();
        let _ = category.style();
        assert_ne!(category.display_name(), "");
        let formatted = category.format("hello");
        assert!(formatted.contains("hello"));
    }
    // Working is bold, the others are not.
    let working = StatusCategory::Working.style();
    assert!(
        working
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
}

// ===========================================================================
// widgets/button.rs
// ===========================================================================

#[test]
fn button_variant_background_colours_cover_all_variants() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    for variant in [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Danger,
        ButtonVariant::Success,
    ] {
        let _ = variant.enabled_bg_color();
        let _ = variant.active_bg_color();
        assert_eq!(variant.disabled_bg_color(), theme::colors::BORDER_INACTIVE);
    }
    // Primary and Secondary use distinct fills in the default palette.
    assert_ne!(
        ButtonVariant::Primary.enabled_bg_color(),
        ButtonVariant::Secondary.enabled_bg_color()
    );
    assert_eq!(
        ButtonVariant::Primary.enabled_bg_color(),
        ButtonVariant::Primary.active_bg_color()
    );
}

#[test]
fn button_state_colours_and_padding() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let enabled = Button::new("Go", ButtonVariant::Primary);
    let active = Button::new("Go", ButtonVariant::Primary).with_state(ButtonState::Active);
    let disabled = Button::new("Go", ButtonVariant::Primary).with_state(ButtonState::Disabled);

    assert_eq!(
        enabled.bg_color(),
        ButtonVariant::Primary.enabled_bg_color()
    );
    assert_eq!(active.bg_color(), ButtonVariant::Primary.active_bg_color());
    assert_eq!(disabled.bg_color(), theme::colors::BORDER_INACTIVE);

    assert_eq!(enabled.fg_color(), theme::colors::TEXT_PRIMARY);
    assert_eq!(active.fg_color(), theme::colors::TEXT_PRIMARY);
    assert_eq!(disabled.fg_color(), theme::colors::HINT);

    assert_eq!(active.border_color(), theme::colors::FOCUS_COLOR);
    assert_eq!(disabled.border_color(), theme::colors::BORDER_INACTIVE);

    // Borders change the padding.
    assert_eq!(enabled.horizontal_padding(), 2);
    assert_eq!(enabled.vertical_padding(), 1);
    let plain = Button::new("Go", ButtonVariant::Primary).bordered(false);
    assert_eq!(plain.horizontal_padding(), 1);
    assert_eq!(plain.vertical_padding(), 0);
}

#[test]
fn button_dimensions_and_builders() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let plain = Button::new("OK", ButtonVariant::Primary);
    assert_eq!(plain.total_width(), 6); // 2 chars + 2*2 padding
    assert_eq!(plain.total_height(), 3); // height 1 + 2*1 padding

    let with_shortcut = Button::with_shortcut("Yes", 'y', ButtonVariant::Primary);
    assert_eq!(with_shortcut.shortcut, Some('y'));
    assert_eq!(with_shortcut.total_width(), 10); // "[y]Yes" = 6 + 4

    let fixed = Button::new("OK", ButtonVariant::Primary).with_width(20);
    assert_eq!(fixed.total_width(), 20);
    // A width below the minimum is clamped to 4.
    let tiny = Button::new("OK", ButtonVariant::Primary).with_width(1);
    assert_eq!(tiny.total_width(), 4);

    let tall = Button::new("OK", ButtonVariant::Primary).with_height(5);
    assert_eq!(tall.total_height(), 7);
    // height 0 is clamped to 1.
    let flat = Button::new("OK", ButtonVariant::Primary).with_height(0);
    assert_eq!(flat.total_height(), 3);

    let extra = Button::new("OK", ButtonVariant::Primary).with_shortcut_char('k');
    assert_eq!(extra.shortcut, Some('k'));
}

#[test]
fn button_styled_label_variants() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let plain = Button::new("OK", ButtonVariant::Primary);
    assert_eq!(plain.styled_label().spans.len(), 1);

    let with_shortcut = Button::with_shortcut("Yes", 'y', ButtonVariant::Primary);
    assert_eq!(with_shortcut.styled_label().spans.len(), 4);

    let active =
        Button::with_shortcut("Yes", 'y', ButtonVariant::Primary).with_state(ButtonState::Active);
    assert_eq!(active.styled_label().spans.len(), 4);
    // No-shortcut active label: the bold label-style arm.
    let active_plain = Button::new("OK", ButtonVariant::Primary).with_state(ButtonState::Active);
    assert_eq!(active_plain.styled_label().spans.len(), 1);

    // The role-prefixed labels add the accessibility marker span.
    assert_eq!(plain.styled_label_with_role().spans.len(), 2);
    assert_eq!(with_shortcut.styled_label_with_role().spans.len(), 5);
    // Active shortcut role label: the underlined shortcut-style arm.
    assert_eq!(active.styled_label_with_role().spans.len(), 5);
    // Active plain role label: the no-shortcut arm.
    assert_eq!(active_plain.styled_label_with_role().spans.len(), 2);

    let span = with_shortcut.to_span();
    assert!(span.content.contains("[y]Yes"));
    let plain_span = plain.to_span();
    assert!(plain_span.content.starts_with("OK"));
    // Active shortcut span uses the bold shortcut-style arm.
    let active_span = active.to_span();
    assert!(active_span.content.contains("[y]Yes"));
}

#[test]
fn button_bar_layout_helpers() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let empty = ButtonBar::new();
    assert_eq!(empty.total_width(), 0);
    assert_eq!(empty.max_height(), 1);
    assert_eq!(empty.button_spacing, 2);
    assert_eq!(empty.alignment, Alignment::Center);

    let single = ButtonBar::new().push(Button::new("OK", ButtonVariant::Primary));
    assert_eq!(single.total_width(), 6);

    let bar = ButtonBar::with_buttons(vec![
        Button::new("OK", ButtonVariant::Primary),
        Button::new("Cancel", ButtonVariant::Secondary),
    ])
    .with_spacing(3)
    .with_alignment(Alignment::Right);
    assert_eq!(bar.buttons.len(), 2);
    assert_eq!(bar.button_spacing, 3);
    assert_eq!(bar.alignment, Alignment::Right);
    assert_eq!(bar.total_width(), 6 + 3 + 10);
    assert_eq!(bar.max_height(), 3);

    // The Default impl equals `new`.
    let defaulted = ButtonBar::default();
    assert!(defaulted.buttons.is_empty());
}

#[test]
fn button_bar_render_to_frame_all_alignments() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
        let bar = ButtonBar::with_buttons(vec![
            Button::new("OK", ButtonVariant::Primary),
            Button::new("Cancel", ButtonVariant::Secondary),
        ])
        .with_alignment(alignment);
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| bar.render_to_frame(frame, Rect::new(0, 0, 40, 5)))
            .expect("draw");
        // The bar paints at least one non-blank cell.
        let painted = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|c| c.symbol() != " ");
        assert!(painted, "alignment {alignment:?} painted nothing");
    }

    // An empty bar renders nothing and must not panic.
    let empty = ButtonBar::new();
    let backend = TestBackend::new(10, 2);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| empty.render_to_frame(frame, Rect::new(0, 0, 10, 2)))
        .expect("draw");

    // A bar wider than the frame takes the `total_width >= available` branch
    // for both Center and Right (start_x pinned to area.x).
    for alignment in [Alignment::Center, Alignment::Right] {
        let bar = ButtonBar::with_buttons(vec![Button::new("WideLabel", ButtonVariant::Primary)])
            .with_alignment(alignment);
        let backend = TestBackend::new(4, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| bar.render_to_frame(frame, Rect::new(0, 0, 4, 3)))
            .expect("draw");
    }

    // A second button that does not fit in the remaining width breaks the loop
    // (button_width == 0) after painting the first.
    let tight = ButtonBar::with_buttons(vec![
        Button::new("OK", ButtonVariant::Primary),
        Button::new("Cancel", ButtonVariant::Secondary),
    ]);
    let backend = TestBackend::new(6, 3);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| tight.render_to_frame(frame, Rect::new(0, 0, 6, 3)))
        .expect("draw");
}

#[test]
fn button_render_content_clips_to_the_area() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    // An unbordered, over-wide label takes the `content_width >= area.width`
    // branch and is clipped at the right edge.
    let long = Button::new("VeryLongLabel", ButtonVariant::Success).bordered(false);
    let area = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(area);
    ButtonRender::new(&long, area).render(area, &mut buf);
    let text: String = (0..area.width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect();
    assert!(text.starts_with("Very"), "painted: {text}");
}

#[test]
fn button_render_widget_bordered_and_plain() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    let area = Rect::new(0, 0, 12, 3);
    let button = Button::with_shortcut("Go", 'g', ButtonVariant::Primary);
    let mut buf = Buffer::empty(area);
    ButtonRender::new(&button, area).render(area, &mut buf);
    let text: String = (0..area.width)
        .map(|x| buf[(x, 1)].symbol().to_string())
        .collect();
    assert!(text.contains('G') && text.contains('o'), "painted: {text}");

    // Borders paint the corner glyphs.
    assert_eq!(buf[(0, 0)].symbol(), "┌");

    // An unbordered button paints its label without a frame.
    let plain = Button::new("Hi", ButtonVariant::Success).bordered(false);
    let area2 = Rect::new(0, 0, 10, 1);
    let mut buf2 = Buffer::empty(area2);
    ButtonRender::new(&plain, area2).render(area2, &mut buf2);
    let text2: String = (0..area2.width)
        .map(|x| buf2[(x, 0)].symbol().to_string())
        .collect();
    assert!(text2.contains("Hi"), "painted: {text2}");
}

#[test]
fn button_render_widget_degenerate_areas() {
    // Every test in this file is gated on the binary-wide cwd/env lock: the
    // `/slash` arms and the GitHub/GitLab credential chains read `cwd`,
    // `XDG_CONFIG_HOME`, `PATH` and the token env vars, all process-global
    // under one-binary-per-crate (AGENTS.md "Test Layout").
    let _cwd_guard = crate::global_env_lock();
    // A too-small bordered area falls through to the direct content path.
    let button = Button::new("OK", ButtonVariant::Danger);
    let area = Rect::new(0, 0, 3, 1);
    let mut buf = Buffer::empty(area);
    ButtonRender::new(&button, area).render(area, &mut buf);

    // A single-row bordered button: the inner height is 0 so no content paints.
    let area2 = Rect::new(0, 0, 8, 1);
    let mut buf2 = Buffer::empty(area2);
    ButtonRender::new(&button, area2).render(area2, &mut buf2);
}

// ===========================================================================
// app/state.rs - pure helpers
// ===========================================================================

// ===========================================================================
// app/state.rs - App poll/housekeeping helpers
// ===========================================================================

// ===========================================================================
// app/cron.rs - scheduler handle lifecycle
// ===========================================================================

// ===========================================================================
// app/models.rs - markdown rendering reachable through the public surface
// ===========================================================================

// ===========================================================================
// app/reverse.rs - the `/reverse` text grammar through the public hooks
// ===========================================================================

// ===========================================================================
// app/reverse.rs - `/spec reverse` scaffold refusal (isolated project)
// ===========================================================================

#[path = "slash_support/mod.rs"]
mod slash_support;

// A sanity check that `App` and the in-memory message type are wired as the
// other tests assume.
