//! Tests that `XDG_CONFIG_HOME` selects the global config directory on every
//! platform.
//!
//! `dirs::config_dir()` honours `XDG_CONFIG_HOME` only on Linux; on Windows and
//! macOS it returns the platform directory regardless, so a test that isolated
//! the global config through `XDG_CONFIG_HOME` used to write into the real
//! user's `%APPDATA%\codemason\codemason.json`.
//!
//! Note: `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024; the
//! workspace denies `unsafe_code`, so this test target opts back in.

#![allow(unsafe_code)]

use std::path::PathBuf;

use codemason_config::Config;

/// Restores `XDG_CONFIG_HOME` to its value at construction when dropped.
struct XdgGuard(Option<std::ffi::OsString>);

impl XdgGuard {
    fn new() -> Self {
        Self(std::env::var_os("XDG_CONFIG_HOME"))
    }
}

impl Drop for XdgGuard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(prev) => unsafe { std::env::set_var("XDG_CONFIG_HOME", prev) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
    }
}

/// An absolute scratch directory under the git-ignored `target/temp` tree.
fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("temp")
        .join("global-config-dir-xdg")
        .join(name)
}

#[test]
#[serial_test::serial]
fn test_global_config_dir_honours_absolute_xdg_config_home() {
    let _guard = XdgGuard::new();
    let base = scratch("absolute");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &base) };

    assert_eq!(Config::global_config_dir(), Some(base.join("codemason")));
    assert_eq!(
        Config::global_config_path(),
        Some(base.join("codemason").join("codemason.json"))
    );
}

#[test]
#[serial_test::serial]
fn test_global_config_dir_ignores_blank_xdg_config_home() {
    let _guard = XdgGuard::new();
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    let platform = Config::global_config_dir();

    unsafe { std::env::set_var("XDG_CONFIG_HOME", "   ") };
    assert_eq!(Config::global_config_dir(), platform);
}

#[test]
#[serial_test::serial]
fn test_global_config_dir_ignores_relative_xdg_config_home() {
    // The XDG base-directory spec says a relative value is invalid and must be
    // ignored; a relative value would otherwise move the global config with the
    // process working directory.
    let _guard = XdgGuard::new();
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
    let platform = Config::global_config_dir();

    unsafe { std::env::set_var("XDG_CONFIG_HOME", "relative/config") };
    assert_eq!(Config::global_config_dir(), platform);
}
