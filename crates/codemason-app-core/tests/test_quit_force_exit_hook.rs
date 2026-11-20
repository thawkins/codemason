//! The quit safety net's pre-exit hook.
//!
//! `arm_force_exit` ends the process with `process::exit`, which skips the
//! binary's `clean exit` crash-marker stamp; the hook registered with
//! `set_force_exit_hook` runs first so the marker can be stamped. Ending the
//! process cannot be asserted in-process, so the hook is run directly.

use std::sync::atomic::{AtomicUsize, Ordering};

use codemason_app_core::app::quit::{run_force_exit_hook, set_force_exit_hook};

static CALLS: AtomicUsize = AtomicUsize::new(0);

fn count_call() {
    CALLS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn test_quit_force_exit_hook_runs_the_registered_hook() {
    set_force_exit_hook(count_call);
    let before = CALLS.load(Ordering::SeqCst);
    run_force_exit_hook();
    assert_eq!(CALLS.load(Ordering::SeqCst), before + 1);
}
