//! Compile-time checks that the state moved into `codemason-app-core` is the
//! same type the terminal frontend re-exports under its old paths.
//!
//! Lives in `codemason-tui` rather than `codemason-app-core` because a
//! dev-dependency on `codemason-tui` from `codemason-app-core` would close a
//! dependency cycle (`codemason-tui` depends on `codemason-app-core`).

/// Accepts only the core `App`; calling it with the tui path proves identity.
fn same_app(_: &codemason_app_core::app::App) {}

/// Accepts only the core `StartupSession`.
fn same_startup(_: Option<&codemason_app_core::app::StartupSession>) {}

/// Accepts only the core `ThemeMode`.
fn same_theme_mode(_: codemason_app_core::theme_mode::ThemeMode) {}

#[test]
fn test_app_core_moved_app_is_reexported() {
    // Building an `App` needs a live runtime; coercing `same_app` to a
    // function over the tui path type-checks only if the two types are one.
    let _check: fn(&codemason_app_core::app::App) = same_app;
    same_startup(None::<&codemason_app_core::app::StartupSession>);
    same_theme_mode(codemason_app_core::theme_mode::ThemeMode::default());
    assert!(
        std::any::type_name::<codemason_app_core::app::App>().starts_with("codemason_app_core::")
    );
}
