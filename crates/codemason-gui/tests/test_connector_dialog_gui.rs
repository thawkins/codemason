//! Tests for the Connectors dialog (`/connectors claude`,
//! `codemason_gui::dialogs::stores::connector`): no Codex | Claude
//! switcher, a row's sign-in badge and server-count chip, and the details
//! pane's Sign-in, Requested scopes, Credential and MCP servers sections.
//!
//! Each test draws `show_connector_store` over an App whose
//! `CatalogueBrowser` is filled directly, in an egui_kittest harness, so no
//! catalogue endpoint is contacted.

use std::collections::BTreeMap;

use codemason_app_core::app::App;
use codemason_connectors::{
    CatalogueBrowser, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};
use codemason_gui::dialogs::stores::connector::{auth_badge, auth_sentence, server_count_label};
use codemason_gui::dialogs::stores::{CONNECTORS_TITLE, DETAILS_LABEL, show_connector_store};
use codemason_gui::material::badge::BadgeTone;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

#[path = "support_material.rs"]
#[allow(dead_code)] // Only its harness builder is used here.
mod support_material;

/// An env value no rendered text may contain.
const ENV_SECRET: &str = "env-value-s3cret";

/// A header value no rendered text may contain.
const HEADER_SECRET: &str = "header-value-s3cret";

/// The window the tests draw in.
const WIDE: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// A stdio server `id` running `npx -y pkg` with a secret env value.
fn stdio_server(id: &str) -> ConnectorServer {
    ConnectorServer {
        id: id.to_string(),
        transport: "stdio".to_string(),
        command: Some("npx".to_string()),
        args: vec!["-y".to_string(), "pkg".to_string()],
        env: BTreeMap::from([("API_KEY".to_string(), ENV_SECRET.to_string())]),
        url: None,
        headers: BTreeMap::new(),
    }
}

/// An http server `id` at `https://mcp.example.org/{id}` with a secret
/// header value.
fn http_server(id: &str) -> ConnectorServer {
    ConnectorServer {
        id: id.to_string(),
        transport: "http".to_string(),
        command: None,
        args: Vec::new(),
        env: BTreeMap::new(),
        url: Some(format!("https://mcp.example.org/{id}")),
        headers: BTreeMap::from([("Authorization".to_string(), HEADER_SECRET.to_string())]),
    }
}

/// A connector `id` in category `developer` signing in with `auth`.
fn descriptor(id: &str, auth: ConnectorAuthShape) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: format!("{id} description"),
        category: "developer".to_string(),
        tags: vec!["git".to_string()],
        source: format!("https://example.org/{id}.json"),
        marketplace: "acme".to_string(),
        provenance: Default::default(),
        auth,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![stdio_server("main")],
        unsupported: Vec::new(),
    }
}

/// An OAuth connector with two servers, two scopes and a credential name.
fn oauth_two_servers() -> ConnectorDescriptor {
    let mut github = descriptor("github", ConnectorAuthShape::Oauth);
    github.auth_scope = vec!["repo".to_string(), "read:org".to_string()];
    github.credential = Some("GITHUB_TOKEN".to_string());
    github.servers = vec![stdio_server("local"), http_server("remote")];
    github
}

/// An App with the catalogue open over `entries`.
fn app_with(entries: Vec<ConnectorDescriptor>) -> App {
    let mut app = support::make_app();
    // Fill the browser directly: opening the real catalogue would start a live
    // fetch thread on an empty cache.
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(entries, 0);
    app.connector_store = Some(browser);
    app
}

/// A harness drawing the Connectors dialog over `app`.
fn rig(app: App) -> Harness<'static, App> {
    let mut harness = support_material::builder().with_size(WIDE).build_ui_state(
        |ui, app: &mut App| {
            show_connector_store(ui.ctx(), app);
        },
        app,
    );
    harness.run();
    harness
}

/// Whether a node labelled exactly `label` lies inside the details pane.
fn in_details(harness: &Harness<'static, App>, label: &str) -> bool {
    let details = harness
        .get_by_role_and_label(Role::Group, DETAILS_LABEL)
        .rect();
    harness
        .query_all_by_label(label)
        .any(|node| details.contains_rect(node.rect()))
}

#[test]
fn test_connector_dialog_connector_dialog_has_no_switcher() {
    let harness = rig(app_with(vec![oauth_two_servers()]));
    harness.get_by_role_and_label(Role::Dialog, CONNECTORS_TITLE);
    for kind in ["Codex", "Claude"] {
        assert!(
            harness
                .query_by_role_and_label(Role::RadioButton, kind)
                .is_none(),
            "no {kind} switcher segment"
        );
    }
}

#[test]
fn test_connector_dialog_row_shows_auth_badge_and_server_count() {
    let harness = rig(app_with(vec![
        oauth_two_servers(),
        descriptor("slack", ConnectorAuthShape::Token),
    ]));
    let row = harness
        .get_by_role_and_label(Role::ListItem, "github")
        .rect();
    let in_row = |label: &str| {
        harness
            .query_all_by_label(label)
            .any(|node| row.contains_rect(node.rect()))
    };
    assert!(in_row("OAuth"), "the OAuth sign-in badge is in the row");
    assert!(in_row("2 servers"), "the server-count chip is in the row");
    assert!(in_row("developer"), "the category is the row's subtitle");
    let slack = harness
        .get_by_role_and_label(Role::ListItem, "slack")
        .rect();
    assert!(
        harness
            .query_all_by_label("1 server")
            .any(|node| slack.contains_rect(node.rect())),
        "one server reads 1 server"
    );
}

#[test]
fn test_connector_dialog_details_show_scopes_credential_servers() {
    let harness = rig(app_with(vec![oauth_two_servers()]));
    for label in [
        "Sign-in",
        "You'll sign in in your browser after installing",
        "Requested scopes",
        "repo",
        "read:org",
        "Credential",
        "GITHUB_TOKEN",
        "MCP servers",
        "local",
        "stdio",
        "npx -y pkg",
        "remote",
        "http",
        "https://mcp.example.org/remote",
        "Tags",
        "Source",
        "https://example.org/github.json",
    ] {
        assert!(in_details(&harness, label), "{label} is in the details");
    }
    let order: Vec<f32> = ["Sign-in", "Requested scopes", "Credential", "MCP servers"]
        .iter()
        .map(|label| harness.get_by_label(label).rect().top())
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "sections in order: {order:?}"
    );
}

#[test]
fn test_connector_dialog_scopes_hidden_when_empty() {
    let harness = rig(app_with(vec![descriptor(
        "slack",
        ConnectorAuthShape::Token,
    )]));
    assert!(in_details(&harness, "Sign-in"));
    assert!(
        harness.query_by_label("Requested scopes").is_none(),
        "no scopes section without scopes"
    );
    assert!(
        harness.query_by_label("Credential").is_none(),
        "no credential section without a credential"
    );
}

#[test]
fn test_connector_dialog_credential_value_never_shown() {
    let harness = rig(app_with(vec![oauth_two_servers()]));
    assert!(in_details(&harness, "GITHUB_TOKEN"));
    for secret in [ENV_SECRET, HEADER_SECRET, "API_KEY", "Authorization"] {
        assert!(
            harness.query_by_label_contains(secret).is_none(),
            "{secret} is never rendered"
        );
    }
}

#[test]
fn test_connector_dialog_each_auth_shape_has_sentence() {
    let cases = [
        (
            ConnectorAuthShape::None,
            "No sign-in",
            BadgeTone::Neutral,
            "No sign-in needed",
        ),
        (
            ConnectorAuthShape::Env,
            "Env",
            BadgeTone::Primary,
            "Reads its credential from an environment variable",
        ),
        (
            ConnectorAuthShape::Token,
            "Token",
            BadgeTone::Secondary,
            "Needs an API token, kept in the credential store",
        ),
        (
            ConnectorAuthShape::Oauth,
            "OAuth",
            BadgeTone::Tertiary,
            "You'll sign in in your browser after installing",
        ),
        (
            ConnectorAuthShape::Password,
            "Password",
            BadgeTone::Error,
            "Needs a username and password",
        ),
    ];
    for (shape, label, tone, sentence) in cases {
        assert_eq!(auth_badge(shape), (label, tone), "{shape:?} badge");
        assert_eq!(auth_sentence(shape), sentence, "{shape:?} sentence");
        let harness = rig(app_with(vec![descriptor("conn", shape)]));
        assert!(in_details(&harness, sentence), "{shape:?} drawn sentence");
        assert!(in_details(&harness, label), "{shape:?} drawn badge");
    }
    assert_eq!(server_count_label(1), "1 server");
    assert_eq!(server_count_label(3), "3 servers");
}
