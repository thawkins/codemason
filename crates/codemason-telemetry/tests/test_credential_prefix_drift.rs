//! CO-07: telemetry flags the token types log redaction knew and it did not.

use codemason_telemetry::sensitive::looks_sensitive;

/// GitLab, Hugging Face, npm and GitHub user-to-server tokens are flagged.
#[test]
fn test_credential_prefix_drift_new_tokens_are_sensitive() {
    for token in [
        "glpat-abcdefghijklmnopqrst",
        "glrt-abcdefghijklmnopqrst",
        "hf_abcdefghijklmnopqrstuv",
        "npm_abcdefghijklmnopqrstuv",
        "ghu_abcdefghijklmnopqrstuv",
        "AIzaSyA1b2C3d4E5f6",
        "ya29.a0AfH6SMB",
        "github_pat_11ABCDEF",
        "xoxs-1234",
    ] {
        assert!(looks_sensitive(token), "{token} should be sensitive");
    }
}

/// A short name that only shares a new prefix stays visible: the new
/// entries need 16 characters after the prefix.
#[test]
fn test_credential_prefix_drift_tool_names_are_not_sensitive() {
    for name in ["npm_install", "hf_search", "ghu_lookup", "glpat-help"] {
        assert!(!looks_sensitive(name), "{name} should not be sensitive");
    }
}
