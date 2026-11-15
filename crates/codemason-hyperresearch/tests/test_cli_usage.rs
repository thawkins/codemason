//! RS-06: `hyperresearch help` must only list subcommands the parser accepts.

use clap::Parser;
use codemason_hyperresearch::cli::args::{Cli, USAGE};

/// Every command name listed under `COMMANDS:` in the usage text.
fn listed_commands() -> Vec<&'static str> {
    USAGE
        .split_once("COMMANDS:")
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

#[test]
fn test_cli_usage_lists_only_parseable_commands() {
    let commands = listed_commands();
    assert!(!commands.is_empty(), "usage lists no commands");
    for cmd in commands {
        assert!(
            Cli::try_parse_from(["hyperresearch", cmd]).is_ok(),
            "usage lists `{cmd}` but the parser rejects it"
        );
    }
}
