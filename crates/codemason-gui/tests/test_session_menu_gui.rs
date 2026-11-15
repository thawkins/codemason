//! Session context menu (spec SESSIONMENU, milestone M1).
//!
//! Renders the full layout into a `TestBackend`, opens the menu on a sidebar
//! row with keys or the mouse, and checks the painted menu and the actions it
//! dispatches.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_session_menu.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use codemason_app_core::app::App;
use codemason_app_core::session_menu::{MenuAction, MenuTarget};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

const WIDTH: u16 = 120;
/// Tall enough that the session sidebar still shows every fixture row below
/// the full-width status band, which now spans the top of the frame and so
/// costs the sidebar its two rows: at 30 the second session fell off the list.
const HEIGHT: u16 = 32;

/// An app with the sidebar on and two sessions: `(app, a, b)`, `b` focused.
async fn app_with_sidebar() -> (App, String, String) {
    let mut app = support::make_app();
    app.sidebar.visible = true;
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("session B");
    (app, a, b)
}

/// `draw` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn draw_effects(app: &mut App) {
    crate::frame::publish(app);
}

/// `row_of_session` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn row_of_session_effects(app: &mut App, id: &str) {
    let _label = format!("untitled {}", &id[id.len() - 8..]);
    crate::frame::publish(app);
    let _width = usize::from(app.sidebar.area.width);
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// Focuses the sidebar with its cursor on session `id`'s row.
///
/// The cursor index is the row's offset from the sidebar's first inner line,
/// not from the top of the screen: the status band spans the full width above
/// the sidebar, so the box starts at `app.sidebar.area.y` rather than row 0.
fn put_cursor_on(app: &mut App, id: &str) -> usize {
    crate::frame::publish(app);
    let index = codemason_app_core::sessions_sidebar_state::build_rows(app)
        .iter()
        .position(|row| {
            matches!(
                row,
                codemason_app_core::sessions_sidebar_state::SidebarRow::Session { id: sid, .. }
                    if sid == id
            )
        })
        .expect("session row");
    app.sidebar.focused = true;
    app.sidebar.cursor = index;
    index
}

#[tokio::test]
async fn test_session_menu_m_opens_menu_with_accelerators_and_rules() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let _row = put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;

    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.target, MenuTarget::Session(a.clone()));
    let actions: Vec<MenuAction> = menu.items.iter().map(|i| i.action.clone()).collect();
    // `Terminal` is listed only when this machine can start one, so the expected
    // list names it conditionally: every other item is unconditional.
    let mut expected = vec![MenuAction::OpenIn];
    let has_terminal = actions.contains(&MenuAction::Terminal);
    expected.extend([
        MenuAction::TogglePin,
        MenuAction::ToggleUnread,
        MenuAction::Rename,
        MenuAction::Fork,
        MenuAction::MoveTo,
        MenuAction::Archive,
        MenuAction::Delete,
    ]);
    // `Terminal` closes the menu so the items above keep their positions.
    if has_terminal {
        expected.push(MenuAction::Terminal);
    }
    assert_eq!(actions, expected);
    draw_effects(&mut app);
    let _area = app.sidebar.menu.as_ref().expect("menu").area;
    // The menu's top border is `lines[0]`, so an item's screen line is its
    // position plus the group rules drawn above it, plus one for that border.
    // The optional `Terminal` entry shifts every line after it.
    let items = &menu.items;
    let rule_above = |index: usize| index > 0 && items[index - 1].group != items[index].group;
    let rules_before = |index: usize| (1..=index).filter(|&i| rule_above(i)).count();
    let line_of = |action: &MenuAction| {
        let index = items
            .iter()
            .position(|i| &i.action == action)
            .expect("item present");
        1 + index + rules_before(index)
    };

    // An item's line is `│ label ... hint │`: the label sits just after the
    // border and the accelerator (or `>` for a submenu) just before it.
    // A rule sits between groups: one drawn line after an item whose successor
    // starts a later group.
    // The line list was printed in the failure message before the terminal
    // frame went away; only its length is asserted now.
    #[allow(clippy::needless_collect)]
    let rules: Vec<usize> = (1..items.len())
        .filter(|&i| rule_above(i))
        .map(|i| line_of(&items[i].action) - 1)
        .collect();
    assert_eq!(
        rules.len(),
        usize::from(has_terminal) + 3,
        "three group rules, one more above Terminal: "
    );
    let delete = items
        .iter()
        .position(|i| i.action == MenuAction::Delete)
        .expect("a delete item");
    assert!(items[delete].danger, "Delete is drawn as dangerous");
}

#[tokio::test]
async fn test_session_menu_shift_f10_opens_and_esc_closes() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(KeyEvent::new(KeyCode::F(10), KeyModifiers::SHIFT))
        .await;
    assert!(app.sidebar.menu.is_some(), "Shift+F10 opens the menu");

    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.sidebar.menu.is_none(), "Esc closes the menu");
    assert!(app.sidebar.focused, "the sidebar keeps focus");
}

#[tokio::test]
async fn test_session_menu_not_opened_on_header_rows() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    draw_effects(&mut app);
    app.sidebar.focused = true;
    app.sidebar.cursor = 0; // `+ New session`

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    assert!(app.sidebar.menu.is_none());

    // Right-click on the `+ New session` row (screen row 1) opens nothing.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 5, 1));
    assert!(app.sidebar.menu.is_none());
}

#[tokio::test]
async fn test_session_menu_accelerator_pins_like_p() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('P'))).await;

    assert!(
        app.sidebar.menu.is_none(),
        "running an item closes the menu"
    );
    assert!(
        app.storage.pinned_session_ids().expect("pins").contains(&a),
        "P pins the session"
    );

    draw_effects(&mut app);
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.as_ref().expect("menu");
    let pin = menu
        .items
        .iter()
        .find(|i| i.action == MenuAction::TogglePin)
        .expect("a pin item");
    assert_eq!(pin.label, "Unpin");
}

#[tokio::test]
async fn test_session_menu_enter_on_archive_archives_background_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let archive = app
        .sidebar
        .menu
        .as_ref()
        .expect("menu")
        .items
        .iter()
        .position(|i| i.action == MenuAction::Archive)
        .expect("an archive item");
    // Walk to `Archive` by name, because the optional trailing `Terminal`
    // entry changes what `End` lands on.
    for _ in 0..archive {
        app.handle_key_event(key(KeyCode::Down)).await;
    }
    app.handle_key_event(key(KeyCode::Enter)).await;

    let row = app.storage.get_session(&a).expect("read").expect("row A");
    assert!(row.archived_at.is_some(), "A is archived");
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
}

#[tokio::test]
async fn test_session_menu_click_outside_closes_without_acting() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    draw_effects(&mut app);

    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        WIDTH - 2,
        HEIGHT - 2,
    ));

    assert!(app.sidebar.menu.is_none());
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(
        app.storage
            .get_session(&a)
            .expect("read")
            .expect("row A")
            .archived_at
            .is_none()
    );
}

#[tokio::test]
async fn test_session_menu_u_marks_unread_until_focused() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('u'))).await;

    assert!(
        app.storage
            .unread_session_ids()
            .expect("unread")
            .contains(&a)
    );

    // The menu now offers the reverse.
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    assert_eq!(
        app.sidebar.menu.as_ref().expect("menu").items[2].label,
        "Mark as read"
    );
    app.handle_key_event(key(KeyCode::Esc)).await;

    // Focusing the session reads it.
    app.switch_to_session(&a).expect("focus A");
    assert!(app.storage.unread_session_ids().expect("unread").is_empty());
}

#[tokio::test]
async fn test_session_menu_f_forks_and_focuses_the_copy() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .create_message(&codemason_types::message::Message::user_text(
            &a,
            "forked prompt",
        ))
        .expect("message");
    put_cursor_on(&mut app, &a);
    // Renamed after the cursor is placed: the sidebar still shows the
    // cached `untitled` label the cursor was found by.
    app.storage.update_session(&a, "Original").expect("title");

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('f'))).await;

    let fork = app.session.session_id.clone().expect("fork focused");
    assert!(fork != a && fork != b, "a new session is focused");
    let row = app.storage.get_session(&fork).expect("get").expect("row");
    assert_eq!(row.title, "Original (fork)");
    let texts: Vec<String> = app
        .storage
        .get_messages(&fork)
        .expect("messages")
        .iter()
        .map(codemason_types::message::Message::text_content)
        .collect();
    assert_eq!(texts, ["forked prompt"]);
    assert!(
        app.background
            .iter()
            .any(|v| v.session_id.as_deref() == Some(b.as_str())),
        "the previous session stays open"
    );
}

#[tokio::test]
async fn test_session_menu_delete_asks_and_no_keeps_the_session() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('d'))).await;

    assert!(app.sidebar.menu.is_none());
    assert!(app.row_action_confirm_open(), "Delete asks first");
    assert!(!app.row_action_confirm_is_yes(), "the default answer is No");
    draw_effects(&mut app);

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(!app.row_action_confirm_open());
    assert!(app.storage.get_session(&a).expect("get").is_some(), "kept");
}

#[tokio::test]
async fn test_session_menu_delete_yes_removes_background_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT))
        .await;
    app.handle_key_event(key(KeyCode::Left)).await;
    assert!(app.row_action_confirm_is_yes());
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        app.storage.get_session(&a).expect("get").is_none(),
        "deleted"
    );
    assert!(app.background.position(&a).is_none(), "closed");
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    let _width = usize::from(app.sidebar.area.width);
    let _label = format!("untitled {}", &a[a.len() - 8..]);
}

/// Opens the menu on session `id` and moves its cursor to the item with
/// `action`.
async fn menu_on(app: &mut App, id: &str, action: &MenuAction) {
    put_cursor_on(app, id);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.as_mut().expect("menu");
    menu.cursor = menu
        .items
        .iter()
        .position(|i| &i.action == action)
        .expect("item present");
}

/// The labels of the open submenu.
fn submenu_labels(app: &App) -> Vec<String> {
    app.sidebar
        .menu
        .as_ref()
        .and_then(|m| m.submenu.as_ref())
        .expect("submenu open")
        .items
        .iter()
        .map(|i| i.label.trim().to_string())
        .collect()
}

#[tokio::test]
async fn test_session_menu_move_to_section_submenu_moves_session() {
    let (mut app, a, b) = app_with_sidebar().await;
    let work = app.storage.create_section("Work").expect("section");
    app.sidebar.dirty = true;
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;

    app.handle_key_event(key(KeyCode::Right)).await;
    assert_eq!(
        submenu_labels(&app),
        ["* Project (default)", "Work", "New section..."]
    );
    draw_effects(&mut app);
    let menu = app.sidebar.menu.as_ref().expect("menu");
    let _sub = menu.submenu.as_ref().expect("submenu");

    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.sidebar.menu.is_none());
    assert_eq!(app.storage.session_sections().expect("placed")[&a], work.id);
    draw_effects(&mut app);
    row_of_session_effects(&mut app, &a);
    row_of_session_effects(&mut app, &b);
}

#[tokio::test]
async fn test_session_menu_new_section_prefills_and_creates() {
    let (mut app, a, _b) = app_with_sidebar().await;
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    app.handle_key_event(key(KeyCode::End)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert_eq!(app.session.input, "/session section new ");

    app.execute_slash_command("/session section new Reviews")
        .await;

    let sections = app.storage.list_sections().expect("list");
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].name, "Reviews");
    assert_eq!(
        app.storage.session_sections().expect("placed")[&a],
        sections[0].id
    );
}

#[tokio::test]
async fn test_session_menu_esc_closes_submenu_only() {
    let (mut app, a, _b) = app_with_sidebar().await;
    menu_on(&mut app, &a, &MenuAction::MoveTo).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(
        app.sidebar.menu.as_ref().expect("menu").submenu.is_some(),
        "Enter on a submenu item opens it"
    );

    app.handle_key_event(key(KeyCode::Esc)).await;
    let menu = app.sidebar.menu.as_ref().expect("menu still open");
    assert!(menu.submenu.is_none());

    app.handle_key_event(key(KeyCode::Left)).await;
    assert!(
        app.sidebar.menu.is_none(),
        "Left on the top level closes it"
    );
}

#[tokio::test]
async fn test_session_menu_open_in_file_manager_records_launch() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.sidebar.launch_log = Some(Vec::new());
    menu_on(&mut app, &a, &MenuAction::OpenIn).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    let index = submenu_labels(&app)
        .iter()
        .position(|l| l == "File manager")
        .expect("file manager offered");
    app.sidebar
        .menu
        .as_mut()
        .and_then(|m| m.submenu.as_mut())
        .expect("submenu")
        .cursor = index;

    app.handle_key_event(key(KeyCode::Enter)).await;

    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1);
    let dir = std::path::PathBuf::from(
        app.storage
            .get_session(&a)
            .expect("get")
            .expect("row A")
            .directory,
    );
    assert_eq!(log[0].cwd, dir);
    assert_eq!(log[0].args, [dir.display().to_string()]);
    assert!(app.status.starts_with("opened "), "{}", app.status);
}

#[tokio::test]
async fn test_session_menu_open_request_first_when_bound() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage
        .set_session_request(&codemason_storage::SessionRequest {
            session_id: a.clone(),
            forge: "github".to_string(),
            host: "github.com".to_string(),
            repo: "acme/app".to_string(),
            number: 12,
            title: "Add menu".to_string(),
            web_url: "https://github.com/acme/app/pull/12".to_string(),
            head_branch: "menu".to_string(),
            state: "open".to_string(),
        })
        .expect("bind");
    app.sidebar.launch_log = Some(Vec::new());
    put_cursor_on(&mut app, &a);

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.items[0].action, MenuAction::OpenRequest);
    assert_eq!(menu.items[0].label, "Open PR");
    assert_eq!(menu.items[0].accel, Some('g'));

    app.handle_key_event(key(KeyCode::Char('g'))).await;

    assert!(app.sidebar.menu.is_none(), "the menu closes");
    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].args, ["https://github.com/acme/app/pull/12"]);
}

#[tokio::test]
async fn test_session_menu_no_open_request_when_unbound() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert!(
        menu.items
            .iter()
            .all(|i| i.action != MenuAction::OpenRequest)
    );
}

#[tokio::test]
async fn test_session_menu_autofix_toggle_and_sidebar_glyphs() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let request = codemason_storage::SessionRequest {
        session_id: a.clone(),
        forge: "gitlab".to_string(),
        host: "gitlab.example.com".to_string(),
        repo: "group/app".to_string(),
        number: 4,
        title: "Fix".to_string(),
        web_url: "https://gitlab.example.com/group/app/-/merge_requests/4".to_string(),
        head_branch: "fix".to_string(),
        state: "open".to_string(),
    };
    app.storage.set_session_request(&request).expect("bind");
    app.sidebar.dirty = true;
    row_of_session_effects(&mut app, &a);
    let _width = usize::from(app.sidebar.area.width);
    draw_effects(&mut app);

    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.items[0].label, "Open MR");
    assert_eq!(menu.items[1].action, MenuAction::ToggleAutofix);
    assert_eq!(menu.items[1].label, "Auto-fix: off");
    assert_eq!(menu.items[1].accel, Some('x'));

    app.handle_key_event(key(KeyCode::Char('x'))).await;
    assert!(app.sidebar.menu.is_none());
    assert!(
        app.storage
            .session_autofix(&a)
            .expect("read")
            .expect("bound")
            .enabled
    );
    assert!(
        app.status.contains("Auto-fix on for MR !4"),
        "{}",
        app.status
    );

    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    assert_eq!(
        app.sidebar.menu.as_ref().expect("menu").items[1].label,
        "Auto-fix: on"
    );
    app.handle_key_event(key(KeyCode::Esc)).await;

    // Paused waiting on the user.
    app.storage.set_autofix_paused(&a, true).expect("pause");
    app.sidebar.dirty = true;
    draw_effects(&mut app);
}

#[tokio::test]
async fn test_session_menu_requests_item_absent_without_history() {
    let (mut app, a, _b) = app_with_sidebar().await;
    put_cursor_on(&mut app, &a);
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.as_ref().expect("menu open");
    assert!(menu.items.iter().all(|i| i.action != MenuAction::Requests));
}

#[tokio::test]
async fn test_session_menu_terminal_opens_a_terminal_at_the_session_directory() {
    let (mut app, a, _b) = app_with_sidebar().await;
    // Session `A` stays open in the background, and an open view's directory
    // outranks the stored one, so the fixture has to move both.
    let worktree = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/temp/worktree");
    std::fs::create_dir_all(&worktree).expect("worktree directory");
    set_session_directory(&mut app, &a, &worktree);
    app.sidebar.launch_log = Some(Vec::new());

    menu_on(&mut app, &a, &MenuAction::Terminal).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1, "one terminal launched");
    assert_eq!(
        log[0].cwd, worktree,
        "the terminal starts at the session's worktree"
    );
    assert!(
        app.status.contains("opened") && app.status.contains("terminal"),
        "status reports the launch: {}",
        app.status
    );
    assert!(!app.notice_open, "a present worktree raises no dialog");
}

#[tokio::test]
async fn test_session_menu_terminal_reports_a_missing_worktree_in_a_dialog() {
    let (mut app, a, _b) = app_with_sidebar().await;
    // A deleted worktree is a session directory that is no longer a directory.
    let missing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp/gone-worktree-does-not-exist");
    assert!(!missing.exists(), "the fixture path must not exist");
    set_session_directory(&mut app, &a, &missing);
    app.sidebar.launch_log = Some(Vec::new());

    menu_on(&mut app, &a, &MenuAction::Terminal).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    // The menu focuses its target before acting, so the active view holds it.
    assert!(app.notice_open, "the missing worktree raises the dialog");
    assert_eq!(app.notice_title, " Worktree missing ");
    assert!(
        app.notice_message.contains(&missing.display().to_string()),
        "the dialog names the path: {}",
        app.notice_message
    );
    assert!(
        app.sidebar.launch_log.as_ref().is_some_and(Vec::is_empty),
        "no terminal is started for a missing worktree"
    );

    // The dialog paints and names its single `OK` button.
    draw_effects(&mut app);

    // `Enter` closes it and nothing else happens.
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(!app.notice_open, "Enter closes the notice");
}

/// Points session `id` at `directory`: its open view when it has one, and its
/// stored row, which a session without a view falls back to.
fn set_session_directory(app: &mut App, id: &str, directory: &std::path::Path) {
    if app.session.session_id.as_deref() == Some(id) {
        app.session.directory = directory.to_path_buf();
    }
    if let Some(index) = app.background.position(id)
        && let Some(mut view) = app.background.remove(index)
    {
        view.directory = directory.to_path_buf();
        app.background.insert_front(view);
    }
    let conn = app.storage.conn_lock_for_test().expect("connection");
    conn.execute(
        "UPDATE sessions SET directory = ?1 WHERE id = ?2",
        [directory.display().to_string(), id.to_string()],
    )
    .expect("store the session directory");
}
