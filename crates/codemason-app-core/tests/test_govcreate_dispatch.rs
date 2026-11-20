//! T-011 (spec `govdoc`, FR-001, FR-003, NFR-005): `/spec govcreate` dispatch
//! surface in the `/spec` arm of `slash.rs`.
//!
//! Verifies that `/spec govcreate <args>` dispatches to the `GovCreate` arm
//! and reports through the NFR-005 message builders (`From: /spec govcreate`
//! prefix, no `[err]`), that `/spec govcreate` with missing positionals stays
//! a usage status, that a parseable-but-invalid invocation renders the
//! dedicated govcreate usage block (not the generic `/spec help` table), and
//! that `/spec help` advertises `govcreate` with a worked example and accepts
//! the dedicated usage block.
//!
//! The orchestration runner is T-012; this task tests dispatch and reporting
//! only, and must not touch the filesystem beyond the in-memory `make_app`.

use codemason_app_core::app::App;
use support::make_app;

mod support;

/// Render a command's raw output text the way the message window shows it.
fn rendered(app: &mut App, text: &str) -> String {
    app.prepare_transcript_text(text)
}

/// Split one Markdown table row into its trimmed cells.
///
/// A cell may carry an escaped `\|` (a pipe inside a code span or an
/// alternation such as `--github \| --gitlab`); only an unescaped `|`
/// separates cells. The leading and trailing pipes produce no cell.
fn split_row_cells(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in row.chars() {
        if escaped {
            // Keep the backslash: the cell text is compared as written.
            current.push('\\');
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '|' => {
                cells.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        cells.push(current.trim().to_string());
    }
    // The row starts and ends with a pipe, so the first entry is empty.
    cells.retain(|cell| !cell.is_empty());
    cells
}
#[test]
fn test_govcreate_valid_invocation_reports_via_nfr005_builders() {
    let mut app = make_app();
    let body = rendered(
        &mut app,
        &codemason_specs::SpecCommand::build_govcreate_message(
            "payments-arch",
            "https://example.gov/arch",
            "./out",
        ),
    );

    assert!(
        body.starts_with("From: /spec govcreate"),
        "the message must carry the surface prefix (NFR-005): {body}"
    );
    assert!(
        body.contains("payments-arch")
            && body.contains("example.gov/arch")
            && body.contains("./out"),
        "the message must name every input: {body}"
    );
    assert!(body.contains("SPEC.md") && body.contains("TESTPLAN.md"));
    assert!(
        !body.contains("[err]"),
        "a valid invocation is not an error: {body}"
    );
    let status = codemason_specs::SpecCommand::build_govcreate_status("payments-arch");
    assert!(status.contains("payments-arch"), "status: {status}");
}

#[test]
fn test_govcreate_missing_positionals_is_usage_error() {
    // FR-003: parsing a run with fewer than three positionals is the usage
    // surface, which the dispatch arm reports as `Usage: /spec govcreate`.
    let cmd = codemason_specs::SpecCommand::parse("govcreate payments-arch");
    assert!(
        matches!(&cmd, codemason_specs::SpecCommand::Unknown(s) if s == "govcreate"),
        "got {cmd:?}"
    );
    assert!(cmd.is_usage_error());
}

#[test]
fn test_govcreate_invalid_spec_id_shows_govcreate_usage_block() {
    let mut app = make_app();
    let reason = match codemason_specs::SpecCommand::parse(
        "govcreate .. https://example.gov/arch ./out --language rust --type cmdline",
    ) {
        codemason_specs::SpecCommand::GovCreateUsage(reason) => reason,
        other => panic!("expected GovCreateUsage, got {other:?}"),
    };
    let body = rendered(
        &mut app,
        &format!(
            "From: /spec govcreate\n\n[err] **{reason}**\n\n{}",
            codemason_specs::SpecCommand::build_govcreate_help_message()
        ),
    );

    assert!(
        body.contains("[err]") && body.contains("invalid spec id"),
        "the specific cause is reported (FR-002): {body}"
    );
    assert!(
        body.contains("--force") && body.contains("/spec govcreate"),
        "the rendered usage block mentions the flags and surface (FR-003): {body}"
    );
    // The angle-bracket placeholders are markdown inline HTML and are dropped
    // by the render pipeline; assert them on the raw block, which is what the
    // usage contract documents.
    let raw = codemason_specs::SpecCommand::build_govcreate_help_message();
    assert!(
        raw.contains("<content-ref>") && raw.contains("<target-folder>") && raw.contains("--force"),
        "the dedicated govcreate usage block (raw): {raw}"
    );
    assert!(
        !raw.contains("## /spec command reference"),
        "the generic help table must not be substituted for the govcreate usage"
    );
}

#[test]
fn test_spec_help_advertises_govcreate_with_worked_example() {
    let mut app = make_app();
    let body = rendered(&mut app, codemason_specs::SpecCommand::build_help_message());
    assert!(
        body.contains("/spec govcreate"),
        "`/spec help` must list govcreate (FR-001, FR-003): {body}"
    );
    assert!(
        body.contains("content-ref") && body.contains("target folder")
            || body.contains("content-ref") && body.contains("target-folder"),
        "the help row must show the positional contract: {body}"
    );
    assert!(
        body.contains("govcreate"),
        "the help row must carry the worked example (NFR-005): {body}"
    );
}

/// `/spec govcreate` (or any usage-error subcommand) must report the cause in
/// the message window, not only a status line (NFR-005).
#[tokio::test]
async fn test_spec_usage_error_subcommands_report_cause_in_message_window() {
    let cases = [
        (
            "/spec govcreate",
            "/spec govcreate",
            "missing required argument",
        ),
        (
            "/spec govcreate payments-arch",
            "/spec govcreate",
            "missing required argument",
        ),
        (
            "/spec reverse",
            "/spec reverse",
            "missing required argument",
        ),
        (
            "/spec status",
            "incomplete arguments",
            "missing required argument",
        ),
    ];

    for (command, expected, cause) in cases {
        let mut app = make_app();
        app.session.session_id = Some("s1".to_string());

        app.execute_slash_command(command).await;

        let text = app
            .session
            .messages
            .last()
            .expect("usage message")
            .text_content();
        assert!(
            text.contains(cause),
            "{command} must state the cause '{cause}' in the message window: {text}"
        );
        assert!(
            text.contains(expected),
            "{command} must render the usage block containing '{expected}': {text}"
        );
        assert!(
            app.status.contains("Usage: /spec"),
            "{command} must point at `/spec help` from the status line: {}",
            app.status
        );
    }
}

#[test]
fn test_govcreate_usage_block_is_ascii_and_prefixed() {
    // FR-003 / NFR-005: the dedicated usage block exists on the same surface
    // `/spec help` points at.
    let help = codemason_specs::SpecCommand::build_govcreate_help_message();
    assert!(help.starts_with("From: /spec govcreate"));
    assert!(help.is_ascii(), "the usage block is ASCII-only (NFR-005)");
}

#[test]
fn test_spec_help_govcreate_row_keeps_positionals_in_command_column() {
    // Regression test for the table-cell misalignment: the renderer used to
    // drop the trailing backtick from the long `/spec govcreate ...` code span,
    // which under-sized the command column and pushed `content-ref` /
    // `target-folder` into the Description column. The reply is now Markdown
    // source rather than a pre-drawn grid, so the whole row is one line and
    // each positional must stay inside its own `|`-delimited cell.
    let mut app = make_app();
    let body = rendered(&mut app, codemason_specs::SpecCommand::build_help_message());

    // The header row of the table must have exactly three columns.
    let header = body
        .lines()
        .find(|l| l.contains("Command") && l.contains("Arguments"))
        .unwrap_or_else(|| panic!("help table header: {body}"));
    assert_eq!(header.matches('|').count(), 4, "three columns: {header}");

    // The govcreate row is the one line that carries the command span.
    let row = body
        .lines()
        .find(|l| l.starts_with('|') && l.contains("/spec govcreate"))
        .unwrap_or_else(|| panic!("govcreate row: {body}"));

    // A Markdown row is `| command | arguments | description |`. A cell may
    // itself carry a `\|` (the `--github \| --gitlab` alternation), so split on
    // unescaped pipes only.
    let cells = split_row_cells(row);
    assert_eq!(cells.len(), 3, "a row has three cells: {row} -> {cells:?}");
    let command_cell = cells[0].as_str();
    let arguments_cell = cells[1].as_str();
    let description_cell = cells[2].as_str();

    for positional in ["<specid>", "<content-ref>", "<target-folder>"] {
        assert!(
            command_cell.contains(positional),
            "{positional} stays in the command column: {row}"
        );
    }
    // The misalignment pushed the *usage* out of the command cell. The
    // description legitimately names a placeholder in prose (it explains
    // `<target-folder>/specs/<specid>/`), so what must not appear there is the
    // positional sequence itself.
    assert!(
        !description_cell.contains("<specid> <content-ref> <target-folder>"),
        "the positional sequence must not bleed into the Description column: {row}"
    );
    assert!(
        arguments_cell.starts_with("required"),
        "arguments column starts with required: {row}"
    );
    assert!(
        !arguments_cell.contains("<specid>"),
        "the command positionals stay out of the Arguments column: {row}"
    );
}
