//! The interactive frontend `cdmn` opens by default.
//!
//! The desktop GUI is the only interactive UI, so an empty argv selects it,
//! `--gui` is still accepted (and changes nothing), and `--no-tui` keeps the
//! plain stdin/stdout mode. The flags are parsed through the binary's own
//! [`InteractiveArgs`], re-imported from `src/interactive_mode.rs`.

#[path = "../src/interactive_mode.rs"]
mod interactive_mode;

use clap::Parser;
use interactive_mode::{InteractiveArgs, Mode};

/// A parser holding only the interactive flags, flattened as the binary does.
#[derive(Parser)]
#[command(name = "cdmn")]
struct Argv {
    #[command(flatten)]
    interactive: InteractiveArgs,
}

/// Parse `args` (after the program name) into the selected mode.
fn mode_of(args: &[&str]) -> Mode {
    let argv = Argv::try_parse_from(std::iter::once("cdmn").chain(args.iter().copied()))
        .expect("argv parses");
    argv.interactive.mode()
}

#[test]
fn test_cli_default_interactive_mode_is_gui() {
    assert_eq!(mode_of(&[]), Mode::Gui);
}

#[test]
fn test_cli_gui_flag_is_an_accepted_no_op() {
    assert_eq!(mode_of(&["--gui"]), mode_of(&[]));
}

#[test]
fn test_cli_no_tui_selects_plain_mode() {
    assert_eq!(mode_of(&["--no-tui"]), Mode::Plain);
}

/// With no X11 or Wayland display, an interactive `cdmn` exits non-zero with
/// the no-display message (pointing at headless `cdmn run`) and does not
/// panic or fall back to a terminal UI.
#[test]
fn test_cli_interactive_without_display_exits_with_headless_hint() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp")
        .join(format!("cli-no-display-{}", std::process::id()));
    let cwd = scratch.join("cwd");
    std::fs::create_dir_all(&cwd).expect("scratch cwd");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cdmn"))
        .current_dir(&cwd)
        .env("XDG_CONFIG_HOME", scratch.join("config"))
        .env("HOME", scratch.join("home"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("spawn cdmn");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _ = std::fs::remove_dir_all(&scratch); // INTENTIONAL: best-effort scratch cleanup

    assert!(!output.status.success(), "cdmn must fail without a display");
    assert!(
        stderr.contains("no graphical display found") && stderr.contains("cdmn run"),
        "stderr must name the missing display and the headless mode: {stderr}"
    );
    assert!(
        !stderr.contains("panicked") && !stdout.contains("panicked"),
        "cdmn must not panic: {stderr}"
    );
    assert!(
        !stderr.contains("terminal UI"),
        "no terminal UI fallback remains: {stderr}"
    );
}
