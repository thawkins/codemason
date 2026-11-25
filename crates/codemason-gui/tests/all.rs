//! Consolidated integration-test target for `codemason-gui`.
//!
//! Every `tests/test_*.rs` file is a child module of this one binary (see
//! AGENTS-LANGUAGE.md, "Test Organization"). The binary-wide environment lock
//! lives here so every test that touches a process-global takes the same one.

// Each child file declares `#[path = "support/mod.rs"] mod support;`, so the
// helper is loaded once per child (AGENTS-LANGUAGE.md, "Test Organization").
#![allow(clippy::duplicate_mod)]

use std::sync::{Mutex, MutexGuard, OnceLock};

#[path = "../../codemason-app-core/tests/term_events/mod.rs"]
pub mod term_events;

#[path = "frame/mod.rs"]
pub mod frame;

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

#[path = "test_activity_rows.rs"]
mod test_activity_rows;
#[path = "test_bridge.rs"]
mod test_bridge;
#[path = "test_clipboard.rs"]
mod test_clipboard;
#[path = "test_command_palette.rs"]
mod test_command_palette;
#[path = "test_dialog_bounds_gui.rs"]
mod test_dialog_bounds_gui;
#[path = "test_dialogs.rs"]
mod test_dialogs;
#[path = "test_dialogs_more.rs"]
mod test_dialogs_more;
#[path = "test_diff_pane.rs"]
mod test_diff_pane;
#[path = "test_dock.rs"]
mod test_dock;
#[path = "test_document_panes.rs"]
mod test_document_panes;
#[path = "test_external_edit.rs"]
mod test_external_edit;
#[path = "test_files.rs"]
mod test_files;
#[path = "test_fonts.rs"]
mod test_fonts;
#[path = "test_housekeeping.rs"]
mod test_housekeeping;
#[path = "test_image_load.rs"]
mod test_image_load;
#[path = "test_keys.rs"]
mod test_keys;
#[path = "test_launch.rs"]
mod test_launch;
#[path = "test_main_thread.rs"]
mod test_main_thread;
#[path = "test_markdown.rs"]
mod test_markdown;
#[path = "test_markdown_table_border.rs"]
mod test_markdown_table_border;
#[path = "test_material_avatar_item.rs"]
mod test_material_avatar_item;
#[path = "test_material_badge.rs"]
mod test_material_badge;
#[path = "test_material_button.rs"]
mod test_material_button;
#[path = "test_material_card.rs"]
mod test_material_card;
#[path = "test_material_chip.rs"]
mod test_material_chip;
#[path = "test_material_dialog.rs"]
mod test_material_dialog;
#[path = "test_material_drawer_count.rs"]
mod test_material_drawer_count;
#[path = "test_material_feedback.rs"]
mod test_material_feedback;
#[path = "test_material_icon_button.rs"]
mod test_material_icon_button;
#[path = "test_material_list.rs"]
mod test_material_list;
#[path = "test_material_menu.rs"]
mod test_material_menu;
#[path = "test_material_segmented.rs"]
mod test_material_segmented;
#[path = "test_material_sheet.rs"]
mod test_material_sheet;
#[path = "test_material_tabs.rs"]
mod test_material_tabs;
#[path = "test_material_text_field.rs"]
mod test_material_text_field;
#[path = "test_material_thumbnail.rs"]
mod test_material_thumbnail;
#[path = "test_menu.rs"]
mod test_menu;
#[path = "test_mermaid_gui.rs"]
mod test_mermaid_gui;
#[path = "test_messages.rs"]
mod test_messages;
#[path = "test_overlays.rs"]
mod test_overlays;
#[path = "test_palette.rs"]
mod test_palette;
#[path = "test_panels.rs"]
mod test_panels;
#[path = "test_prompt.rs"]
mod test_prompt;
#[path = "test_provider_dialog_masking_gui.rs"]
mod test_provider_dialog_masking_gui;
#[path = "test_research.rs"]
mod test_research;
#[path = "test_runtime_context.rs"]
mod test_runtime_context;
#[path = "test_sessions_request_popup.rs"]
mod test_sessions_request_popup;
#[path = "test_side_panels.rs"]
mod test_side_panels;
#[path = "test_startup.rs"]
mod test_startup;
#[path = "test_status_bar.rs"]
mod test_status_bar;
#[path = "test_store_dialog_gui.rs"]
mod test_store_dialog_gui;
#[path = "test_theme.rs"]
mod test_theme;
#[path = "test_theme_editor_dialog.rs"]
mod test_theme_editor_dialog;
#[path = "test_window.rs"]
mod test_window;

/// The one mutex that serialises every test mutating a process-global
/// (environment variables, cwd) in this binary.
pub fn global_env_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering the guard if a previous holder
/// panicked (a poisoned lock still serialises correctly).
pub fn global_env_lock() -> MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

#[path = "test_about_gui.rs"]
mod test_about_gui;
#[path = "test_agents_teams_cursor_gui.rs"]
mod test_agents_teams_cursor_gui;
#[path = "test_agents_window_table_gui.rs"]
mod test_agents_window_table_gui;
#[path = "test_agentui_t019_offline_gui.rs"]
mod test_agentui_t019_offline_gui;
#[path = "test_alt_a_agents_window_gui.rs"]
mod test_alt_a_agents_window_gui;
#[path = "test_alt_t_teams_window_gui.rs"]
mod test_alt_t_teams_window_gui;
#[path = "test_connector_catalogue_mouse_gui.rs"]
mod test_connector_catalogue_mouse_gui;
#[path = "test_connector_catalogue_panel_gui.rs"]
mod test_connector_catalogue_panel_gui;
#[path = "test_connector_dialog_gui.rs"]
mod test_connector_dialog_gui;
#[path = "test_context_panel_t002_gui.rs"]
mod test_context_panel_t002_gui;
#[path = "test_context_panel_t010_gui.rs"]
mod test_context_panel_t010_gui;
#[path = "test_context_panel_t012_gui.rs"]
mod test_context_panel_t012_gui;
#[path = "test_context_panel_t016_gui.rs"]
mod test_context_panel_t016_gui;
#[path = "test_event_swarm_research_deep_gui.rs"]
mod test_event_swarm_research_deep_gui;
#[path = "test_gcf_indicator_gui.rs"]
mod test_gcf_indicator_gui;
#[path = "test_hit_rect_lifecycle_gui.rs"]
mod test_hit_rect_lifecycle_gui;
#[path = "test_i18n_command_gui.rs"]
mod test_i18n_command_gui;
#[path = "test_input_queue_geometry_gui.rs"]
mod test_input_queue_geometry_gui;
#[path = "test_input_queue_menu_render_gui.rs"]
mod test_input_queue_menu_render_gui;
#[path = "test_keyboard_control_gui.rs"]
mod test_keyboard_control_gui;
#[path = "test_layout_extra_gui.rs"]
mod test_layout_extra_gui;
#[path = "test_memory_clear_confirm_gui.rs"]
mod test_memory_clear_confirm_gui;
#[path = "test_memory_panel_gui.rs"]
mod test_memory_panel_gui;
#[path = "test_misc_modules_deep_gui.rs"]
mod test_misc_modules_deep_gui;
#[path = "test_output_view_footer_gui.rs"]
mod test_output_view_footer_gui;
#[path = "test_plugin_store_category_acceptance_gui.rs"]
mod test_plugin_store_category_acceptance_gui;
#[path = "test_plugin_store_category_mouse_gui.rs"]
mod test_plugin_store_category_mouse_gui;
#[path = "test_plugin_store_failure_gui.rs"]
mod test_plugin_store_failure_gui;
#[path = "test_plugins_list_panel_gui.rs"]
mod test_plugins_list_panel_gui;
#[path = "test_project_menu_gui.rs"]
mod test_project_menu_gui;
#[path = "test_render_cost_scaling_gui.rs"]
mod test_render_cost_scaling_gui;
#[path = "test_ribbon.rs"]
mod test_ribbon;
#[path = "test_router_dialog_material_gui.rs"]
mod test_router_dialog_material_gui;
#[path = "test_router_setup_gui.rs"]
mod test_router_setup_gui;
#[path = "test_row_action_confirm_gui.rs"]
mod test_row_action_confirm_gui;
#[path = "test_row_action_dispatch_gui.rs"]
mod test_row_action_dispatch_gui;
#[path = "test_row_click_destinations_gui.rs"]
mod test_row_click_destinations_gui;
#[path = "test_session_menu_gui.rs"]
mod test_session_menu_gui;
#[path = "test_session_models_state_deep_gui.rs"]
mod test_session_models_state_deep_gui;
#[path = "test_sessions_sidebar_gui.rs"]
mod test_sessions_sidebar_gui;
#[path = "test_setup_selector_scroll_gui.rs"]
mod test_setup_selector_scroll_gui;
#[path = "test_slash_commands_gui.rs"]
mod test_slash_commands_gui;
#[path = "test_swarm_gui.rs"]
mod test_swarm_gui;
#[path = "test_tasks_panel_gui.rs"]
mod test_tasks_panel_gui;
#[path = "test_teams_tui_gui.rs"]
mod test_teams_tui_gui;
#[path = "test_teams_window_frame_gui.rs"]
mod test_teams_window_frame_gui;
#[path = "test_text_selection_gui.rs"]
mod test_text_selection_gui;
#[path = "test_title_bar.rs"]
mod test_title_bar;
#[path = "test_window_lifecycle_gui.rs"]
mod test_window_lifecycle_gui;
#[path = "test_window_routing_gui.rs"]
mod test_window_routing_gui;
#[path = "test_window_scroll_wheel_gui.rs"]
mod test_window_scroll_wheel_gui;
