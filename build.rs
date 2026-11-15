//! Build script of the `codemason` binary package.
//!
//! Its one job: give the `cdmn` executable a 32 MiB main-thread stack when
//! it targets Windows with the MSVC linker.
//!
//! `cdmn --gui` runs the eframe window on the process main thread (winit
//! requires it on macOS, and the GUI host does the same everywhere), and that
//! thread also drives `App::handle_event` and the other App calls through
//! `Handle::block_on`. On Windows the main thread's stack is fixed at link
//! time, 1 MiB by default, while the program's work thread
//! (`codemason-main`) runs with 32 MiB because that same work once overflowed
//! a smaller stack. Linux and macOS give the main thread 8 MiB, growing on
//! demand, so they need nothing.
//!
//! `cargo:rustc-link-arg-bin=cdmn=...` scopes the flag to the `cdmn` binary:
//! tests, examples and other workspace crates link as before. `/STACK` sets
//! only the reserve of the main thread (pages are committed as used) and the
//! default of threads created with a zero stack size; Rust and tokio threads
//! pass an explicit size, so they are unaffected. The value matches
//! `MAIN_THREAD_STACK_BYTES` in `src/main.rs`.
//!
//! Dependencies: none (std only).

/// The main-thread stack reserve for Windows MSVC builds, in bytes (32 MiB).
const WINDOWS_MAIN_STACK_BYTES: u64 = 32 * 1024 * 1024;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os == "windows" && env == "msvc" {
        println!("cargo:rustc-link-arg-bin=cdmn=/STACK:{WINDOWS_MAIN_STACK_BYTES}");
    }
}
