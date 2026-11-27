//! Consolidated integration-test target for `codemason-app-core`.
//!
//! Each `tests/test_*.rs` file is included as a child module via `#[path]`,
//! so it keeps its own file-relative `mod` / `#[path]` resolution and its
//! `test_<file>::<fn>` test name. See `AGENTS.md` ("Test Layout").
//!
//! Most of these files exercise the App through the key and mouse handlers;
//! [`term_events`] supplies the terminal-style event builders they use.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: each `#[path]`-included child file declares its own
// `mod support;` / `mod slash_support;`, so a shared helper file is a module
// of several children. Under one-binary-per-crate that loads it more than
// once, which clippy::duplicate_mod flags; hoisting it would require editing
// every child file. See AGENTS.md ("Test Layout").

#[path = "term_events/mod.rs"]
pub mod term_events;

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

/// Disable, in `storage`, every provider the app would auto-detect from the
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
    for (pid, _) in codemason_app_core::app::PROVIDER_LIST {
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
pub fn without_host_providers(app: &mut codemason_app_core::app::App) {
    disable_host_providers(&app.storage);
    app.configured_provider = codemason_app_core::app::App::detect_provider(&app.storage);
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
        let Ok(dir) = tempfile::tempdir_in(codemason_app_core::app::workspace_scratch_dir()) else {
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

#[path = "test_active_tasks_reconcile.rs"]
mod test_active_tasks_reconcile;
#[path = "test_activity.rs"]
mod test_activity;
#[path = "test_agent_notice_separation.rs"]
mod test_agent_notice_separation;
#[path = "test_agent_switching.rs"]
mod test_agent_switching;
#[path = "test_agents_window_refusals.rs"]
mod test_agents_window_refusals;
#[path = "test_alt_a_agents_window.rs"]
mod test_alt_a_agents_window;
#[path = "test_alt_t_teams_window.rs"]
mod test_alt_t_teams_window;
#[path = "test_app_core_input.rs"]
mod test_app_core_input;
#[path = "test_app_core_moved.rs"]
mod test_app_core_moved;
#[path = "test_app_send_sync.rs"]
mod test_app_send_sync;
#[path = "test_autofix.rs"]
mod test_autofix;
#[path = "test_automation_command.rs"]
mod test_automation_command;
#[path = "test_autopilot_alt_o.rs"]
mod test_autopilot_alt_o;
#[path = "test_azure_resource_flow.rs"]
mod test_azure_resource_flow;
#[path = "test_backend_command.rs"]
mod test_backend_command;
#[path = "test_bench_command.rs"]
mod test_bench_command;
#[path = "test_block_in_place_guard.rs"]
mod test_block_in_place_guard;
#[path = "test_blueprints.rs"]
mod test_blueprints;
#[path = "test_busy_send_guard.rs"]
mod test_busy_send_guard;
#[path = "test_cancel_slash.rs"]
mod test_cancel_slash;
#[path = "test_clipboard.rs"]
mod test_clipboard;
#[path = "test_clipboard_routing.rs"]
mod test_clipboard_routing;
#[path = "test_clipboard_tempfile.rs"]
mod test_clipboard_tempfile;
#[path = "test_codeindex_backward_compat.rs"]
mod test_codeindex_backward_compat;
#[path = "test_codeindex_disabled_guard.rs"]
mod test_codeindex_disabled_guard;
#[path = "test_codeindex_empty_graph_guard.rs"]
mod test_codeindex_empty_graph_guard;
#[path = "test_codeindex_indicators.rs"]
mod test_codeindex_indicators;
#[path = "test_codeindex_skillgen.rs"]
mod test_codeindex_skillgen;
#[path = "test_codeindex_toggle_safety.rs"]
mod test_codeindex_toggle_safety;
#[path = "test_command_catalog.rs"]
mod test_command_catalog;
#[path = "test_compaction_result.rs"]
mod test_compaction_result;
#[path = "test_completer_stream_error.rs"]
mod test_completer_stream_error;
#[path = "test_compression_indicator.rs"]
mod test_compression_indicator;
#[path = "test_config_write.rs"]
mod test_config_write;
#[path = "test_configured_provider_selection.rs"]
mod test_configured_provider_selection;
#[path = "test_connector_catalogue_enrich.rs"]
mod test_connector_catalogue_enrich;
#[path = "test_connector_catalogue_marketplace_warning.rs"]
mod test_connector_catalogue_marketplace_warning;
#[path = "test_connector_catalogue_panel.rs"]
mod test_connector_catalogue_panel;
#[path = "test_connectors_command.rs"]
mod test_connectors_command;
#[path = "test_context_items.rs"]
mod test_context_items;
#[path = "test_context_panel_lines.rs"]
mod test_context_panel_lines;
#[path = "test_context_panel_t004.rs"]
mod test_context_panel_t004;
#[path = "test_context_panel_t005.rs"]
mod test_context_panel_t005;
#[path = "test_context_panel_t006.rs"]
mod test_context_panel_t006;
#[path = "test_context_panel_t007.rs"]
mod test_context_panel_t007;
#[path = "test_context_panel_t008.rs"]
mod test_context_panel_t008;
#[path = "test_context_panel_t009.rs"]
mod test_context_panel_t009;
#[path = "test_context_panel_t013.rs"]
mod test_context_panel_t013;
#[path = "test_context_panel_t015.rs"]
mod test_context_panel_t015;
#[path = "test_cron_add_positional.rs"]
mod test_cron_add_positional;
#[path = "test_db_command.rs"]
mod test_db_command;
#[path = "test_diff_view.rs"]
mod test_diff_view;
#[path = "test_dirs_show.rs"]
mod test_dirs_show;
#[path = "test_dispatch_ops_deep.rs"]
mod test_dispatch_ops_deep;
#[path = "test_esc_stops_agent.rs"]
mod test_esc_stops_agent;
#[path = "test_event_handler_more.rs"]
mod test_event_handler_more;
#[path = "test_event_swarm_research_deep.rs"]
mod test_event_swarm_research_deep;
#[path = "test_external_editor.rs"]
mod test_external_editor;
#[path = "test_force_cleanup_modal.rs"]
mod test_force_cleanup_modal;
#[path = "test_fr030_surface_agreement.rs"]
mod test_fr030_surface_agreement;
#[path = "test_fr033_completion_surfaces.rs"]
mod test_fr033_completion_surfaces;
#[path = "test_fuzzy.rs"]
mod test_fuzzy;
#[path = "test_gcf_command.rs"]
mod test_gcf_command;
#[path = "test_gcf_indicator.rs"]
mod test_gcf_indicator;
#[path = "test_gcf_slash_registration.rs"]
mod test_gcf_slash_registration;
#[path = "test_geometry.rs"]
mod test_geometry;
#[path = "test_gitlab_setup_unicode.rs"]
mod test_gitlab_setup_unicode;
#[path = "test_govcreate_dispatch.rs"]
mod test_govcreate_dispatch;
#[path = "test_govcreate_poll.rs"]
mod test_govcreate_poll;
#[path = "test_govcreate_registration.rs"]
mod test_govcreate_registration;
#[path = "test_hard_break_lines.rs"]
mod test_hard_break_lines;
#[path = "test_history.rs"]
mod test_history;
#[path = "test_hook_warning_flagged.rs"]
mod test_hook_warning_flagged;
#[path = "test_housekeeping.rs"]
mod test_housekeeping;
#[path = "test_i18n_command.rs"]
mod test_i18n_command;
#[path = "test_init_config.rs"]
mod test_init_config;
#[path = "test_input_deep2.rs"]
mod test_input_deep2;
#[path = "test_input_deep3.rs"]
mod test_input_deep3;
#[path = "test_input_event.rs"]
mod test_input_event;
#[path = "test_input_field_extra.rs"]
mod test_input_field_extra;
#[path = "test_input_handler_more.rs"]
mod test_input_handler_more;
#[path = "test_input_more.rs"]
mod test_input_more;
#[path = "test_input_queue.rs"]
mod test_input_queue;
#[path = "test_input_queue_command.rs"]
mod test_input_queue_command;
#[path = "test_input_queue_drain.rs"]
mod test_input_queue_drain;
#[path = "test_input_queue_enqueue.rs"]
mod test_input_queue_enqueue;
#[path = "test_input_queue_error_drain.rs"]
mod test_input_queue_error_drain;
#[path = "test_input_queue_geometry.rs"]
mod test_input_queue_geometry;
#[path = "test_input_queue_guard.rs"]
mod test_input_queue_guard;
#[path = "test_input_queue_menu_binding.rs"]
mod test_input_queue_menu_binding;
#[path = "test_input_queue_menu_clear.rs"]
mod test_input_queue_menu_clear;
#[path = "test_input_queue_menu_clear_confirm.rs"]
mod test_input_queue_menu_clear_confirm;
#[path = "test_input_queue_menu_clear_gating.rs"]
mod test_input_queue_menu_clear_gating;
#[path = "test_input_queue_menu_esc.rs"]
mod test_input_queue_menu_esc;
#[path = "test_input_queue_menu_integration.rs"]
mod test_input_queue_menu_integration;
#[path = "test_input_queue_menu_navigation.rs"]
mod test_input_queue_menu_navigation;
#[path = "test_input_queue_menu_next.rs"]
mod test_input_queue_menu_next;
#[path = "test_input_queue_menu_render.rs"]
mod test_input_queue_menu_render;
#[path = "test_input_queue_menu_stop_resume.rs"]
mod test_input_queue_menu_stop_resume;
#[path = "test_input_queue_show_panel.rs"]
mod test_input_queue_show_panel;
#[path = "test_input_queue_slash.rs"]
mod test_input_queue_slash;
#[path = "test_input_transition_spool.rs"]
mod test_input_transition_spool;
#[path = "test_input_unicode.rs"]
mod test_input_unicode;
#[path = "test_keyboard_control.rs"]
mod test_keyboard_control;
#[path = "test_layout_extra.rs"]
mod test_layout_extra;
#[path = "test_log_window_otel_export.rs"]
mod test_log_window_otel_export;
#[path = "test_loop_dialog.rs"]
mod test_loop_dialog;
#[path = "test_markdown.rs"]
mod test_markdown;
#[path = "test_markdown_images.rs"]
mod test_markdown_images;
#[path = "test_markdown_table.rs"]
mod test_markdown_table;
#[path = "test_markdown_table_rows.rs"]
mod test_markdown_table_rows;
#[path = "test_mcp_bridged_surfaces.rs"]
mod test_mcp_bridged_surfaces;
#[path = "test_mcp_discover_panel.rs"]
mod test_mcp_discover_panel;
#[path = "test_mcp_remove.rs"]
mod test_mcp_remove;
#[path = "test_memory_clear_confirm.rs"]
mod test_memory_clear_confirm;
#[path = "test_memory_panel.rs"]
mod test_memory_panel;
#[path = "test_mentions.rs"]
mod test_mentions;
#[path = "test_message_blocks.rs"]
mod test_message_blocks;
#[path = "test_message_edit_seq_cache.rs"]
mod test_message_edit_seq_cache;
#[path = "test_message_widget_duration_tests.rs"]
mod test_message_widget_duration_tests;
#[path = "test_message_widget_more.rs"]
mod test_message_widget_more;
#[path = "test_message_widget_summaries.rs"]
mod test_message_widget_summaries;
#[path = "test_message_widget_summaries2.rs"]
mod test_message_widget_summaries2;
#[path = "test_message_widget_tests.rs"]
mod test_message_widget_tests;
#[path = "test_misc_modules_deep.rs"]
mod test_misc_modules_deep;
#[path = "test_model_favorites.rs"]
mod test_model_favorites;
#[path = "test_models_state_misc.rs"]
mod test_models_state_misc;
#[path = "test_modules_arms_deep.rs"]
mod test_modules_arms_deep;
#[path = "test_ms03_mask_secret.rs"]
mod test_ms03_mask_secret;
#[path = "test_ms04_tui_source_guards.rs"]
mod test_ms04_tui_source_guards;
#[path = "test_multisession.rs"]
mod test_multisession;
#[path = "test_new_command.rs"]
mod test_new_command;
#[path = "test_open_in.rs"]
mod test_open_in;
#[path = "test_osinfo_command.rs"]
mod test_osinfo_command;
#[path = "test_osinfo_registration.rs"]
mod test_osinfo_registration;
#[path = "test_output_view_footer.rs"]
mod test_output_view_footer;
#[path = "test_output_view_live_steps.rs"]
mod test_output_view_live_steps;
#[path = "test_palette_command.rs"]
mod test_palette_command;
#[path = "test_perf_render_idle.rs"]
mod test_perf_render_idle;
#[path = "test_permission_answer_guard.rs"]
mod test_permission_answer_guard;
#[path = "test_permission_countdown.rs"]
mod test_permission_countdown;
#[path = "test_permission_preview.rs"]
mod test_permission_preview;
#[path = "test_picker_empty_guard.rs"]
mod test_picker_empty_guard;
#[path = "test_plugin_commands.rs"]
mod test_plugin_commands;
#[path = "test_plugin_store_browser.rs"]
mod test_plugin_store_browser;
#[path = "test_plugin_store_category_acceptance.rs"]
mod test_plugin_store_category_acceptance;
#[path = "test_plugin_store_category_filter.rs"]
mod test_plugin_store_category_filter;
#[path = "test_plugin_store_category_launch.rs"]
mod test_plugin_store_category_launch;
#[path = "test_plugin_store_category_nav.rs"]
mod test_plugin_store_category_nav;
#[path = "test_plugin_store_default_behavior.rs"]
mod test_plugin_store_default_behavior;
#[path = "test_plugin_store_enrich.rs"]
mod test_plugin_store_enrich;
#[path = "test_plugin_store_failure.rs"]
mod test_plugin_store_failure;
#[path = "test_plugin_store_fetch.rs"]
mod test_plugin_store_fetch;
#[path = "test_plugin_store_filter.rs"]
mod test_plugin_store_filter;
#[path = "test_plugin_store_install.rs"]
mod test_plugin_store_install;
#[path = "test_plugin_store_installed_category.rs"]
mod test_plugin_store_installed_category;
#[path = "test_plugin_store_launch.rs"]
mod test_plugin_store_launch;
#[path = "test_plugin_store_marketplaces.rs"]
mod test_plugin_store_marketplaces;
#[path = "test_plugin_store_probe.rs"]
mod test_plugin_store_probe;
#[path = "test_plugin_store_seam.rs"]
mod test_plugin_store_seam;
#[path = "test_plugin_store_state.rs"]
mod test_plugin_store_state;
#[path = "test_plugin_store_stores_report.rs"]
mod test_plugin_store_stores_report;
#[path = "test_plugins_command.rs"]
mod test_plugins_command;
#[path = "test_plugins_list_panel.rs"]
mod test_plugins_list_panel;
#[path = "test_project_base_model.rs"]
mod test_project_base_model;
#[path = "test_project_command.rs"]
mod test_project_command;
#[path = "test_project_menu.rs"]
mod test_project_menu;
#[path = "test_project_services.rs"]
mod test_project_services;
#[path = "test_prompt_readonly.rs"]
mod test_prompt_readonly;
#[path = "test_prompt_size_cap.rs"]
mod test_prompt_size_cap;
#[path = "test_prompt_subagent_render.rs"]
mod test_prompt_subagent_render;
#[path = "test_prompt_tool_free.rs"]
mod test_prompt_tool_free;
#[path = "test_prompt_units.rs"]
mod test_prompt_units;
#[path = "test_provider_detection.rs"]
mod test_provider_detection;
#[path = "test_question_answer_guard.rs"]
mod test_question_answer_guard;
#[path = "test_quit_force_exit_hook.rs"]
mod test_quit_force_exit_hook;
#[path = "test_remove_pending_attachment.rs"]
mod test_remove_pending_attachment;
#[path = "test_render_cost_scaling.rs"]
mod test_render_cost_scaling;
#[path = "test_research_app_more.rs"]
mod test_research_app_more;
#[path = "test_research_clarification.rs"]
mod test_research_clarification;
#[path = "test_research_cluster.rs"]
mod test_research_cluster;
#[path = "test_research_deck.rs"]
mod test_research_deck;
#[path = "test_research_deck_progress_message.rs"]
mod test_research_deck_progress_message;
#[path = "test_research_progress_config.rs"]
mod test_research_progress_config;
#[path = "test_research_progress_extra.rs"]
mod test_research_progress_extra;
#[path = "test_research_progress_sanitize.rs"]
mod test_research_progress_sanitize;
#[path = "test_research_run_lifecycle.rs"]
mod test_research_run_lifecycle;
#[path = "test_research_swarm_deep.rs"]
mod test_research_swarm_deep;
#[path = "test_research_viewer.rs"]
mod test_research_viewer;
#[path = "test_rollback_flow.rs"]
mod test_rollback_flow;
#[path = "test_router_save_dialog.rs"]
mod test_router_save_dialog;
#[path = "test_router_setup.rs"]
mod test_router_setup;
#[path = "test_row_action_confirm.rs"]
mod test_row_action_confirm;
#[path = "test_row_action_dispatch.rs"]
mod test_row_action_dispatch;
#[path = "test_row_click_destinations.rs"]
mod test_row_click_destinations;
#[path = "test_run_cost_banner.rs"]
mod test_run_cost_banner;
#[path = "test_scrolling.rs"]
mod test_scrolling;
#[path = "test_security_command.rs"]
mod test_security_command;
#[path = "test_session_events_deep.rs"]
mod test_session_events_deep;
#[path = "test_session_menu.rs"]
mod test_session_menu;
#[path = "test_session_models_state_deep.rs"]
mod test_session_models_state_deep;
#[path = "test_session_ops_more.rs"]
mod test_session_ops_more;
#[path = "test_session_resume.rs"]
mod test_session_resume;
#[path = "test_sessions_sidebar.rs"]
mod test_sessions_sidebar;
#[path = "test_setup_dialog_chrome.rs"]
mod test_setup_dialog_chrome;
#[path = "test_shortcuts_catalog.rs"]
mod test_shortcuts_catalog;
#[path = "test_skill_command.rs"]
mod test_skill_command;
#[path = "test_slash_advertised_forms.rs"]
mod test_slash_advertised_forms;
#[path = "test_slash_arms_deep.rs"]
mod test_slash_arms_deep;
#[path = "test_slash_codeindex.rs"]
mod test_slash_codeindex;
#[path = "test_slash_codeindex_deep.rs"]
mod test_slash_codeindex_deep;
#[path = "test_slash_commands.rs"]
mod test_slash_commands;
#[path = "test_slash_compact_prune.rs"]
mod test_slash_compact_prune;
#[path = "test_slash_config_write.rs"]
mod test_slash_config_write;
#[path = "test_slash_deep2.rs"]
mod test_slash_deep2;
#[path = "test_slash_heavy_deep.rs"]
mod test_slash_heavy_deep;
#[path = "test_slash_help.rs"]
mod test_slash_help;
#[path = "test_slash_info.rs"]
mod test_slash_info;
#[path = "test_slash_menu_escape.rs"]
mod test_slash_menu_escape;
#[path = "test_slash_misc.rs"]
mod test_slash_misc;
#[path = "test_slash_misc_deep2.rs"]
mod test_slash_misc_deep2;
#[path = "test_slash_router.rs"]
mod test_slash_router;
#[path = "test_slash_session.rs"]
mod test_slash_session;
#[path = "test_slash_spec.rs"]
mod test_slash_spec;
#[path = "test_slash_spec_deep.rs"]
mod test_slash_spec_deep;
#[path = "test_slash_spec_deep2.rs"]
mod test_slash_spec_deep2;
#[path = "test_slash_teams_deep.rs"]
mod test_slash_teams_deep;
#[path = "test_slash_tools.rs"]
mod test_slash_tools;
#[path = "test_spec_jtbd.rs"]
mod test_spec_jtbd;
#[path = "test_startup_services.rs"]
mod test_startup_services;
#[path = "test_startup_session.rs"]
mod test_startup_session;
#[path = "test_stats_command.rs"]
mod test_stats_command;
#[path = "test_status_expiry.rs"]
mod test_status_expiry;
#[path = "test_statusbar_last_prompt.rs"]
mod test_statusbar_last_prompt;
#[path = "test_store_cache.rs"]
mod test_store_cache;
#[path = "test_store_cache_fetch.rs"]
mod test_store_cache_fetch;
#[path = "test_store_cache_marketplace.rs"]
mod test_store_cache_marketplace;
#[path = "test_store_cache_open.rs"]
mod test_store_cache_open;
#[path = "test_store_uninstall.rs"]
mod test_store_uninstall;
#[path = "test_stream_metrics.rs"]
mod test_stream_metrics;
#[path = "test_styled.rs"]
mod test_styled;
#[path = "test_subagent_events.rs"]
mod test_subagent_events;
#[path = "test_subagent_lifecycle_review.rs"]
mod test_subagent_lifecycle_review;
#[path = "test_subagent_step_log.rs"]
mod test_subagent_step_log;
#[path = "test_svg_attachments.rs"]
mod test_svg_attachments;
#[path = "test_swarm_more.rs"]
mod test_swarm_more;
#[path = "test_swarm_offload.rs"]
mod test_swarm_offload;
#[path = "test_swarm_result_handoff.rs"]
mod test_swarm_result_handoff;
#[path = "test_swarm_synthesis.rs"]
mod test_swarm_synthesis;
#[path = "test_syntax_highlight.rs"]
mod test_syntax_highlight;
#[path = "test_tasks_panel.rs"]
mod test_tasks_panel;
#[path = "test_teams_message_counts.rs"]
mod test_teams_message_counts;
#[path = "test_teams_multi.rs"]
mod test_teams_multi;
#[path = "test_teams_tui.rs"]
mod test_teams_tui;
#[path = "test_teams_window_table.rs"]
mod test_teams_window_table;
#[path = "test_telemetry_export_notice.rs"]
mod test_telemetry_export_notice;
#[path = "test_telemetry_slash.rs"]
mod test_telemetry_slash;
#[path = "test_text_selection.rs"]
mod test_text_selection;
#[path = "test_theme_command.rs"]
mod test_theme_command;
#[path = "test_theme_editor.rs"]
mod test_theme_editor;
#[path = "test_theme_save.rs"]
mod test_theme_save;
#[path = "test_theme_tokens.rs"]
mod test_theme_tokens;
#[path = "test_think_narration_scrub.rs"]
mod test_think_narration_scrub;
#[path = "test_thinking_defaults.rs"]
mod test_thinking_defaults;
#[path = "test_thinking_selector.rs"]
mod test_thinking_selector;
#[path = "test_tool_display.rs"]
mod test_tool_display;
#[path = "test_tool_event_state.rs"]
mod test_tool_event_state;
#[path = "test_transcript_context_menu.rs"]
mod test_transcript_context_menu;
#[path = "test_transcript_prefs.rs"]
mod test_transcript_prefs;
#[path = "test_undo_redo.rs"]
mod test_undo_redo;
#[path = "test_user_commands.rs"]
mod test_user_commands;
#[path = "test_widgets_and_events_extra.rs"]
mod test_widgets_and_events_extra;
#[path = "test_window_lifecycle.rs"]
mod test_window_lifecycle;
#[path = "test_window_routing.rs"]
mod test_window_routing;
#[path = "test_window_scroll_wheel.rs"]
mod test_window_scroll_wheel;

/// A scratch config file that receives config writes (such as the theme
/// `/theme` saves) for as long as the guard lives, so a test never edits the
/// developer's own config.
///
/// Holds the binary-wide `global_env_lock` for its lifetime, because the write
/// target is a process-global; dropping it restores the previous target and
/// deletes the scratch directory. A test that takes this guard must not take
/// `global_env_lock()` itself.
pub struct ScratchConfig {
    path: std::path::PathBuf,
    previous: Option<std::path::PathBuf>,
    _dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl ScratchConfig {
    /// The scratch `codemason.json`; it does not exist until something is
    /// written.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for ScratchConfig {
    fn drop(&mut self) {
        codemason_config::Config::set_config_write_target(self.previous.take());
    }
}

/// A fresh, git-ignored scratch directory under `target/temp` with `prefix` in
/// its name.
///
/// Every test that needs a directory of its own uses this rather than a
/// hard-coded path, so two tests never share one and a run leaves the
/// repository byte-identical.
pub fn scratch_tempdir(prefix: &str) -> tempfile::TempDir {
    let root = codemason_app_core::app::workspace_scratch_dir();
    std::fs::create_dir_all(&root).expect("scratch root");
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Points config writes at a fresh `codemason.json` under `target/temp` until
/// the returned guard is dropped.
pub fn scratch_config_target() -> ScratchConfig {
    let lock = global_env_lock();
    let dir = scratch_tempdir("config-writes-");
    let path = dir.path().join("codemason.json");
    let previous = codemason_config::Config::config_write_target();
    codemason_config::Config::set_config_write_target(Some(path.clone()));
    ScratchConfig {
        path,
        previous,
        _dir: dir,
        _lock: lock,
    }
}
