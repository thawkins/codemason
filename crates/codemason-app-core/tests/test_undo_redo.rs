//! `/undo` and `/redo`: the turn leaves and returns to memory, storage and the
//! workspace files.

use std::path::{Path, PathBuf};

use codemason_agent::session::turn_snapshot::{self, SnapshotRepo};
use codemason_app_core::app::App;
use codemason_types::message::{Message, MessagePart, Role};
use support::make_app;

mod support;

const SID: &str = "undo-session";

fn temp_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/undo_redo")
}

/// A fresh project directory with a `.git` marker, and the app focused on it.
/// Every test shares one snapshot store: the store is process-wide.
fn app_in_project(name: &str) -> (App, PathBuf) {
    turn_snapshot::enable(temp_root().join("store"));
    let project = temp_root().join(name);
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join(".git")).expect("project dir");
    let project = project.canonicalize().expect("canonical project dir");

    let mut app = make_app();
    app.cwd_path = project.clone();
    app.cwd = project.display().to_string();
    app.session.session_id = Some(SID.to_string());
    app.storage
        .create_session(SID, &project.display().to_string())
        .expect("create_session");
    (app, project)
}

/// Append a stored message to the focused session, one second after the last.
fn push(app: &mut App, role: Role, text: &str) -> String {
    let mut message = Message::user_text(SID, text);
    message.role = role;
    message.parts = vec![MessagePart::Text {
        text: text.to_string(),
    }];
    let at = chrono::Utc::now() + chrono::Duration::seconds(app.session.messages.len() as i64);
    message.created_at = at;
    message.updated_at = at;
    app.storage
        .create_message(&message)
        .expect("create_message");
    let id = message.id.clone();
    app.session.messages.push(message);
    id
}

fn stored_texts(app: &App) -> Vec<String> {
    app.storage
        .get_messages(SID)
        .expect("get_messages")
        .iter()
        .map(Message::text_content)
        .collect()
}

fn read(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

#[tokio::test]
async fn test_undo_removes_turn_from_storage_and_restores_files() {
    let (mut app, project) = app_in_project("undo_files");
    push(&mut app, Role::User, "first question");
    push(&mut app, Role::Assistant, "first answer");
    std::fs::write(project.join("a.txt"), "before").expect("write");

    // The second turn: snapshot at its first tool call, then it changes files.
    let turn = push(&mut app, Role::User, "edit the file");
    SnapshotRepo::for_working_dir(&project)
        .expect("snapshot repo")
        .record_turn(SID, &turn)
        .await
        .expect("record_turn");
    std::fs::write(project.join("a.txt"), "after").expect("write");
    std::fs::write(project.join("new.txt"), "created").expect("write");
    push(&mut app, Role::Assistant, "done");

    app.execute_slash_command("/undo").await;

    assert_eq!(app.session.messages.len(), 2);
    assert_eq!(stored_texts(&app), ["first question", "first answer"]);
    assert_eq!(read(&project, "a.txt").as_deref(), Some("before"));
    assert_eq!(read(&project, "new.txt"), None);
    assert!(
        app.status.contains("removed 2 message(s)"),
        "{}",
        app.status
    );
    assert!(app.status.contains("2 file(s) changed"), "{}", app.status);

    app.execute_slash_command("/redo").await;

    assert_eq!(
        stored_texts(&app),
        ["first question", "first answer", "edit the file", "done"]
    );
    assert_eq!(app.session.messages.len(), 4);
    assert_eq!(read(&project, "a.txt").as_deref(), Some("after"));
    assert_eq!(read(&project, "new.txt").as_deref(), Some("created"));
    assert!(
        app.status.contains("restored 2 message(s)"),
        "{}",
        app.status
    );

    // The redone turn can be undone again.
    app.execute_slash_command("/undo").await;
    assert_eq!(read(&project, "a.txt").as_deref(), Some("before"));
    assert_eq!(read(&project, "new.txt"), None);
    assert_eq!(stored_texts(&app).len(), 2);
}

#[tokio::test]
async fn test_undo_without_snapshot_leaves_files_alone() {
    let (mut app, project) = app_in_project("undo_no_snapshot");
    std::fs::write(project.join("a.txt"), "user edit").expect("write");
    push(&mut app, Role::User, "just a question");
    push(&mut app, Role::Assistant, "just an answer");

    app.execute_slash_command("/undo").await;

    assert!(app.session.messages.is_empty());
    assert_eq!(stored_texts(&app), [] as [std::string::String; 0]);
    assert_eq!(read(&project, "a.txt").as_deref(), Some("user edit"));
    assert!(!app.status.contains("file(s) changed"), "{}", app.status);
}

#[tokio::test]
async fn test_multiple_undo_then_redo_in_reverse_order() {
    let (mut app, _project) = app_in_project("undo_stack");
    push(&mut app, Role::User, "q1");
    push(&mut app, Role::Assistant, "a1");
    push(&mut app, Role::User, "q2");
    push(&mut app, Role::Assistant, "a2");

    app.execute_slash_command("/undo").await;
    app.execute_slash_command("/undo").await;
    assert_eq!(stored_texts(&app), [] as [std::string::String; 0]);

    app.execute_slash_command("/redo").await;
    assert_eq!(stored_texts(&app), ["q1", "a1"]);
    app.execute_slash_command("/redo").await;
    assert_eq!(stored_texts(&app), ["q1", "a1", "q2", "a2"]);

    app.execute_slash_command("/redo").await;
    assert_eq!(app.status, "[warn] Nothing to redo");
}

#[tokio::test]
async fn test_redo_is_dropped_once_the_conversation_moves_on() {
    let (mut app, _project) = app_in_project("redo_stale");
    push(&mut app, Role::User, "q1");
    push(&mut app, Role::Assistant, "a1");

    app.execute_slash_command("/undo").await;
    push(&mut app, Role::User, "a different question");
    app.execute_slash_command("/redo").await;

    assert_eq!(app.status, "[warn] Nothing to redo");
    assert_eq!(stored_texts(&app), ["a different question"]);
    assert!(app.session.undo_stack.is_empty());
}

#[tokio::test]
async fn test_undo_and_redo_refuse_while_the_agent_runs() {
    let (mut app, _project) = app_in_project("undo_busy");
    push(&mut app, Role::User, "q1");
    app.session.is_processing = true;

    app.execute_slash_command("/undo").await;
    assert_eq!(app.status, "[warn] Cannot undo while the agent is running");
    assert_eq!(stored_texts(&app), ["q1"]);

    app.execute_slash_command("/redo").await;
    assert_eq!(app.status, "[warn] Cannot redo while the agent is running");
}

#[tokio::test]
async fn test_redo_help() {
    let (mut app, _project) = app_in_project("redo_help");
    app.execute_slash_command("/redo help").await;
    let text = app
        .session
        .messages
        .last()
        .expect("help creates a message")
        .text_content();
    assert!(text.starts_with("From: /redo help"), "{text}");
    assert_eq!(app.status, "redo: help");
}

/// A live turn: the transcript holds its own copy of the user message, under
/// a different id, created just before the processor stores its copy.
#[tokio::test]
async fn test_undo_resolves_the_stored_turn_when_ids_differ() {
    let (mut app, project) = app_in_project("undo_live_ids");
    push(&mut app, Role::User, "first question");
    push(&mut app, Role::Assistant, "first answer");
    std::fs::write(project.join("a.txt"), "before").expect("write");

    let mut view_user = Message::user_text(SID, "/review a.txt");
    view_user.created_at = chrono::Utc::now() + chrono::Duration::seconds(10);
    let mut stored_user = Message::user_text(SID, "Review a.txt for bugs.");
    stored_user.created_at = view_user.created_at + chrono::Duration::milliseconds(5);
    stored_user.updated_at = stored_user.created_at;
    let mut stored_reply = Message::user_text(SID, "edited it");
    stored_reply.role = Role::Assistant;
    stored_reply.created_at = stored_user.created_at + chrono::Duration::seconds(1);
    stored_reply.updated_at = stored_reply.created_at;
    app.storage.create_message(&stored_user).expect("store");
    app.storage.create_message(&stored_reply).expect("store");
    app.session.messages.push(view_user);
    let mut view_reply = Message::user_text(SID, "edited it");
    view_reply.role = Role::Assistant;
    app.session.messages.push(view_reply);
    SnapshotRepo::for_working_dir(&project)
        .expect("snapshot repo")
        .record_turn(SID, &stored_user.id)
        .await
        .expect("record_turn");
    std::fs::write(project.join("a.txt"), "after").expect("write");

    app.execute_slash_command("/undo").await;

    assert_eq!(stored_texts(&app), ["first question", "first answer"]);
    assert_eq!(app.session.messages.len(), 2);
    assert_eq!(read(&project, "a.txt").as_deref(), Some("before"));

    app.execute_slash_command("/redo").await;

    assert_eq!(
        stored_texts(&app),
        [
            "first question",
            "first answer",
            "Review a.txt for bugs.",
            "edited it"
        ]
    );
    assert_eq!(app.session.messages.len(), 4);
    assert_eq!(app.session.messages[2].text_content(), "/review a.txt");
    assert_eq!(read(&project, "a.txt").as_deref(), Some("after"));
}

/// A transcript-only user message newer than every stored turn (its turn
/// never reached storage) must not take an older stored turn with it.
#[tokio::test]
async fn test_undo_of_an_unstored_turn_leaves_storage_alone() {
    let (mut app, _project) = app_in_project("undo_unstored");
    push(&mut app, Role::User, "stored question");
    push(&mut app, Role::Assistant, "stored answer");
    let mut unstored = Message::user_text(SID, "/failed-command");
    unstored.created_at = chrono::Utc::now() + chrono::Duration::seconds(60);
    app.session.messages.push(unstored);

    app.execute_slash_command("/undo").await;

    assert_eq!(app.session.messages.len(), 2);
    assert_eq!(stored_texts(&app), ["stored question", "stored answer"]);
    assert!(
        app.status.contains("removed 1 message(s)"),
        "{}",
        app.status
    );

    app.execute_slash_command("/redo").await;
    assert_eq!(app.session.messages.len(), 3);
    assert_eq!(stored_texts(&app).len(), 2);
}

#[tokio::test]
async fn test_undo_refuses_while_another_session_runs_in_the_work_tree() {
    let (mut app, project) = app_in_project("undo_other_session");
    push(&mut app, Role::User, "q1");
    push(&mut app, Role::Assistant, "a1");
    let mut other = codemason_app_core::app::SessionView::new(
        app.session.agent_name.clone(),
        app.session.agent_info.clone(),
        app.session.current_agent_index,
    );
    other.session_id = Some("other-session".to_string());
    other.directory = project.join("sub");
    other.is_processing = true;
    app.background.insert_front(other);

    app.execute_slash_command("/undo").await;

    assert_eq!(
        app.status,
        "[warn] Cannot undo while another session is running in this work tree"
    );
    assert_eq!(stored_texts(&app), ["q1", "a1"]);
    assert_eq!(app.session.messages.len(), 2);
}
