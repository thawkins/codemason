//! Launch-path tests for the GUI frontend: the no-display guard and the
//! first render of the shell over an empty session.
//!
//! GUI assertions use egui_kittest accessibility queries only (no wgpu
//! snapshots), so no GPU or display is needed.

// Rust 2024 makes `std::env::set_var`/`remove_var` unsafe; this is the one
// sanctioned test use (AGENTS-LANGUAGE.md, Core Code Rules).
#![allow(unsafe_code)]

#[path = "support/mod.rs"]
mod support;

use codemason_gui::{GuiLaunchError, Shell, check_display};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Restores the display variables a test removed, even if it panics.
struct DisplayEnvRestore {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Drop for DisplayEnvRestore {
    fn drop(&mut self) {
        for (name, value) in &self.saved {
            match value {
                // SAFETY: the caller holds `crate::global_env_lock()`.
                Some(v) => unsafe { std::env::set_var(name, v) },
                // SAFETY: the caller holds `crate::global_env_lock()`.
                None => unsafe { std::env::remove_var(name) },
            }
        }
    }
}

#[test]
fn test_gui_launch_no_display_returns_error() {
    let _guard = crate::global_env_lock();
    let names = ["DISPLAY", "WAYLAND_DISPLAY"];
    let _restore = DisplayEnvRestore {
        saved: names.iter().map(|n| (*n, std::env::var_os(n))).collect(),
    };
    for name in names {
        // SAFETY: `_guard` serialises every env mutation in this binary.
        unsafe { std::env::remove_var(name) };
    }

    let result = check_display();

    if cfg!(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )) {
        assert!(
            matches!(result, Err(GuiLaunchError::NoDisplay)),
            "expected NoDisplay, got {result:?}"
        );
    } else {
        assert!(
            result.is_ok(),
            "non-X11/Wayland platforms need no display variable"
        );
    }
}

#[tokio::test]
async fn test_shell_renders_empty_session() {
    let shell = Shell::new(support::make_app());
    let harness = Harness::new_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);

    harness.get_by_label("Prompt");
}
