//! The status bar along the bottom of the window.
//!
//! One row, kept to what the user needs while working:
//!
//! - a spinner while the focused session's turn is running, then the status
//!   text (or `Ready`), the focused session's Auto-fix state and the
//!   `/research` web-phase countdown;
//! - the compaction/compression activity, the provider health dot with
//!   `Model:` and the model label, the thinking level, the agent, and the
//!   share of the context window the last request used;
//! - states that need attention, shown only while they apply: the quota from
//!   80%, a non-default execution backend, the `AUTO` chip with its
//!   countdown, the `YOLO` chip, the code-index busy tags, and the
//!   `N need you` badge for background sessions waiting on the user;
//! - a `Details` button whose popup holds the rest: the application name and
//!   version, `Project:`, `Branch:`, the last-prompt tag, the backend, the
//!   quota, and a chip for every service ([`Service`]: GCF, code index,
//!   activity log, autopilot, edit log, telemetry, YOLO).
//!
//! Every service indicator, in the bar and in Details, is a dense Material
//! status chip ([`crate::material::chip::chip_dense`]): a green check while
//! the service is on and a red cross while it is off, and a click asks for the service's own `/<command> on|off`
//! slash command, which [`status_bar_ui`] returns for the shell to run.
//!
//! A run's cost and tokens appear in the run-cost banner
//! (`crate::dialogs::banner`) and the log, not here. The labels are app-core's
//! i18n strings. The row wraps when the window is too narrow for it.
//! Characters the window's fonts cannot draw (the status text's icons) are
//! drawn as ASCII stand-ins ([`drawable`]).
//!
//! Dependencies: `codemason-app-core` (the App), `codemason-config` (i18n
//! labels and the GCF, activity-log and YOLO switches),
//! `codemason-tools-core` (the edit-log switch), `egui`,
//! [`crate::material`] and [`crate::palette`].

use codemason_app_core::app::App;
use codemason_app_core::styled::ThemeColor;
use codemason_config::i18n::{MessageKey, t};
use egui::{Color32, RichText, Role, WidgetInfo};

use crate::material::chip::{ChipKind, chip_dense};
use crate::palette::Palette;
use crate::styled_text::drawable;

/// Accessible label of the status bar.
pub const STATUS_BAR_LABEL: &str = "Status bar";

/// Accessible label of the spinner shown while a turn is running.
pub const BUSY_LABEL: &str = "Working";

/// How often the busy spinner asks for a new frame.
const SPINNER_FRAME: std::time::Duration = std::time::Duration::from_millis(60);

/// A progress spinner: a three-quarter arc that turns once a second,
/// labelled `label` for assistive technology.
///
/// It asks for its next frame after [`SPINNER_FRAME`] instead of at once, so
/// an idle window still sleeps between frames.
pub(crate) fn busy_spinner(ui: &mut egui::Ui, palette: &Palette, label: &str) {
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
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, label));
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

/// A service the status bar shows as an on/off chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// GCF tool-result encoding.
    Gcf,
    /// The code index.
    CodeIndex,
    /// The activity log.
    ActivityLog,
    /// Autopilot (autonomous operation).
    Autopilot,
    /// The edit log.
    EditLog,
    /// OpenTelemetry export.
    Telemetry,
    /// YOLO mode (tool calls run without asking).
    Yolo,
}

impl Service {
    /// Every service, in the order Details shows them.
    pub const ALL: [Self; 7] = [
        Self::Gcf,
        Self::CodeIndex,
        Self::ActivityLog,
        Self::EditLog,
        Self::Telemetry,
        Self::Autopilot,
        Self::Yolo,
    ];

    /// The chip's text, also its accessible name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gcf => "GCF",
            Self::CodeIndex => "CDX",
            Self::ActivityLog => "LOG",
            Self::Autopilot => "AUTO",
            Self::EditLog => "EDIT",
            Self::Telemetry => "TELE",
            Self::Yolo => "YOLO",
        }
    }

    /// The service's full name, shown when the pointer rests on the chip.
    const fn title(self) -> &'static str {
        match self {
            Self::Gcf => "GCF tool-result encoding",
            Self::CodeIndex => "Code index",
            Self::ActivityLog => "Activity log",
            Self::Autopilot => "Autopilot",
            Self::EditLog => "Edit log",
            Self::Telemetry => "Telemetry",
            Self::Yolo => "YOLO mode",
        }
    }

    /// Whether the service is on.
    pub fn is_on(self, app: &App) -> bool {
        match self {
            Self::Gcf => codemason_config::gcf::is_enabled(),
            Self::CodeIndex => app.code_index_enabled,
            Self::ActivityLog => codemason_config::activity_log::is_enabled(),
            Self::Autopilot => app.session.autopilot_enabled,
            Self::EditLog => codemason_tools_core::edit_log::is_edit_log_enabled(),
            Self::Telemetry => app.session_processor.telemetry.is_enabled(),
            Self::Yolo => codemason_config::yolo::is_enabled(),
        }
    }

    /// The slash command that turns the service on (`on`) or off.
    pub const fn command(self, on: bool) -> &'static str {
        match (self, on) {
            (Self::Gcf, true) => "/gcf on",
            (Self::Gcf, false) => "/gcf off",
            (Self::CodeIndex, true) => "/codeindex on",
            (Self::CodeIndex, false) => "/codeindex off",
            (Self::ActivityLog, true) => "/alog on",
            (Self::ActivityLog, false) => "/alog off",
            (Self::Autopilot, true) => "/autopilot on",
            (Self::Autopilot, false) => "/autopilot off",
            (Self::EditLog, true) => "/editlog on",
            (Self::EditLog, false) => "/editlog off",
            (Self::Telemetry, true) => "/telemetry on",
            (Self::Telemetry, false) => "/telemetry off",
            (Self::Yolo, true) => "/yolo on",
            (Self::Yolo, false) => "/yolo off",
        }
    }

    /// The chip kind for the state `on`: the services that let the agent
    /// act without asking are caution chips.
    const fn kind(self, on: bool) -> ChipKind {
        let caution = match self {
            Self::Autopilot | Self::Yolo => true,
            Self::Gcf | Self::CodeIndex | Self::ActivityLog | Self::EditLog | Self::Telemetry => {
                false
            }
        };
        ChipKind::Status { on, caution }
    }
}

/// A service's chip: selected while the service is on. A click returns the
/// slash command that flips it.
fn service_chip(
    ui: &mut egui::Ui,
    app: &App,
    palette: &Palette,
    service: Service,
) -> Option<&'static str> {
    let on = service.is_on(app);
    let command = service.command(!on);
    let state = if on { "on" } else { "off" };
    let response = chip_dense(ui, palette, service.kind(on), None, service.label()).on_hover_text(
        format!("{}: {state}. Click to run {command}", service.title()),
    );
    response.clicked().then_some(command)
}

/// Accessible label of the button that opens the details popup.
pub const DETAILS_LABEL: &str = "Details";

/// The quota percentage from which the quota is shown in the bar itself.
const QUOTA_WARN_PERCENT: u32 = 80;

/// The colour a quota percentage is drawn in.
fn quota_color(percent: u32, palette: &Palette) -> Color32 {
    match percent {
        95.. => palette.error,
        QUOTA_WARN_PERCENT..=94 => palette.warning,
        _ => palette.success,
    }
}

/// The share of the model's context window the last request used, in percent.
fn context_percent(app: &App) -> Option<u64> {
    let window = app.active_context_window_tokens()? as u64;
    let used = app.session.last_input_tokens;
    (window > 0 && used > 0).then(|| (used.saturating_mul(100) / window).min(100))
}

/// The bar: what the turn is doing, which model and agent it uses, and the
/// states that need attention. Everything else is in [`details`].
///
/// Returns the slash command a clicked service chip asks for.
fn summary(ui: &mut egui::Ui, app: &App, palette: &Palette) -> Option<&'static str> {
    let mut command = None;
    if app.session.is_processing {
        busy_spinner(ui, palette, BUSY_LABEL);
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
                palette.color(ThemeColor::Info),
                true,
            );
        }
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
    if !app.session.agent_name.is_empty() {
        item(
            ui,
            format!("Agent: {}", app.session.agent_name),
            palette.muted,
            false,
        );
    }
    if let Some(percent) = context_percent(app) {
        item(ui, format!("Context: {percent}%"), palette.muted, false);
    }
    if let Some(percent) = app.quota_percent.map(|quota| quota as u32)
        && percent >= QUOTA_WARN_PERCENT
    {
        item(
            ui,
            format!("quota: {percent}%"),
            quota_color(percent, palette),
            true,
        );
    }
    // A non-default backend changes where commands run, so it stays in view.
    if !app.session.active_backend.is_empty() && app.session.active_backend != "local" {
        item(
            ui,
            format!("Backend:{}", app.session.active_backend),
            palette.color(ThemeColor::Info),
            false,
        );
    }
    if app.session.autopilot_enabled {
        command = command.or(service_chip(ui, app, palette, Service::Autopilot));
        if let Some(remaining) = autopilot_remaining_secs(app) {
            item(
                ui,
                format!("[{}]", format_countdown(remaining)),
                palette.muted,
                true,
            );
        }
    }
    if codemason_config::yolo::is_enabled() {
        command = command.or(service_chip(ui, app, palette, Service::Yolo));
    }
    if app.code_index_busy {
        item(ui, "BUSY:cdx-idx", palette.warning, true);
    }
    if app.code_index_graph_busy {
        item(ui, "BUSY:cdx-graph", palette.color(ThemeColor::Info), true);
    }
    let waiting = app.background.waiting_count();
    if waiting > 0 {
        let badge = RichText::new(format!(" {waiting} need you "))
            .color(palette.background)
            .background_color(palette.warning)
            .strong();
        ui.label(badge);
    }
    // Click-only: a button that can hold the keyboard focus would share Enter
    // with the prompt, which sends the draft.
    let button = egui::Button::new(DETAILS_LABEL).sense(egui::Sense::CLICK);
    // The popup stays open on a chip's click, so several services can be
    // flipped in one visit.
    let config = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    // A fixed id: the AUTO and YOLO chips before the button come and go, and
    // the button's automatic id (and so its popup's) would change with them,
    // closing the popup when one of them is flipped from inside it.
    let fixed = egui::UiBuilder::new().scope_id(egui::Id::unique("status_bar_details"));
    let (_, picked) = ui
        .scope_builder(fixed, |ui| {
            egui::containers::menu::MenuButton::from_button(button)
                .config(config)
                .ui(ui, |ui| details(ui, app, palette))
        })
        .inner;
    command.or(picked.and_then(|inner| inner.inner))
}

/// The details popup: identity, place, the last prompt, the backend, the
/// quota and a chip for every service.
///
/// Returns the slash command a clicked service chip asks for.
fn details(ui: &mut egui::Ui, app: &App, palette: &Palette) -> Option<&'static str> {
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
        item(ui, tag, palette.color(ThemeColor::Info), true);
    }
    if !app.session.active_backend.is_empty() {
        item(
            ui,
            format!("Backend:{}", app.session.active_backend),
            palette.color(ThemeColor::Info),
            false,
        );
    }
    if let Some(percent) = app.quota_percent.map(|quota| quota as u32) {
        item(
            ui,
            format!("quota: {percent}%"),
            quota_color(percent, palette),
            true,
        );
    }
    ui.separator();
    // Wrapped, so every chip stays reachable in a narrow or zoomed window.
    ui.horizontal_wrapped(|ui| {
        Service::ALL.into_iter().fold(None, |picked, service| {
            picked.or(service_chip(ui, app, palette, service))
        })
    })
    .inner
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
///
/// # Returns
///
/// The slash command a clicked service chip asks for (such as
/// `/yolo off`), for the caller to run through
/// `App::execute_slash_command`.
pub fn status_bar_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) -> Option<&'static str> {
    app.update_cached_provider_model_label(app.compute_provider_model_label());
    let inner = ui.horizontal_wrapped(|ui| {
        // As tall as a chip whether or not one shows, so the bar keeps its
        // height when YOLO or autopilot turns on or off.
        ui.set_min_height(crate::material::chip::DENSE_HEIGHT);
        summary(ui, app, palette)
    });
    let bar = ui.interact(
        inner.response.rect,
        ui.scope_id().with("status_bar"),
        egui::Sense::hover(),
    );
    bar.widget_info(|| WidgetInfo::labeled(Role::Status, true, STATUS_BAR_LABEL));
    inner.inner
}

/// Accessible name of the status bar's zoom slider.
pub const ZOOM_LABEL: &str = "Zoom";

/// The zoom the status bar's slider covers, as egui zoom factors.
pub const ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.5..=2.0;

/// The status bar's zoom control (the word-processor layout): the zoom as a
/// percentage and a compact Material slider over [`ZOOM_RANGE`] that sets
/// the window's zoom factor (when the pointer is released, or at once from
/// the arrow keys), which Ctrl+Plus and Ctrl+Minus also change.
///
/// # Arguments
///
/// * `ui` - the status bar's ui, laid out right to left.
/// * `palette` - the colours.
pub fn zoom_ui(ui: &mut egui::Ui, palette: &Palette) {
    let ctx = ui.ctx().clone();
    // While the pointer is down the new zoom is only held: applying it would
    // rescale the slider under the pointer, which would move the value again.
    let pending = ui.scope_id().with("zoom_pending");
    let held = ui.data(|d| d.get_temp::<f32>(pending));
    let mut zoom = held.unwrap_or_else(|| ctx.zoom_factor());
    let size = egui::vec2(140.0, ui.spacing().interact_size.y);
    let slider = crate::material::feedback::slider_sized(
        ui, palette, &mut zoom, ZOOM_RANGE, ZOOM_LABEL, size,
    );
    if slider.is_pointer_button_down_on() {
        ui.data_mut(|d| d.insert_temp(pending, zoom));
    } else if slider.changed() || held.is_some() {
        ui.data_mut(|d| d.remove::<f32>(pending));
        // Whole percent steps, so the label and the zoom agree.
        ctx.set_zoom_factor((zoom * 100.0).round() / 100.0);
    }
    ui.label(egui::RichText::new(format!("{:.0}%", zoom * 100.0)).color(palette.muted));
}
