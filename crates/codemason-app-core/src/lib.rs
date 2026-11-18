//! Frontend-neutral application logic and types behind the desktop GUI.
//!
//! Holds styled text ([`styled`]), input events ([`input_event`]), geometry
//! ([`geometry`]), the frontend-neutral application state ([`app::App`]) with
//! its input handling ([`input`], [`input_field`]) and the supporting state
//! modules (status, theme tokens, sidebar and menu state, research progress,
//! message text and line wrapping, markdown rendering, the log channel and
//! its tracing layer). `codemason-gui` renders them.

#![deny(unsafe_code)]

pub mod activity;
pub mod app;
pub mod branch_names;
pub mod clipboard;
pub mod context_panel;
pub mod deck_progress;
pub mod diff_view;
pub mod geometry;
pub mod input;
pub mod input_event;
pub mod input_field;
pub mod input_prompt;
pub mod line_wrap;
pub mod log_channel;
pub mod markdown;
pub mod message_blocks;
pub mod message_lines;
pub mod message_text;
pub mod open_in;
pub mod permission_preview;
pub mod project_properties;
pub mod research_adapter;
pub mod research_progress;
pub mod session_menu;
pub mod sessions_sidebar_state;
pub mod shortcuts;
pub mod status;
pub mod styled;
pub mod syntax_highlight;
pub mod text_utils;
pub mod theme_tokens;
pub mod tracing_layer;
pub mod transcript_prefs;
pub mod usage_stats;

/// Self-healing staleness caps for spinner/progress latches (in seconds).
///
/// The model-list and model-download spinners are set on a single event and
/// cleared by a single `*Finished` event. The event bus is broadcast, so an
/// event burst can make the bridge drop the clearing event (`Lagged`), after
/// which the latch would otherwise stay set forever: a stuck popup plus a
/// 250 ms idle deadline for the remaining process lifetime. When a latch
/// outlives its cap, `App::poll_stale_spinners` clears it.
const MODEL_LOADING_STALE_SECS: u64 = 120;

/// Self-healing staleness cap for model downloads (large models can take
/// tens of minutes over slow links).
const MODEL_DOWNLOAD_STALE_SECS: u64 = 45 * 60;

/// Self-healing staleness cap for a wedged benchmark run.
const BENCH_STALE_SECS: u64 = 30 * 60;
