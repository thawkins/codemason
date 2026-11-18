//! The research and deck progress views, drawn above the transcript while
//! a `/research` run is going.
//!
//! app-core tracks each run in `App::research_progress`
//! ([`ResearchProgress`], fed by the research observer's progress notices;
//! a `/research deck` run carries a
//! [`codemason_app_core::deck_progress::DeckTracker`]). The TUI renders the
//! same trackers as a self-updating message in the transcript, which the
//! GUI transcript shows too; this view keeps the running ones in sight at
//! the top of the Messages pane, one framed group per run:
//!
//! - a research run ([`RESEARCH_PROGRESS_LABEL`]): its name, topic and
//!   options, one row per step (`status phase - detail`, coloured by
//!   status), the live web-fetch totals and the web phase's countdown;
//! - a deck run ([`DECK_PROGRESS_LABEL`]): its name and one row per stage
//!   (`status stage - [n/m] detail`), and the failure reason if it failed.
//!
//! Which runs are shown comes from app-core's state only: a run is shown
//! while `ResearchProgress::is_running` says it is going. A deck run that
//! failed stays, with its `Failed: <reason>` row, as the TUI's progress
//! message keeps showing it, until app-core drops its tracker (a new run
//! under the same name, which also retires the TUI's message). Any other run
//! leaves the view when it ends; the transcript keeps its final report.
//!
//! Dependencies: `codemason-app-core` (the trackers) and `egui`.

use std::time::{Duration, Instant};

use codemason_app_core::app::App;
use codemason_app_core::deck_progress::{DeckStageRow, DeckTracker};
use codemason_app_core::research_progress::{ResearchProgress, ResearchStep, StepStatus};
use codemason_app_core::styled::ThemeColor;
use egui::{Color32, FontId, RichText, WidgetInfo};

use crate::palette::Palette;
use crate::styled_text::{FONT_SIZE, drawable};

/// Accessible label of a research run's progress view.
pub const RESEARCH_PROGRESS_LABEL: &str = "Research progress";

/// Accessible label of a deck run's progress view.
pub const DECK_PROGRESS_LABEL: &str = "Deck progress";

/// Share of the pane's height the views may take before they scroll.
const MAX_HEIGHT_FRACTION: f32 = 0.4;

/// Whether the view shows `progress`: while app-core says the run is
/// going ([`ResearchProgress::is_running`]), and, for a deck run that
/// failed, from then on with its reason, as the TUI's progress message keeps
/// showing it. The view never decides a run's state itself.
fn shown(progress: &ResearchProgress) -> bool {
    progress.is_running()
        || progress
            .deck
            .as_ref()
            .is_some_and(|deck| deck.failure.is_some())
}

/// The word a step's status is shown with.
fn step_word(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Started => "running",
        StepStatus::Done => "done",
        StepStatus::Error => "error",
        StepStatus::Excluded => "excluded",
        StepStatus::Skipped => "skipped",
    }
}

/// The colour a step's row is drawn in.
fn step_color(status: StepStatus, palette: &Palette) -> Color32 {
    match status {
        StepStatus::Started => palette.accent,
        StepStatus::Done => palette.success,
        StepStatus::Error => palette.error,
        StepStatus::Excluded | StepStatus::Skipped => palette.muted,
    }
}

/// The colour a deck stage row in status `word` is drawn in.
fn stage_color(word: &str, palette: &Palette) -> Color32 {
    match word {
        "started" => palette.accent,
        "done" => palette.success,
        _ => palette.muted,
    }
}

/// Draw the progress views of the App's running research runs at the top
/// of `ui`; draws nothing when no run is going.
///
/// # Arguments
///
/// * `ui` - the Messages pane's body, above the transcript.
/// * `app` - the App whose research trackers are drawn.
/// * `palette` - the colours to draw with.
pub fn research_ui(ui: &mut egui::Ui, app: &App, palette: &Palette) {
    let runs: Vec<&ResearchProgress> = app.research_progress.iter().filter(|p| shown(p)).collect();
    if runs.is_empty() {
        return;
    }
    let max_height = ui.available_height() * MAX_HEIGHT_FRACTION;
    egui::ScrollArea::vertical()
        .id_salt("research_progress")
        .max_height(max_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (index, progress) in runs.into_iter().enumerate() {
                match &progress.deck {
                    Some(deck) => group(ui, index, DECK_PROGRESS_LABEL, |ui| {
                        deck_rows(ui, &progress.name, deck, palette);
                    }),
                    None => group(ui, index, RESEARCH_PROGRESS_LABEL, |ui| {
                        research_rows(ui, progress, palette);
                    }),
                }
            }
        });
    ui.separator();
}

/// Draw one run's rows in a frame, labelled `label` for assistive tech.
fn group(ui: &mut egui::Ui, index: usize, label: &str, rows: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        rows(ui);
    });
    let response = ui.interact(
        frame.response.rect,
        ui.scope_id().with(("research_group", index)),
        egui::Sense::hover(),
    );
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Group, true, label));
}

/// One monospace row of `text` in `color`.
fn row(ui: &mut egui::Ui, text: &str, color: Color32) {
    let font = FontId::monospace(FONT_SIZE);
    let text = drawable(ui.ctx(), &font, text);
    ui.label(RichText::new(text).font(font).color(color));
}

/// The rows of a research run.
fn research_rows(ui: &mut egui::Ui, progress: &ResearchProgress, palette: &Palette) {
    ui.label(
        RichText::new(format!("{RESEARCH_PROGRESS_LABEL} - {}", progress.name))
            .strong()
            .color(palette.text),
    );
    row(ui, &format!("Topic: {}", progress.topic), palette.muted);
    if let Some(options) = &progress.options {
        row(ui, &format!("Options: {options}"), palette.muted);
    }
    for step in &progress.steps {
        row(ui, &step_text(step), step_color(step.status, palette));
    }
    if progress.fetched_count > 0 || progress.failed_count > 0 || progress.excluded_live > 0 {
        let excluded = if progress.excluded_live > 0 {
            format!(", {} excluded", progress.excluded_live)
        } else {
            String::new()
        };
        row(
            ui,
            &format!(
                "Web fetch totals: {} fetched, {} failed{excluded}",
                progress.fetched_count, progress.failed_count
            ),
            palette.color(ThemeColor::Info),
        );
    }
    if let Some(deadline) = progress.web_phase_deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        if !left.is_zero() {
            row(
                ui,
                &format!("Web phase: {}s left", left.as_secs()),
                palette.warning,
            );
            // Tick the countdown once a second.
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
    }
}

/// The text of one step row: `status phase - detail`, the detail's later
/// lines joined with ` / `.
fn step_text(step: &ResearchStep) -> String {
    let detail: Vec<&str> = step.detail.lines().collect();
    format!(
        "{:<8} {:<10} - {}",
        step_word(step.status),
        step.phase,
        detail.join(" / ")
    )
}

/// The rows of a deck run.
fn deck_rows(ui: &mut egui::Ui, name: &str, deck: &DeckTracker, palette: &Palette) {
    ui.label(
        RichText::new(format!("{DECK_PROGRESS_LABEL} - {name}"))
            .strong()
            .color(palette.text),
    );
    for stage in &deck.rows {
        let word = stage.status.as_str();
        row(ui, &stage_text(stage), stage_color(word, palette));
    }
    if let Some(reason) = &deck.failure {
        row(ui, &format!("Failed: {reason}"), palette.error);
    }
}

/// The text of one deck stage row: `status stage - [n/m] detail`.
fn stage_text(stage: &DeckStageRow) -> String {
    format!(
        "{:<8} {:<16} - {}{}",
        stage.status.as_str(),
        stage.stage.label(),
        stage.counter(),
        stage.detail
    )
}
