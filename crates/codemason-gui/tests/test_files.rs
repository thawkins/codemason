//! Tests for the native file dialogs (`codemason_gui::platform::files`):
//! a File menu item queues the dialog, the answer is taken on a later frame
//! and runs the app-core action a typed path reaches, and a cancelled
//! dialog changes nothing.
//!
//! The dialogs are answered through craft-dialogs' override
//! (`Shell::set_file_dialog_override`), so no native window opens. GUI
//! assertions use egui_kittest accessibility queries. Files are written
//! only under the workspace `target/temp/`.

#[path = "support/mod.rs"]
mod support;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use codemason_app_core::app::App;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::platform::files::{
    ATTACH_LABEL, FILE_MENU_LABEL, NEW_PROJECT_DIR_LABEL, OPEN_PROJECT_LABEL,
};
use craft_dialogs::{Answer, Kind, Request};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The session the tests run in.
const SESSION: &str = "session-files";

/// A fresh scratch directory under the workspace `target/temp/`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/gui-files")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch dir");
    }
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// A shell with sources attached whose file dialogs answer `answer` and
/// record each request, plus the runtime driving it.
fn rig(
    answer: Answer,
) -> (
    tokio::runtime::Runtime,
    Harness<'static, Shell>,
    Rc<RefCell<Vec<Request>>>,
) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let mut shell = Shell::new(app).with_sources(events, logs, runtime.handle().clone());
    let requests = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&requests);
    shell.set_file_dialog_override(Some(Box::new(move |request: &Request| {
        seen.borrow_mut().push(request.clone());
        answer.clone()
    })));
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness, requests)
}

/// Open the File menu and choose `item`, then run the frames that show the
/// dialog and take its answer.
fn choose(harness: &mut Harness<'static, Shell>, item: &str) {
    harness.run();
    harness.get_by_label(FILE_MENU_LABEL).click();
    harness.run();
    harness.get_by_label(item).click();
    harness.run();
}

/// The log lines the App holds.
fn log_lines(app: &App) -> Vec<String> {
    app.log_entries.iter().map(|e| e.message.clone()).collect()
}

#[test]
fn test_attach_dialog_answer_adds_attachment() {
    let dir = scratch("attach");
    let image = dir.join("shot.png");
    std::fs::write(&image, b"png").expect("write image");
    let (_runtime, mut harness, requests) = rig(Answer::Paths(vec![image.clone()]));

    choose(&mut harness, ATTACH_LABEL);

    let requests = requests.borrow();
    assert_eq!(requests.len(), 1);
    let Request::File(dialog) = &requests[0] else {
        panic!("a file dialog was asked: {:?}", requests[0]);
    };
    assert_eq!(dialog.kind, Kind::Open { multiple: true });
    assert!(
        dialog
            .filters
            .iter()
            .any(|f| f.extensions.contains(&"png".to_string()))
    );
    assert_eq!(
        harness.state().app().session.pending_attachments,
        vec![image]
    );
    // The prompt lists the staged image.
    harness.get_by_label("shot.png");
}

#[test]
fn test_cancelled_dialog_changes_nothing() {
    let (_runtime, mut harness, requests) = rig(Answer::Cancelled);
    harness.run();
    let (messages, logs, status) = {
        let app = harness.state().app();
        (
            app.session.messages.len(),
            log_lines(app),
            app.status.clone(),
        )
    };

    for item in [ATTACH_LABEL, OPEN_PROJECT_LABEL, NEW_PROJECT_DIR_LABEL] {
        choose(&mut harness, item);
    }

    assert_eq!(requests.borrow().len(), 3, "each item asked once");
    let app = harness.state().app();
    assert!(app.session.pending_attachments.is_empty());
    assert_eq!(app.session.messages.len(), messages);
    assert_eq!(log_lines(app), logs);
    assert_eq!(app.status, status);
    assert_eq!(app.session.session_id.as_deref(), Some(SESSION));
}

#[test]
fn test_open_project_dialog_runs_project_add_with_the_folder() {
    let dir = scratch("open-project");
    let (_runtime, mut harness, requests) = rig(Answer::Paths(vec![dir.clone()]));

    choose(&mut harness, OPEN_PROJECT_LABEL);

    let Request::File(dialog) = &requests.borrow()[0] else {
        panic!("a file dialog was asked");
    };
    assert_eq!(dialog.kind, Kind::Folder);
    let expected = format!("Executing /project add {}", dir.display());
    assert!(
        log_lines(harness.state().app()).contains(&expected),
        "the folder reaches `/project add` as a typed path does"
    );
}

#[test]
fn test_new_project_dir_dialog_runs_session_new_with_the_folder() {
    let dir = scratch("new-project");
    let (_runtime, mut harness, requests) = rig(Answer::Paths(vec![dir.clone()]));

    choose(&mut harness, NEW_PROJECT_DIR_LABEL);

    let Request::File(dialog) = &requests.borrow()[0] else {
        panic!("a file dialog was asked");
    };
    assert_eq!(dialog.kind, Kind::Folder);
    let expected = format!("Executing /session new {}", dir.display());
    assert!(
        log_lines(harness.state().app()).contains(&expected),
        "the folder reaches `/session new` as a typed path does"
    );
}

#[test]
fn test_unavailable_dialog_is_logged_and_changes_nothing_else() {
    let (_runtime, mut harness, _requests) = rig(Answer::Unavailable("no portal".to_string()));

    choose(&mut harness, ATTACH_LABEL);

    let app = harness.state().app();
    assert!(app.session.pending_attachments.is_empty());
    assert!(
        log_lines(app)
            .iter()
            .any(|l| l.contains("could not be shown: no portal"))
    );
}
