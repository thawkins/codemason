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
    // The window's fonts: a new session's startup banner draws the
    // block-character logo, which egui's default fonts lack.
    codemason_gui::fonts::install(&harness.ctx, craft_ui::fonts::SystemFonts::Off);
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

/// The text of every message in the focused session's transcript.
fn transcript(app: &App) -> String {
    app.session
        .messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            codemason_agent::message::MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A folder named `name` inside a fresh scratch root that `harness`'s App
/// may open sessions in (`dirs.allowed_roots`).
fn permitted_folder(harness: &mut Harness<'static, Shell>, tag: &str, name: &str) -> PathBuf {
    let root = scratch(tag);
    let folder = root.join(name);
    std::fs::create_dir_all(&folder).expect("create folder");
    support::allow_session_root(harness.state().app(), &root);
    folder.canonicalize().expect("canonical folder")
}

#[test]
fn test_open_project_dialog_registers_the_folder_as_a_project() {
    // The permitted roots read the process cwd and config overrides.
    let _env = crate::global_env_lock();
    let (_runtime, mut harness, requests) = rig(Answer::Cancelled);
    let folder = permitted_folder(&mut harness, "open-project", "Ann's App");
    let answer = folder.clone();
    harness
        .state_mut()
        .set_file_dialog_override(Some(Box::new(move |_: &Request| {
            Answer::Paths(vec![answer.clone()])
        })));

    choose(&mut harness, OPEN_PROJECT_LABEL);

    assert!(
        requests.borrow().is_empty(),
        "the replaced override answered"
    );
    // `/project add` registers the checkout the folder belongs to (here
    // the enclosing workspace repository), as it does for a typed path.
    let text = transcript(harness.state().app());
    let registered = text
        .split_once("registered at `")
        .and_then(|(_, rest)| rest.split_once('`'))
        .map(|(path, _)| PathBuf::from(path))
        .unwrap_or_else(|| panic!("no project was registered: {text}"));
    assert!(
        folder.starts_with(&registered),
        "{} holds the chosen folder {}",
        registered.display(),
        folder.display()
    );
}

#[test]
fn test_new_project_dir_dialog_opens_a_session_in_any_folder_name() {
    // The permitted roots read the process cwd and config overrides.
    let _env = crate::global_env_lock();
    for (tag, name) in [
        ("new-apostrophe", "Ann's App"),
        ("new-spaces", "two words here"),
        ("new-dashes", "draft --x"),
    ] {
        let (_runtime, mut harness, requests) = rig(Answer::Cancelled);
        let folder = permitted_folder(&mut harness, tag, name);
        let answer = folder.clone();
        harness
            .state_mut()
            .set_file_dialog_override(Some(Box::new(move |_: &Request| {
                Answer::Paths(vec![answer.clone()])
            })));

        choose(&mut harness, NEW_PROJECT_DIR_LABEL);

        assert!(requests.borrow().is_empty());
        let app = harness.state().app();
        assert_eq!(
            app.session.directory, folder,
            "{name:?}: the new session works in the chosen folder (status {:?})",
            app.status
        );
        assert_eq!(app.cwd_path, folder, "{name:?}: the App moved into it");
        assert_ne!(app.session.session_id.as_deref(), Some(SESSION));
    }
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
