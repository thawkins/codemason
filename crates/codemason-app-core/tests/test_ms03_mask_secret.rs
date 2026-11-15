//! MS-03 regression tests for credential masking in the provider dialog.
//!
//! SEC-codemason-tui-002 (SECTASKS T-045): the provider-setup dialog rendered the
//! API key and the GitLab PAT verbatim, so a live credential was readable
//! during a screen share, a terminal recording, or a shoulder-surf.
//!
//! Ported from the removed terminal UI, whose `mask_secret` the GUI provider
//! dialog mirrors: both mask with `codemason_types::sanitize::mask_edges(text,
//! 4, 12)`, so these tests pin that call.

/// The provider dialog's masking rule: keep the first and last four
/// characters of a secret longer than twelve, mask everything else.
fn mask_secret(text: &str) -> String {
    codemason_types::sanitize::mask_edges(text, 4, 12)
}

#[test]
fn test_short_secret_is_fully_masked() {
    assert_eq!(mask_secret("abc"), "***");
    assert_eq!(mask_secret("123456789012"), "************");
    assert_eq!(mask_secret(""), "");
}

#[test]
fn test_long_secret_keeps_only_the_ends() {
    let masked = mask_secret("sk-ant-api03-abcdefghijklmnop");
    assert!(masked.starts_with("sk-a"), "prefix kept: {masked}");
    assert!(masked.ends_with("mnop"), "suffix kept: {masked}");
    assert!(!masked.contains("api03"), "middle must be masked: {masked}");
    assert_eq!(
        masked.chars().count(),
        "sk-ant-api03-abcdefghijklmnop".len()
    );
}

#[test]
fn test_gitlab_pat_is_masked() {
    let masked = mask_secret("glpat-abcdefghijklmnopqrst");
    assert!(
        !masked.contains("abcdefghij"),
        "token body must be hidden: {masked}"
    );
    assert!(masked.starts_with("glpa"));
}

/// Restored from `test_layout_extra.rs::mask_secret_boundaries`.
#[test]
fn test_mask_secret_boundaries() {
    assert_eq!(mask_secret(""), "");
    assert_eq!(mask_secret("123456789012"), "************");
    let masked = mask_secret("sk-abcdefghijklmnop");
    assert!(masked.starts_with("sk-a"));
    assert!(masked.ends_with("mnop"));
}
