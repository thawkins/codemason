//! The sessions sidebar's request chip and popup
//! (`codemason_gui::panels::sessions`): a session row with pull requests
//! describes them, resting on it opens a popup listing every request newest
//! first with each one's diff size when it is known and an eye on the one
//! Auto-fix is watching, and a click in the
//! popup opens that request's page.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::sessions_sidebar_state::{SidebarRow, build_rows};
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use codemason_storage::SessionRequest;
use craft_keymap::Platform;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// A running window: its runtime, harness and the requested session's title.
struct Rig {
    _runtime: tokio::runtime::Runtime,
    harness: Harness<'static, Shell>,
    title: String,
}

/// Request `number` of session `session`, in `state`.
fn request(session: &str, number: u64, state: &str) -> SessionRequest {
    SessionRequest {
        session_id: session.to_string(),
        forge: "github".to_string(),
        host: "github.com".to_string(),
        repo: "acme/app".to_string(),
        number,
        title: format!("Change {number}"),
        web_url: format!("https://github.com/acme/app/pull/{number}"),
        head_branch: format!("b{number}"),
        state: state.to_string(),
    }
}

/// A window with the sessions sidebar shown and the open session having had
/// requests #1 (merged) and then #2 (open).
fn rig() -> Rig {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.sidebar.visible = true;
    app.sidebar.launch_log = Some(Vec::new());
    runtime.block_on(app.execute_slash_command("/session new"));
    let id = app.session.session_id.clone().expect("a session");
    app.storage
        .set_session_request(&request(&id, 1, "merged"))
        .expect("request 1");
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.storage
        .set_session_request(&request(&id, 2, "open"))
        .expect("request 2");
    // The session is bound to #2 (the last one recorded) with Auto-fix on.
    assert!(app.storage.set_session_autofix(&id, true).expect("autofix"));
    // Only #1's diff size has been read.
    app.storage
        .set_request_diff_stats("github.com", "acme/app", 1, 30, 4)
        .expect("stats");
    app.sidebar.dirty = true;
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(&mut app);
    let title = build_rows(&app)
        .iter()
        .find_map(|row| match row {
            SidebarRow::Session {
                id: row_id, title, ..
            } if *row_id == id => Some(title.clone()),
            _ => None,
        })
        .expect("the session's row");
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(64);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    Rig {
        _runtime: runtime,
        harness,
        title,
    }
}

/// The popup entry for request `number`, if it is shown.
fn entry_label(number: u64) -> String {
    format!("acme/app #{number}: Change {number}")
}

#[test]
fn test_sessions_request_row_describes_its_requests() {
    let rig = rig();
    let row = rig.harness.get_by_label(&rig.title);
    let description = row
        .accesskit_node()
        .description()
        .unwrap_or_default()
        .to_string();
    assert!(
        description.contains("Pull requests: 2 (one bound)"),
        "{description}"
    );
    // Nothing is listed until the pointer rests on the row.
    assert!(rig.harness.query_by_label_contains("acme/app #").is_none());
}

#[test]
fn test_sessions_request_popup_lists_and_opens_requests() {
    let mut rig = rig();
    let row = rig.harness.get_by_label(&rig.title).rect();
    rig.harness.hover_at(row.center());
    // Rest on the row past the tooltip delay.
    rig.harness.run_steps(60);
    let newest = rig.harness.get_by_label_contains(&entry_label(2)).rect();
    let older = rig.harness.get_by_label_contains(&entry_label(1)).rect();
    assert!(newest.top() < older.top(), "newest first");
    assert!(newest.left() >= row.right(), "the popup is beside the row");
    // The diff size shows where it is known, and nothing where it is not.
    rig.harness
        .get_by_label_contains(&format!("{} (merged, 30 added, 4 removed)", entry_label(1)));
    rig.harness
        .get_by_label_contains(&format!("{} (open, watched by Auto-fix)", entry_label(2)));
    let older_label = rig
        .harness
        .get_by_label_contains(&entry_label(1))
        .accesskit_node()
        .label()
        .unwrap_or_default()
        .to_string();
    assert!(!older_label.contains("watched"), "{older_label}");

    // Move onto the popup: it stays open, and a click opens the request.
    rig.harness.hover_at(older.center());
    rig.harness.run();
    rig.harness.get_by_label_contains(&entry_label(1)).click();
    rig.harness.run();
    let log = rig
        .harness
        .state()
        .app()
        .sidebar
        .launch_log
        .clone()
        .expect("log");
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].args, ["https://github.com/acme/app/pull/1"]);

    // Leaving both closes it.
    rig.harness.hover_at(egui::pos2(900.0, 850.0));
    rig.harness.run();
    assert!(rig.harness.query_by_label_contains("acme/app #").is_none());
}

#[test]
fn test_sessions_request_popup_rows_are_material_list_items() {
    use codemason_gui::material::list::TWO_LINE_HEIGHT;
    use codemason_gui::panels::sessions::REQUESTS_LABEL;

    let mut rig = rig();
    let row = rig.harness.get_by_label(&rig.title).rect();
    rig.harness.hover_at(row.center());
    rig.harness.run_steps(60);

    // The popup is a list of two-line Material list items: each request is
    // one row of the labelled list, and its accessible name carries the
    // state, the diff size and the Auto-fix watch the painted parts show.
    let list = rig
        .harness
        .get_by_role_and_label(egui::accesskit::Role::List, REQUESTS_LABEL);
    for number in [1, 2] {
        let name = entry_label(number);
        let item = list.get_by_label_contains(&name);
        assert!(
            (item.rect().height() - TWO_LINE_HEIGHT).abs() < 0.5,
            "a request is a two-line item"
        );
        // The supporting line and the trailing diff size are painted, so the
        // item reports them as its description.
        let description = item
            .accesskit_node()
            .description()
            .unwrap_or_default()
            .to_string();
        assert!(
            description.starts_with(if number == 1 { "Merged" } else { "Open" }),
            "{description}"
        );
        assert!(
            description.contains(&format!("acme/app #{number}")),
            "{description}"
        );
    }
    // The watched request says so, and the older one does not.
    let watched = list
        .get_by_label_contains(&entry_label(2))
        .accesskit_node()
        .label()
        .unwrap_or_default()
        .to_string();
    assert!(watched.contains("watched by Auto-fix"), "{watched}");
    let older = list
        .get_by_label_contains(&entry_label(1))
        .accesskit_node()
        .label()
        .unwrap_or_default()
        .to_string();
    assert!(!older.contains("watched"), "{older}");
    assert!(older.contains("30 added, 4 removed"), "{older}");
}
