//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

/// The binary-wide lock every test that mutates a process-global (env vars)
/// takes. See AGENTS.md ("Test Layout").
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Take [`global_env_mutex`], tolerating poisoning from a failed test.
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "research_sandbox.rs"]
pub mod research_sandbox;
#[path = "support/mod.rs"]
pub mod support;
#[path = "test_automation_surface.rs"]
mod test_automation_surface;
#[path = "test_config_redaction.rs"]
mod test_config_redaction;
#[path = "test_constant_time_eq.rs"]
mod test_constant_time_eq;
#[path = "test_event_to_sse.rs"]
mod test_event_to_sse;
#[path = "test_integration.rs"]
mod test_integration;
#[path = "test_memory_api.rs"]
mod test_memory_api;
#[path = "test_ms04_server_guards.rs"]
mod test_ms04_server_guards;
#[path = "test_ms6_hardening.rs"]
mod test_ms6_hardening;
#[path = "test_openai_surface.rs"]
mod test_openai_surface;
#[path = "test_openapi_document.rs"]
mod test_openapi_document;
#[path = "test_projects_routes.rs"]
mod test_projects_routes;
#[path = "test_research_routes.rs"]
mod test_research_routes;
#[path = "test_routes_handlers.rs"]
mod test_routes_handlers;
#[path = "test_sse_event_variants.rs"]
mod test_sse_event_variants;
#[path = "test_web_ui.rs"]
mod test_web_ui;
