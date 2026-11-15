//! Startup session selection and provider wiring (`App::open_startup_session`,
//! `App::attach_provider_storage`), the frontend-neutral half of the startup
//! the removed terminal UI's `run_tui` performed and the GUI now calls.
//!
//! - `Resume(id)` focuses the stored session `id` (`cdmn session resume`).
//! - `New` always starts a fresh session (`cdmn --new`), even when an
//!   open-session list was saved.
//! - `Restore` reopens the saved open sessions, and starts a fresh one when
//!   none can be reopened.
//! - A freshly created startup session is mapped to the agent name for log
//!   display.
//! - The router provider gets the session storage attached.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::{App, StartupSession};

/// The last eight characters of `id`: the short id the log display keys on.
fn tail8(id: &str) -> String {
    id[id.len().saturating_sub(8)..].to_string()
}

/// Store a session row the App has not focused, returning its id.
fn stored_session(app: &App) -> String {
    let dir = std::env::current_dir().expect("cwd");
    app.session_processor
        .session_manager
        .create_session(dir)
        .expect("create session")
        .id
}

/// Focus a stored session and save the open-session list, as a previous
/// launch would have left it, then return a fresh App over the same state.
fn app_with_saved_open_session(runtime: &tokio::runtime::Runtime) -> (App, String) {
    let _enter = runtime.enter();
    let mut app = support::make_app();
    let id = stored_session(&app);
    app.load_session(&id).expect("focus stored session");
    app.persist_open_sessions();
    // Forget the focus so only the saved list says which session was open.
    app.session.session_id = None;
    (app, id)
}

#[test]
fn test_startup_resume_focuses_the_requested_session() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = runtime.enter();
    let mut app = support::make_app();
    let other = stored_session(&app);
    let wanted = stored_session(&app);
    assert_ne!(other, wanted);

    app.open_startup_session(&StartupSession::Resume(wanted.clone()));

    assert_eq!(app.session.session_id.as_deref(), Some(wanted.as_str()));
}

#[test]
fn test_startup_new_starts_a_fresh_session_despite_a_saved_list() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (mut app, saved) = app_with_saved_open_session(&runtime);
    let _enter = runtime.enter();

    app.open_startup_session(&StartupSession::New);

    let focused = app
        .session
        .session_id
        .clone()
        .expect("a session is focused");
    assert_ne!(focused, saved, "--new must not reopen the saved session");
    assert!(
        app.storage.get_session(&focused).expect("read").is_some(),
        "the fresh session is stored"
    );
}

#[test]
fn test_startup_restore_reopens_the_saved_session() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let (mut app, saved) = app_with_saved_open_session(&runtime);
    let _enter = runtime.enter();

    app.open_startup_session(&StartupSession::Restore);

    assert_eq!(app.session.session_id.as_deref(), Some(saved.as_str()));
}

#[test]
fn test_startup_restore_without_a_saved_list_starts_a_fresh_session() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = runtime.enter();
    let mut app = support::make_app();

    app.open_startup_session(&StartupSession::Restore);

    assert!(
        app.session.session_id.is_some(),
        "a fresh session is focused"
    );
}

#[test]
fn test_startup_fresh_session_registers_the_primary_mapping() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = runtime.enter();
    let mut app = support::make_app();

    app.open_startup_session(&StartupSession::New);

    let id = app.session.session_id.clone().expect("session");
    assert_eq!(
        app.sid_to_display_name.get(&tail8(&id)),
        Some(&app.session.agent_name),
        "the primary session maps to the agent name for log display"
    );
}

#[test]
fn test_startup_attaches_storage_to_the_router_provider() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = runtime.enter();
    let app = support::make_app();
    let router = || {
        app.provider_registry
            .get_as_any("router")
            .and_then(|p| p.downcast_ref::<codemason_llm::providers::router::RouterProvider>())
            .expect("router provider is registered")
    };
    assert!(router().storage().is_none(), "nothing attached yet");

    app.attach_provider_storage();

    assert!(
        router().storage().is_some(),
        "the router can resolve stored keys"
    );
}
