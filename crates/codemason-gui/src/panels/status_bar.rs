//! The status bar along the bottom of the window.
//!
//! It shows the fields the TUI's status bar (`layout_statusbar.rs` in
//! codemason-tui) shows, read from the App, in the same two lines:
//!
//! - Line 1: the application name and version, `Project:` with the working
//!   directory, `Branch:` with the git branch, the last-prompt tag, the
//!   status text (or `Ready`), the focused session's Auto-fix state, the
//!   `/research` web-phase countdown, and the `N need you` badge for
//!   background sessions waiting on the user.
//! - Line 2: the execution backend, the compaction/compression activity,
//!   the provider health dot with `Model:` and the model label, the thinking
//!   level, the rate-limit quota, the service indicators (GCF, code index,
//!   activity log, autopilot with its countdown, edit log, telemetry, YOLO)
//!   and the code-index busy tags.
//!
//! The TUI status bar has no cost or token field: a run's cost and tokens
//! appear in the run-cost banner (`crate::dialogs::banner`) and the log, as
//! in the TUI. The labels are app-core's i18n strings; the TUI's
//! responsive narrowing is not needed in a resizable window, so every field
//! is shown and the line wraps when the window is narrow. Characters the
//! window's fonts cannot draw (the status text's icons) are drawn as ASCII
//! stand-ins ([`drawable`]).
//!
//! Dependencies: `codemason-app-core` (the App), `codemason-config` (i18n
//! labels and the GCF, activity-log and YOLO switches),
//! `codemason-tools-core` (the edit-log switch), `egui` and
//! [`crate::palette`].

use codemason_app_core::app::App;
use codemason_app_core::styled::ThemeColor;
use codemason_config::i18n::{MessageKey, t};
use egui::{Color32, RichText, Role, WidgetInfo};

use crate::palette::Palette;
use crate::styled_text::drawable;

/// Accessible label of the status bar.
pub const STATUS_BAR_LABEL: &str = "Status bar";

/// Accessible label of the spinner shown while a turn is running.
pub const BUSY_LABEL: &str = "Working";

/// How often the busy spinner asks for a new frame.
const SPINNER_FRAME: std::time::Duration = std::time::Duration::from_millis(60);

/// The turn-in-progress spinner: a three-quarter arc that turns once a second.
///
/// It asks for its next frame after [`SPINNER_FRAME`] instead of at once, so
/// an idle window still sleeps between frames.
fn busy_spinner(ui: &mut egui::Ui, palette: &Palette) {
    let size = ui.text_style_height(&egui::TextStyle::Body);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let radius = size.mul_add(0.5, -2.0);
        let start = (ui.input(|i| i.time) * std::f64::consts::TAU) as f32;
        let points: Vec<egui::Pos2> = (0..=24)
            .map(|step| {
                let sweep = step as f32 / 24.0 * std::f32::consts::TAU;
                let angle = sweep.mul_add(0.75, start);
                rect.center() + radius * egui::vec2(angle.cos(), angle.sin())
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            points,
            egui::Stroke::new(2.0, palette.accent),
        ));
    }
    ui.ctx().request_repaint_after(SPINNER_FRAME);
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, BUSY_LABEL));
}

/// Characters of the last prompt shown in its tag, as in the TUI.
const PROMPT_TAG_CHARS: usize = 48;

/// The last-prompt tag: the first [`PROMPT_TAG_CHARS`] characters of the
/// prompt, plus `....` when it is longer, in square brackets; empty when
/// there is no prompt (the TUI's `prompt_display_text`).
fn prompt_tag(prompt: &str) -> String {
    if prompt.is_empty() {
        return String::new();
    }
    let mut chars = prompt.chars();
    let mut text: String = chars.by_ref().take(PROMPT_TAG_CHARS).collect();
    if chars.next().is_some() {
        text.push_str("....");
    }
    format!("[{text}]")
}

/// Remaining wall-clock seconds of an autopilot run with a time limit.
fn autopilot_remaining_secs(app: &App) -> Option<u64> {
    if !app.session.autopilot_enabled {
        return None;
    }
    let limit = app.session.autopilot_time_limit_secs?;
    let started = app.session.autopilot_started_at?;
    Some(limit.saturating_sub(started.elapsed().as_secs()))
}

/// `HH:MM:SS` for a number of seconds.
fn format_countdown(total_secs: u64) -> String {
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// One status item: a label drawn in `color`.
fn item(ui: &mut egui::Ui, text: impl Into<String>, color: Color32, strong: bool) {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = drawable(ui.ctx(), &font, &text.into());
    let mut rich = RichText::new(text).color(color);
    if strong {
        rich = rich.strong();
    }
    ui.label(rich);
}

/// A service indicator: its name and `[ok]` (enabled) or `[x]` (disabled).
fn indicator(ui: &mut egui::Ui, name: &str, enabled: bool, palette: &Palette) {
    let (mark, color) = if enabled {
        ("[ok]", palette.success)
    } else {
        ("[x]", palette.error)
    };
    item(ui, format!("{name} {mark}"), color, false);
}

/// Line 1: identity, place, prompt and status.
fn line1(ui: &mut egui::Ui, app: &App, palette: &Palette) {
    if app.session.is_processing {
        busy_spinner(ui, palette);
    }
    item(
        ui,
        format!("codemason v{}", env!("CARGO_PKG_VERSION")),
        palette.success,
        true,
    );
    item(
        ui,
        format!("{}{}", t(MessageKey::StatusProject), app.cwd),
        palette.text,
        false,
    );
    if let Some(branch) = &app.git_branch {
        item(
            ui,
            format!("{}{branch}", t(MessageKey::StatusBranch)),
            palette.text,
            false,
        );
    }
    let tag = prompt_tag(&app.session.last_prompt);
    if !tag.is_empty() {
        item(ui, tag, palette.color(ThemeColor::Cyan), true);
    }
    if !app.status.is_empty() && !app.status.eq_ignore_ascii_case("Ready") {
        item(ui, app.status.as_str(), palette.warning, true);
    } else {
        item(
            ui,
            t(MessageKey::StatusReady).trim_end(),
            palette.success,
            false,
        );
    }
    if let Some(summary) = app
        .session
        .session_id
        .as_ref()
        .and_then(|id| app.autofix_status.get(id))
        .filter(|summary| **summary != app.status)
    {
        item(ui, summary.as_str(), palette.muted, false);
    }
    if let Some(deadline) = app
        .research_progress
        .iter()
        .filter(|p| !p.done)
        .find_map(|p| p.web_phase_deadline)
    {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if !remaining.is_zero() {
            let secs = remaining.as_secs();
            item(
                ui,
                format!("web:{}:{:02}", secs / 60, secs % 60),
                palette.color(ThemeColor::Cyan),
                true,
            );
        }
    }
    let waiting = app.background.waiting_count();
    if waiting > 0 {
        let badge = RichText::new(format!(" {waiting} need you "))
            .color(palette.ansi[0])
            .background_color(palette.warning)
            .strong();
        ui.label(badge);
    }
}

/// Line 2: backend, model and services.
fn line2(ui: &mut egui::Ui, app: &App, palette: &Palette) {
    if !app.session.active_backend.is_empty() {
        item(
            ui,
            format!("Backend:{}", app.session.active_backend),
            palette.color(ThemeColor::Cyan),
            false,
        );
    }
    let (compact, compress) = (
        app.session.compact_in_progress,
        app.session.compress_in_progress,
    );
    if compact || compress {
        let label = match (compact, compress) {
            (true, true) => "cmp+compress",
            (true, false) => "compacting",
            (false, _) => "compressing",
        };
        item(ui, label, palette.warning, true);
    }
    if let Some(label) = app.provider_model_label() {
        let health = match app.provider_health_status() {
            Some(true) => palette.success,
            Some(false) => palette.error,
            None => palette.warning,
        };
        item(ui, "*", health, true);
        item(
            ui,
            format!("{}{label}", t(MessageKey::StatusModel)),
            palette.text,
            true,
        );
    }
    if let Some(level) = app.session.selected_thinking_level {
        item(
            ui,
            format!(
                "{} {}",
                t(MessageKey::StatusThinking).trim(),
                App::thinking_level_short(level)
            ),
            palette.muted,
            false,
        );
    }
    if let Some(quota) = app.quota_percent {
        let percent = quota as u32;
        let color = match percent {
            95.. => palette.error,
            80..=94 => palette.warning,
            _ => palette.success,
        };
        item(ui, format!("quota: {percent}%"), color, true);
    }
    indicator(ui, "GCF", codemason_config::gcf::is_enabled(), palette);
    indicator(ui, "CDX", app.code_index_enabled, palette);
    indicator(
        ui,
        "LOG",
        codemason_config::activity_log::is_enabled(),
        palette,
    );
    indicator(ui, "AUTO", app.session.autopilot_enabled, palette);
    if let Some(remaining) = autopilot_remaining_secs(app) {
        item(
            ui,
            format!("[{}]", format_countdown(remaining)),
            palette.muted,
            true,
        );
    }
    indicator(
        ui,
        "EDIT",
        codemason_tools_core::edit_log::is_edit_log_enabled(),
        palette,
    );
    indicator(
        ui,
        "TELE",
        app.session_processor.telemetry.is_enabled(),
        palette,
    );
    indicator(ui, "YOLO", codemason_config::yolo::is_enabled(), palette);
    if app.code_index_busy {
        item(ui, "BUSY:cdx-idx", palette.warning, true);
    }
    if app.code_index_graph_busy {
        item(ui, "BUSY:cdx-graph", palette.color(ThemeColor::Cyan), true);
    }
}

/// Draw the status bar into `ui`.
///
/// Like the TUI, it refreshes the App's cached model label first, so the
/// label follows a model or provider change.
///
/// # Arguments
///
/// * `ui` - the status bar's panel.
/// * `app` - the App whose status is shown.
/// * `palette` - the colours to draw with.
pub fn status_bar_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    app.update_cached_provider_model_label(app.compute_provider_model_label());
    let rect = ui
        .vertical(|ui| {
            ui.horizontal_wrapped(|ui| line1(ui, app, palette));
            ui.horizontal_wrapped(|ui| line2(ui, app, palette));
        })
        .response
        .rect;
    let bar = ui.interact(rect, ui.scope_id().with("status_bar"), egui::Sense::hover());
    bar.widget_info(|| WidgetInfo::labeled(Role::Status, true, STATUS_BAR_LABEL));
}
