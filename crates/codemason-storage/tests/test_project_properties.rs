//! Project properties (schema 13): a description and an execution backend
//! that sessions copy when they are created.

use codemason_storage::{SessionCheckout, SessionKind, Storage};

#[test]
fn test_project_properties_description_and_backend_round_trip() {
    let storage = Storage::open_in_memory().expect("storage");
    let project = storage
        .ensure_project("/nonexistent/props")
        .expect("project");
    assert_eq!(project.description, None);
    assert_eq!(project.execution_backend, None);

    storage
        .set_project_description(&project.id, Some("AI coding agent"))
        .expect("describe");
    storage
        .set_project_backend(&project.id, Some("podman"))
        .expect("backend");
    let row = storage.get_project(&project.id).expect("get").expect("row");
    assert_eq!(row.description.as_deref(), Some("AI coding agent"));
    assert_eq!(row.execution_backend.as_deref(), Some("podman"));

    storage
        .set_project_description(&project.id, None)
        .expect("clear");
    storage
        .set_project_backend(&project.id, None)
        .expect("clear");
    let row = storage.get_project(&project.id).expect("get").expect("row");
    assert_eq!((row.description, row.execution_backend), (None, None));
}

#[test]
fn test_project_properties_sessions_copy_the_backend_when_created() {
    let storage = Storage::open_in_memory().expect("storage");
    let project = storage
        .ensure_project("/nonexistent/props_copy")
        .expect("project");
    storage
        .create_session_in_project(
            "before",
            &project.id,
            &project.worktree,
            &SessionCheckout::default(),
        )
        .expect("before");

    storage
        .set_project_backend(&project.id, Some("docker"))
        .expect("backend");
    storage
        .create_session_in_project(
            "after",
            &project.id,
            &project.worktree,
            &SessionCheckout::default(),
        )
        .expect("after");
    storage
        .create_session("by_dir", &project.worktree)
        .expect("by directory");
    storage.fork_session("after", "fork", "fork").expect("fork");
    // A sub-agent session follows its parent, not the project's setting.
    storage
        .create_session_with(
            "child",
            &project.worktree,
            Some("before"),
            SessionKind::Interactive,
        )
        .expect("child");

    let backend = |id: &str| storage.session_backend(id).expect("backend");
    assert_eq!(backend("before"), None, "created before the setting");
    assert_eq!(backend("after").as_deref(), Some("docker"));
    assert_eq!(backend("by_dir").as_deref(), Some("docker"));
    assert_eq!(backend("fork").as_deref(), Some("docker"));
    assert_eq!(backend("child"), None, "inherits from its parent");

    storage
        .set_project_backend(&project.id, None)
        .expect("clear");
    assert_eq!(
        backend("after").as_deref(),
        Some("docker"),
        "existing keep theirs"
    );
}

#[test]
fn test_project_properties_session_backends_lists_only_own_backends() {
    let storage = Storage::open_in_memory().expect("storage");
    let project = storage
        .ensure_project("/nonexistent/props_list")
        .expect("project");
    for id in ["global", "podman", "remote"] {
        storage
            .create_session_in_project(
                id,
                &project.id,
                &project.worktree,
                &SessionCheckout::default(),
            )
            .expect("session");
    }
    storage
        .set_session_backend("podman", Some("podman"))
        .expect("podman");
    storage
        .set_session_backend("remote", Some("remote"))
        .expect("remote");

    let backends = storage.session_backends().expect("backends");
    assert_eq!(backends.len(), 2, "{backends:?}");
    assert_eq!(backends.get("podman").map(String::as_str), Some("podman"));
    assert_eq!(backends.get("remote").map(String::as_str), Some("remote"));
    assert!(
        !backends.contains_key("global"),
        "global sessions are absent"
    );
}
