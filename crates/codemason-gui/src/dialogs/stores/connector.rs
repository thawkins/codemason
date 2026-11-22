//! The Connectors dialog's own parts (`/connectors claude`): its copy, a
//! row's sign-in badge and server-count chip, and the connector detail
//! sections (marketplace, Sign-in with its badge and sentence, Requested
//! scopes, Credential, MCP servers, Tags and Source).
//!
//! A credential is shown by name only, and a server by its id, transport
//! and URL (or command and arguments); env and header values are never
//! drawn.
//!
//! Dependencies: `codemason-connectors` (`ConnectorAuthShape`), `egui`,
//! this crate's `material` components and the shared [`super::frame`].

use codemason_connectors::ConnectorAuthShape;
use egui::{Label, RichText};

use super::frame::{Look, detail_field, detail_heading};
use super::snapshot::{Detail, RowSnapshot, ServerSnapshot};
use crate::material::badge::{BadgeTone, badge};
use crate::material::card::{CardKind, card_frame};
use crate::material::chip::{ChipKind, chip_dense};
use crate::metrics::space;
use crate::palette::{Palette, rgb};

/// The Connectors dialog's copy and parts.
pub const LOOK: Look = Look {
    search_placeholder: "Search connectors...",
    loading: "Loading the catalogue...",
    empty: "The catalogue has no entries",
    row_extras,
    details,
};

/// The sign-in badge of an auth shape.
///
/// # Arguments
///
/// * `auth` - how the connector signs in.
///
/// # Returns
///
/// The badge's label and tone.
#[must_use]
pub const fn auth_badge(auth: ConnectorAuthShape) -> (&'static str, BadgeTone) {
    match auth {
        ConnectorAuthShape::None => ("No sign-in", BadgeTone::Neutral),
        ConnectorAuthShape::Env => ("Env", BadgeTone::Primary),
        ConnectorAuthShape::Token => ("Token", BadgeTone::Secondary),
        ConnectorAuthShape::Oauth => ("OAuth", BadgeTone::Tertiary),
        ConnectorAuthShape::Password => ("Password", BadgeTone::Error),
    }
}

/// The plain sentence the Sign-in section shows for an auth shape.
///
/// # Arguments
///
/// * `auth` - how the connector signs in.
///
/// # Returns
///
/// One sentence saying what signing in takes.
#[must_use]
pub const fn auth_sentence(auth: ConnectorAuthShape) -> &'static str {
    match auth {
        ConnectorAuthShape::None => "No sign-in needed",
        ConnectorAuthShape::Env => "Reads its credential from an environment variable",
        ConnectorAuthShape::Token => "Needs an API token, kept in the credential store",
        ConnectorAuthShape::Oauth => "You'll sign in in your browser after installing",
        ConnectorAuthShape::Password => "Needs a username and password",
    }
}

/// The badge a tenant-hosted connector shows: it installs only with the
/// user's own server address.
pub const NEEDS_SERVER_URL: &str = "Needs server URL";

/// The sentence the details pane shows for a tenant-hosted connector.
pub const NEEDS_SERVER_URL_SENTENCE: &str =
    "Runs on your organisation's own server. Install asks for its address, which must match:";

/// The server-count chip's label.
///
/// # Arguments
///
/// * `count` - how many MCP servers the connector runs.
///
/// # Returns
///
/// `"1 server"`, or `"{count} servers"`.
#[must_use]
pub fn server_count_label(count: usize) -> String {
    if count == 1 {
        "1 server".to_string()
    } else {
        format!("{count} servers")
    }
}

/// Draw the sign-in badge of `auth`.
fn auth_badge_widget(ui: &mut egui::Ui, palette: &Palette, auth: ConnectorAuthShape) {
    let (label, tone) = auth_badge(auth);
    badge(ui, palette, tone, label);
}

/// Draw a row's sign-in badge and server-count chip.
///
/// # Arguments
///
/// * `ui` - the row's meta line, a horizontal ui.
/// * `palette` - the colours.
/// * `row` - the row; a plugin row draws nothing.
pub fn row_extras(ui: &mut egui::Ui, palette: &Palette, row: &RowSnapshot) {
    let detail = match &row.detail {
        Detail::Connector(detail) => detail,
        Detail::Plugin(_) => return,
    };
    auth_badge_widget(ui, palette, detail.auth);
    if detail.server_url_pattern.is_some() {
        badge(ui, palette, BadgeTone::Secondary, NEEDS_SERVER_URL);
    }
    chip_dense(
        ui,
        palette,
        ChipKind::Assist,
        None,
        &server_count_label(detail.servers.len()),
    );
}

/// Draw a connector's detail sections.
///
/// # Arguments
///
/// * `ui` - the details pane's ui.
/// * `palette` - the colours.
/// * `row` - the highlighted row; a plugin row draws nothing.
pub fn details(ui: &mut egui::Ui, palette: &Palette, row: &RowSnapshot) {
    let detail = match &row.detail {
        Detail::Connector(detail) => detail,
        Detail::Plugin(_) => return,
    };
    let text = rgb(palette.m3.on_surface);
    detail_field(ui, palette, "Marketplace", &detail.marketplace);
    if let Some(pattern) = &detail.server_url_pattern {
        detail_heading(ui, palette, "Server address");
        badge(ui, palette, BadgeTone::Secondary, NEEDS_SERVER_URL);
        ui.add(Label::new(RichText::new(NEEDS_SERVER_URL_SENTENCE).color(text)).wrap());
        ui.add(
            Label::new(
                RichText::new(pattern)
                    .monospace()
                    .color(rgb(palette.m3.on_surface_variant)),
            )
            .wrap(),
        );
    }
    detail_heading(ui, palette, "Sign-in");
    auth_badge_widget(ui, palette, detail.auth);
    ui.add(Label::new(RichText::new(auth_sentence(detail.auth)).color(text)).wrap());
    if !detail.scopes.is_empty() {
        detail_heading(ui, palette, "Requested scopes");
        ui.horizontal_wrapped(|ui| {
            for scope in &detail.scopes {
                chip_dense(ui, palette, ChipKind::Assist, None, scope);
            }
        });
    }
    if let Some(credential) = &detail.credential {
        detail_field(ui, palette, "Credential", credential);
    }
    if !detail.servers.is_empty() {
        detail_heading(ui, palette, "MCP servers");
        for server in &detail.servers {
            server_card(ui, palette, server);
        }
    }
    if !row.tags.is_empty() {
        detail_heading(ui, palette, "Tags");
        ui.horizontal_wrapped(|ui| {
            for tag in &row.tags {
                chip_dense(ui, palette, ChipKind::Assist, None, tag);
            }
        });
    }
    detail_field(ui, palette, "Source", &detail.source);
}

/// One MCP server as an outlined card: its id and transport chip over its
/// target.
fn server_card(ui: &mut egui::Ui, palette: &Palette, server: &ServerSnapshot) {
    let text = rgb(palette.m3.on_surface);
    card_frame(CardKind::Outlined, palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = space::XS;
        ui.horizontal(|ui| {
            ui.label(RichText::new(&server.id).color(text).strong());
            chip_dense(ui, palette, ChipKind::Assist, None, &server.transport);
        });
        if !server.target.trim().is_empty() {
            ui.add(
                Label::new(
                    RichText::new(&server.target)
                        .monospace()
                        .color(rgb(palette.m3.on_surface_variant)),
                )
                .wrap(),
            );
        }
    });
}
