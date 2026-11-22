//! Tests for `/connectors add` and `/connectors remove` install/remove
//! operations with refusal guards (spec `connectors` T-009; FR-011, FR-027,
//! FR-028, FR-029, FR-030).

mod support;

use support::TempTree;

use std::io::Write as _;
use std::path::{Path, PathBuf};

use codemason_connectors::{
    AddError, ConnectorDescriptor, InstallSource, MANIFEST_FILE, RemoveError, StageError,
    StoreDirs, StoreLedger, add, classify_source, install_descriptor, remove,
};

fn dirs(tree: &TempTree) -> StoreDirs {
    StoreDirs {
        project: Some(tree.0.join("proj/.codemason/connectors")),
        global: Some(tree.0.join("global/connectors")),
    }
}

/// A minimal, valid connector manifest with one stdio server.
fn manifest_for(id: &str, name: &str) -> String {
    format!(
        r#"{{
            "id": "{id}",
            "name": "{name}",
            "category": "developer",
            "servers": [ {{ "id": "main", "transport": "stdio", "command": "npx" }} ]
        }}"#
    )
}

fn descriptor_for(id: &str) -> ConnectorDescriptor {
    serde_json::from_str(&manifest_for(id, "Echo")).expect("descriptor parses")
}

fn stage_source_dir(root: &Path, id: &str) -> PathBuf {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).expect("source dir creatable");
    std::fs::write(dir.join(MANIFEST_FILE), manifest_for(id, "Echo")).expect("manifest writable");
    dir
}

fn make_zip(path: &Path, files: &[(&str, &str)]) {
    std::fs::create_dir_all(path.parent().expect("has parent")).unwrap();
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in files {
        writer.start_file(name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

// -- classify_source (FR-011) -----------------------------------------------

#[test]
fn classify_existing_directory_is_local() {
    let tree = TempTree::new("classify-dir");
    let src = stage_source_dir(&tree.0, "echo");
    assert_eq!(
        classify_source(src.to_str().unwrap(), &tree.0),
        InstallSource::Local(src.to_str().unwrap().to_string())
    );
}

#[test]
fn classify_relative_directory_resolves_against_workdir() {
    let tree = TempTree::new("classify-rel");
    stage_source_dir(&tree.0.join("vendor"), "echo");
    assert_eq!(
        classify_source("vendor/echo", &tree.0),
        InstallSource::Local("vendor/echo".to_string())
    );
}

#[test]
fn classify_https_and_http_are_local_for_stage_to_guard() {
    assert_eq!(
        classify_source("https://example.org/connectors.zip", Path::new("/w")),
        InstallSource::Local("https://example.org/connectors.zip".to_string())
    );
    // A non-https scheme is still routed to stage so it reports the rejection.
    assert_eq!(
        classify_source("http://example.org/connectors.zip", Path::new("/w")),
        InstallSource::Local("http://example.org/connectors.zip".to_string())
    );
}

#[test]
fn classify_archive_extension_is_local_even_when_absent() {
    assert_eq!(
        classify_source("bundle.zip", Path::new("/w")),
        InstallSource::Local("bundle.zip".to_string())
    );
    assert_eq!(
        classify_source("bundle.tar.gz", Path::new("/w")),
        InstallSource::Local("bundle.tar.gz".to_string())
    );
}

#[test]
fn classify_bare_token_is_a_catalogue_id() {
    assert_eq!(
        classify_source("google-drive", Path::new("/w")),
        InstallSource::CatalogueId("google-drive".to_string())
    );
}

#[test]
fn classify_missing_path_shaped_source_is_local() {
    // A mistyped path is reported as an unrecognised source, not as a missing
    // catalogue id.
    assert_eq!(
        classify_source("no/such/dir", Path::new("/w")),
        InstallSource::Local("no/such/dir".to_string())
    );
}

// -- add: local directory (FR-011, FR-027) ----------------------------------

#[test]
fn add_local_directory_installs_an_enabled_connector() {
    let tree = TempTree::new("add-dir");
    let src = stage_source_dir(&tree.0.join("src"), "echo");

    let staged = add(&dirs(&tree), &tree.0, src.to_str().unwrap(), false, &[]).expect("install");

    assert_eq!(staged.descriptor.id.as_str(), "echo");
    let store = tree.0.join("proj/.codemason/connectors");
    assert!(store.join("echo").join(MANIFEST_FILE).is_file());
    // FR-011: recorded enabled so a fresh connector is ready to use.
    let ledger = StoreLedger::load(&store);
    assert!(ledger.state("echo").is_some_and(|s| s.enabled));
}

#[test]
fn add_refuses_an_existing_id_without_force_and_overwrites_with_force() {
    let tree = TempTree::new("add-collision");
    let src = stage_source_dir(&tree.0.join("src"), "echo");
    add(&dirs(&tree), &tree.0, src.to_str().unwrap(), false, &[]).expect("first");

    let err = add(&dirs(&tree), &tree.0, src.to_str().unwrap(), false, &[]).expect_err("refused");
    assert!(
        matches!(err, AddError::Stage(StageError::Exists(ref id)) if id == "echo"),
        "got {err:?}"
    );

    add(&dirs(&tree), &tree.0, src.to_str().unwrap(), true, &[]).expect("force overwrites");
}

#[test]
fn add_refuses_a_non_https_url_naming_the_scheme() {
    let tree = TempTree::new("add-http");
    let err = add(
        &dirs(&tree),
        &tree.0,
        "http://example.org/connectors.zip",
        false,
        &[],
    )
    .expect_err("refused");
    assert!(
        matches!(err, AddError::Stage(StageError::NotHttps(ref s)) if s.starts_with("http://")),
        "got {err:?}"
    );
}

#[test]
fn add_refuses_an_archive_entry_that_escapes_the_store() {
    let tree = TempTree::new("add-escape");
    let zip = tree.0.join("escape.zip");
    make_zip(
        &zip,
        &[
            ("../../escape.json", "{}"),
            (MANIFEST_FILE, &manifest_for("echo", "Echo")),
        ],
    );

    let err = add(&dirs(&tree), &tree.0, zip.to_str().unwrap(), false, &[]).expect_err("refused");
    assert!(
        matches!(err, AddError::Stage(StageError::UnsafePath(ref p)) if p.contains("escape.json")),
        "got {err:?}"
    );
    // No write happened outside the store tree.
    assert!(!tree.0.parent().unwrap().join("escape.json").exists());
    assert!(!tree.0.join("escape.json").exists());
}

#[test]
fn add_installs_from_a_zip_archive() {
    let tree = TempTree::new("add-zip");
    let zip = tree.0.join("echo.zip");
    make_zip(&zip, &[(MANIFEST_FILE, &manifest_for("echo", "Echo"))]);

    let staged = add(&dirs(&tree), &tree.0, zip.to_str().unwrap(), false, &[]).expect("install");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
    assert!(staged.installed_dir.join(MANIFEST_FILE).is_file());
}

// -- add: catalogue id (FR-011) ---------------------------------------------

#[test]
fn add_installs_a_catalogue_id_from_the_supplied_catalogue() {
    let tree = TempTree::new("add-cat-id");
    let catalogue = vec![descriptor_for("echo")];

    let staged = add(&dirs(&tree), &tree.0, "echo", false, &catalogue).expect("install");
    assert_eq!(staged.descriptor.id.as_str(), "echo");
    let store = tree.0.join("proj/.codemason/connectors");
    assert!(store.join("echo").join(MANIFEST_FILE).is_file());
    // FR-011: a catalogue install is recorded enabled, ready to use.
    assert!(
        StoreLedger::load(&store)
            .state("echo")
            .is_some_and(|s| s.enabled)
    );
}

#[test]
fn add_refuses_a_catalogue_id_the_catalogue_does_not_hold() {
    let tree = TempTree::new("add-cat-miss");
    let err = add(&dirs(&tree), &tree.0, "google-drive", false, &[]).expect_err("refused");
    assert!(
        matches!(err, AddError::CatalogueNotFound(ref id) if id == "google-drive"),
        "got {err:?}"
    );
    // No connector directory appeared.
    assert!(
        !tree
            .0
            .join("proj/.codemason/connectors/google-drive")
            .exists()
    );
}

// -- remove (FR-030) --------------------------------------------------------

/// Install `echo` (recorded enabled by the install, FR-011) and return the
/// store path it landed in.
fn install_echo(tree: &TempTree) -> PathBuf {
    let store = tree.0.join("proj/.codemason/connectors");
    install_descriptor(&store, descriptor_for("echo"), false).expect("install");
    store
}

#[test]
fn remove_deletes_a_connector_and_clears_its_ledger_row() {
    let tree = TempTree::new("remove-ok");
    let store = install_echo(&tree);
    assert!(store.join("echo").is_dir());

    // A fresh install is enabled (FR-011), and removal is refused while
    // enabled (FR-030); disable it first, then remove.
    let mut ledger = StoreLedger::load(&store);
    ledger.state_mut("echo").enabled = false;
    ledger.save(&store).expect("ledger saved");

    let outcome = remove(&dirs(&tree), "echo").expect("removed");

    assert_eq!(outcome.id, "echo");
    assert_eq!(outcome.dir, store.join("echo"));
    assert!(!store.join("echo").exists());
    // The ledger row is gone so a re-add starts clean.
    assert!(StoreLedger::load(&store).state("echo").is_none());
}

#[test]
fn remove_refuses_while_enabled_and_changes_nothing() {
    let tree = TempTree::new("remove-enabled");
    let store = install_echo(&tree);
    // A fresh install is enabled (FR-011), so removal is refused as-is.

    let err = remove(&dirs(&tree), "echo").expect_err("refused");
    assert!(
        matches!(err, RemoveError::Enabled(ref id) if id == "echo"),
        "got {err:?}"
    );
    // FR-030: the refusal changes no file.
    assert!(store.join("echo").is_dir());
    assert!(
        StoreLedger::load(&store)
            .state("echo")
            .is_some_and(|s| s.enabled)
    );
}

#[test]
fn remove_reports_an_unknown_connector_id() {
    let tree = TempTree::new("remove-unknown");
    install_echo(&tree);
    let err = remove(&dirs(&tree), "nope").expect_err("refused");
    assert!(
        matches!(err, RemoveError::UnknownConnector(ref id) if id == "nope"),
        "got {err:?}"
    );
}

#[test]
fn remove_never_creates_a_ledger_when_none_existed() {
    let tree = TempTree::new("remove-no-ledger");
    let store = tree.0.join("proj/.codemason/connectors");
    // Install a connector, then remove the ledger so the store has no `_state.json`.
    install_descriptor(&store, descriptor_for("echo"), false).expect("install");
    std::fs::remove_file(store.join(codemason_connectors::STATE_FILE)).expect("ledger removed");

    remove(&dirs(&tree), "echo").expect("removed");
    // The removal must not have re-created the ledger as a side effect.
    assert!(!store.join(codemason_connectors::STATE_FILE).exists());
}

// -- tenant-hosted catalogue connectors ------------------------------------

/// A Claude-directory entry with no fixed server address: the user's own
/// address must match `url_regex` (Glean, Databricks, Metabase, ...).
fn tenant_descriptor() -> ConnectorDescriptor {
    let item = serde_json::json!({
        "slug": "glean",
        "name": "Glean",
        "remote": {
            "url": null,
            "url_regex": "^https://[^/]+\\.glean\\.com/mcp/[A-Za-z0-9_-]+",
            "transport": "streamable-http",
            "is_authless": false,
            "required_fields": [{"field": "url"}]
        }
    });
    let origin = url::Url::parse("https://api.example.test/servers").unwrap();
    codemason_connectors::ConnectorEntry::from_value(&item, &origin)
        .expect("entry parses")
        .to_catalogue_descriptor()
        .expect("a tenant-hosted entry is a valid catalogue descriptor")
}

#[test]
fn tenant_connector_is_kept_and_reports_its_address_pattern() {
    let descriptor = tenant_descriptor();
    assert_eq!(
        descriptor.server_url_pattern(),
        Some("^https://[^/]+\\.glean\\.com/mcp/[A-Za-z0-9_-]+")
    );
    assert!(
        descriptor.supported_servers().is_empty(),
        "no server is connectable until the address is given"
    );
}

#[test]
fn tenant_connector_add_without_url_names_the_flag() {
    let tree = TempTree::new("tenant-no-url");
    let err = add(
        &dirs(&tree),
        &tree.0,
        "glean",
        false,
        &[tenant_descriptor()],
    )
    .expect_err("refused without an address");
    assert!(matches!(err, AddError::NeedsServerUrl { .. }), "{err:?}");
    assert!(err.to_string().contains("--url"), "{err}");
}

#[test]
fn tenant_connector_add_refuses_an_address_off_the_pattern() {
    let tree = TempTree::new("tenant-bad-url");
    for url in [
        "https://evil.example.com/mcp/x",
        "http://acme.glean.com/mcp/x",
    ] {
        let err = codemason_connectors::add_with_server_url(
            &dirs(&tree),
            &tree.0,
            "glean",
            false,
            &[tenant_descriptor()],
            Some(url),
        )
        .expect_err("refused");
        assert!(matches!(err, AddError::ServerUrl(_)), "{url}: {err:?}");
    }
}

#[test]
fn tenant_connector_add_with_a_matching_url_installs_it_connectable() {
    let tree = TempTree::new("tenant-ok");
    let staged = codemason_connectors::add_with_server_url(
        &dirs(&tree),
        &tree.0,
        "glean",
        false,
        &[tenant_descriptor()],
        Some("https://acme-be.glean.com/mcp/default"),
    )
    .expect("installs");
    let server = &staged.descriptor.servers[0];
    assert_eq!(
        server.url.as_deref(),
        Some("https://acme-be.glean.com/mcp/default")
    );
    assert_eq!(server.url_pattern, None);
    assert_eq!(staged.descriptor.supported_servers().len(), 1);
}

#[test]
fn url_flag_is_refused_for_a_fixed_address_connector() {
    let tree = TempTree::new("tenant-fixed");
    let err = codemason_connectors::add_with_server_url(
        &dirs(&tree),
        &tree.0,
        "echo",
        false,
        &[descriptor_for("echo")],
        Some("https://acme.example.com/mcp"),
    )
    .expect_err("refused");
    assert!(matches!(err, AddError::ServerUrl(_)), "{err:?}");
}
