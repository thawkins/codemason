//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: each `#[path]`-included child file declares its own
// `mod support;` / `mod slash_support;`, so a shared helper file is a module
// of several children. Under one-binary-per-crate that loads it more than
// once, which clippy::duplicate_mod flags; hoisting it would require editing
// every child file. See AGENTS.md ("Test Layout").

/// The one process-wide lock that serialises tests which mutate or observe
/// process-global state (`set_current_dir`, `XDG_CONFIG_HOME`, `HOME`,
/// `PATH`).
///
/// Before consolidation each test file was its own binary, so a file-local
/// mutex was enough. One binary per crate merges them, so every mutating test
/// in every child file must contend on the SAME mutex; `crate::` resolves at
/// this binary root, which is what makes that possible. See AGENTS.md
/// ("Test Layout").
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering from a poisoned lock (a panicking
/// test must not wedge every later test).
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Disable, in `storage`, every provider the TUI would auto-detect from the
/// host environment (a credential env var, the Bedrock `AWS_*` pair, or
/// `OLLAMA_HOST`).
///
/// A CI runner or cloud container commonly exports `GITHUB_TOKEN` or AWS keys,
/// which `App::detect_provider` reports as configured Copilot / Bedrock. A test
/// asserting the "no provider" or "DB-stored provider" path calls this so it
/// does not depend on the host. Uses the same `provider_<id>_disabled` setting
/// the `/provider reset` flow writes, so no environment variable is mutated and
/// no lock is needed. Providers stored in the DB by the test itself stay
/// detectable unless their env var is also set on the host.
pub fn disable_host_providers(storage: &codemason_storage::Storage) {
    for (pid, _) in codemason_tui::app::PROVIDER_LIST {
        let from_env = match *pid {
            "bedrock" => {
                std::env::var_os("AWS_ACCESS_KEY_ID").is_some()
                    || std::env::var_os("AWS_SECRET_ACCESS_KEY").is_some()
            }
            "ollama" => std::env::var_os("OLLAMA_HOST").is_some(),
            other => codemason_config::credential_env::provider_credential_env(other).is_some(),
        };
        if from_env {
            storage
                .set_setting(&format!("provider_{pid}_disabled"), "true")
                .expect("disable host-detected provider");
        }
    }
}

/// [`disable_host_providers`] on `app.storage`, then re-run provider detection
/// so `app.configured_provider` (computed when the app was built) reflects it.
pub fn without_host_providers(app: &mut codemason_tui::App) {
    disable_host_providers(&app.storage);
    app.configured_provider = codemason_tui::App::detect_provider(&app.storage);
}

/// Whether POSIX permission bits actually deny this process.
///
/// Root (or any process with `CAP_DAC_OVERRIDE`) reads and writes through
/// `0o000`/`0o500`, so a test that provokes a permission-denied error cannot
/// observe one there. Such a test calls this first and skips, rather than
/// failing for a reason unrelated to the code under test. Probed once per
/// process by writing into a `0o500` directory under `target/temp`.
///
/// Off Unix there is no mode-bit model to bypass, so this reports `true`.
#[cfg(not(unix))]
pub fn permission_bits_enforced() -> bool {
    true
}

/// See the non-Unix variant above for the contract.
#[cfg(unix)]
pub fn permission_bits_enforced() -> bool {
    use std::os::unix::fs::PermissionsExt;

    static ENFORCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENFORCED.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
        let Ok(()) = std::fs::create_dir_all(&root) else {
            return true;
        };
        let Ok(dir) = tempfile::tempdir_in(&root) else {
            return true;
        };
        let Ok(()) = std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500))
        else {
            return true;
        };
        let denied = std::fs::write(dir.path().join("probe"), b"").is_err();
        let _ = std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700));
        denied
    })
}

#[path = "test_adapters.rs"]
mod test_adapters;
#[path = "test_agents_teams_cursor.rs"]
mod test_agents_teams_cursor;
#[path = "test_agents_window_frame.rs"]
mod test_agents_window_frame;
#[path = "test_agents_window_refusals.rs"]
mod test_agents_window_refusals;
#[path = "test_agents_window_table.rs"]
mod test_agents_window_table;
#[path = "test_agentui_t019_offline.rs"]
mod test_agentui_t019_offline;
#[path = "test_alt_a_agents_window.rs"]
mod test_alt_a_agents_window;
#[path = "test_alt_t_teams_window.rs"]
mod test_alt_t_teams_window;
#[path = "test_autopilot_alt_o.rs"]
mod test_autopilot_alt_o;
#[path = "test_backend_command.rs"]
mod test_backend_command;
#[path = "test_busy_send_guard.rs"]
mod test_busy_send_guard;
#[path = "test_button.rs"]
mod test_button;
#[path = "test_button_counts.rs"]
mod test_button_counts;
#[path = "test_codeindex_indicators.rs"]
mod test_codeindex_indicators;
#[path = "test_compaction_label.rs"]
mod test_compaction_label;
#[path = "test_compression_indicator.rs"]
mod test_compression_indicator;
#[path = "test_connector_catalogue_mouse.rs"]
mod test_connector_catalogue_mouse;
#[path = "test_connector_catalogue_panel.rs"]
mod test_connector_catalogue_panel;
#[path = "test_context_panel_t002.rs"]
mod test_context_panel_t002;
#[path = "test_context_panel_t010.rs"]
mod test_context_panel_t010;
#[path = "test_context_panel_t012.rs"]
mod test_context_panel_t012;
#[path = "test_context_panel_t016.rs"]
mod test_context_panel_t016;
#[path = "test_dialog.rs"]
mod test_dialog;
#[path = "test_event_swarm_research_deep.rs"]
mod test_event_swarm_research_deep;
#[path = "test_gcf_indicator.rs"]
mod test_gcf_indicator;
#[path = "test_hit_rect_lifecycle.rs"]
mod test_hit_rect_lifecycle;
#[path = "test_i18n_command.rs"]
mod test_i18n_command;
#[path = "test_input_deep2.rs"]
mod test_input_deep2;
#[path = "test_input_deep3.rs"]
mod test_input_deep3;
#[path = "test_input_queue.rs"]
mod test_input_queue;
#[path = "test_input_queue_geometry.rs"]
mod test_input_queue_geometry;
#[path = "test_input_queue_menu_clear_confirm.rs"]
mod test_input_queue_menu_clear_confirm;
#[path = "test_input_queue_menu_clear_gating.rs"]
mod test_input_queue_menu_clear_gating;
#[path = "test_input_queue_menu_integration.rs"]
mod test_input_queue_menu_integration;
#[path = "test_input_queue_menu_render.rs"]
mod test_input_queue_menu_render;
#[path = "test_input_queue_show_panel.rs"]
mod test_input_queue_show_panel;
#[path = "test_keyboard_control.rs"]
mod test_keyboard_control;
#[path = "test_layout_deep.rs"]
mod test_layout_deep;
#[path = "test_layout_extra.rs"]
mod test_layout_extra;
#[path = "test_lib_schedule_deep.rs"]
mod test_lib_schedule_deep;
#[path = "test_mcp_discover_panel.rs"]
mod test_mcp_discover_panel;
#[path = "test_mcp_remove.rs"]
mod test_mcp_remove;
#[path = "test_memory_clear_confirm.rs"]
mod test_memory_clear_confirm;
#[path = "test_memory_panel.rs"]
mod test_memory_panel;
#[path = "test_message_pinned_scroll.rs"]
mod test_message_pinned_scroll;
#[path = "test_message_text_styles.rs"]
mod test_message_text_styles;
#[path = "test_message_widget_duration_tests.rs"]
mod test_message_widget_duration_tests;
#[path = "test_message_widget_more.rs"]
mod test_message_widget_more;
#[path = "test_misc_modules_deep.rs"]
mod test_misc_modules_deep;
#[path = "test_model_picker_widths.rs"]
mod test_model_picker_widths;
#[path = "test_models_state_misc.rs"]
mod test_models_state_misc;
#[path = "test_ms03_mask_secret.rs"]
mod test_ms03_mask_secret;
#[path = "test_output_view_footer.rs"]
mod test_output_view_footer;
#[path = "test_output_view_live_steps.rs"]
mod test_output_view_live_steps;
#[path = "test_panic_hook_thread_scope.rs"]
mod test_panic_hook_thread_scope;
#[path = "test_perf_message_cache.rs"]
mod test_perf_message_cache;
#[path = "test_perf_render_idle.rs"]
mod test_perf_render_idle;
#[path = "test_perf_single_copy_lines.rs"]
mod test_perf_single_copy_lines;
#[path = "test_permission_countdown.rs"]
mod test_permission_countdown;
#[path = "test_plugin_store_category_acceptance.rs"]
mod test_plugin_store_category_acceptance;
#[path = "test_plugin_store_category_mouse.rs"]
mod test_plugin_store_category_mouse;
#[path = "test_plugin_store_failure.rs"]
mod test_plugin_store_failure;
#[path = "test_plugin_store_installed_category.rs"]
mod test_plugin_store_installed_category;
#[path = "test_plugin_store_panel.rs"]
mod test_plugin_store_panel;
#[path = "test_plugins_list_panel.rs"]
mod test_plugins_list_panel;
#[path = "test_probe_t022.rs"]
mod test_probe_t022;
#[path = "test_project_menu.rs"]
mod test_project_menu;
#[path = "test_question_multiple_choice.rs"]
mod test_question_multiple_choice;
#[path = "test_render_cost_scaling.rs"]
mod test_render_cost_scaling;
#[path = "test_research_swarm_deep.rs"]
mod test_research_swarm_deep;
#[path = "test_research_viewer.rs"]
mod test_research_viewer;
#[path = "test_router_setup.rs"]
mod test_router_setup;
#[path = "test_row_action_confirm.rs"]
mod test_row_action_confirm;
#[path = "test_row_action_dispatch.rs"]
mod test_row_action_dispatch;
#[path = "test_row_click_destinations.rs"]
mod test_row_click_destinations;
#[path = "test_selectable_list.rs"]
mod test_selectable_list;
#[path = "test_session_events_deep.rs"]
mod test_session_events_deep;
#[path = "test_session_menu.rs"]
mod test_session_menu;
#[path = "test_session_models_state_deep.rs"]
mod test_session_models_state_deep;
#[path = "test_sessions_sidebar.rs"]
mod test_sessions_sidebar;
#[path = "test_setup_dialog_chrome.rs"]
mod test_setup_dialog_chrome;
#[path = "test_setup_selector_scroll.rs"]
mod test_setup_selector_scroll;
#[path = "test_shorten_middle.rs"]
mod test_shorten_middle;
#[path = "test_slash_commands.rs"]
mod test_slash_commands;
#[path = "test_statusbar_last_prompt.rs"]
mod test_statusbar_last_prompt;
#[path = "test_statusbar_layout.rs"]
mod test_statusbar_layout;
#[path = "test_tasks_panel.rs"]
mod test_tasks_panel;
#[path = "test_teammate_strip.rs"]
mod test_teammate_strip;
#[path = "test_teams_tui.rs"]
mod test_teams_tui;
#[path = "test_teams_window_frame.rs"]
mod test_teams_window_frame;
#[path = "test_teams_window_table.rs"]
mod test_teams_window_table;
#[path = "test_telemetry_slash.rs"]
mod test_telemetry_slash;
#[path = "test_text_selection.rs"]
mod test_text_selection;
#[path = "test_think_narration_scrub.rs"]
mod test_think_narration_scrub;
#[path = "test_widgets_and_events_extra.rs"]
mod test_widgets_and_events_extra;
#[path = "test_window_lifecycle.rs"]
mod test_window_lifecycle;
#[path = "test_window_routing.rs"]
mod test_window_routing;
#[path = "test_window_scroll_wheel.rs"]
mod test_window_scroll_wheel;
