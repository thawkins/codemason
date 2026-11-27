//! CO-07: the one credential-prefix list and the log redaction regex agree.

use codemason_types::sanitize::{CREDENTIAL_PREFIXES, redact_secrets};

/// A token tail every arm's character class accepts (upper-case for `AKIA`).
const TAIL: &str = "A1B2C3D4E5F6G7H8J9K0L1M2";

/// Every listed prefix followed by a token is masked in logs, so the list
/// telemetry uses and the redaction regex cannot drift apart again.
#[test]
fn test_credential_prefixes_every_entry_is_redacted() {
    for entry in CREDENTIAL_PREFIXES {
        let secret = format!("{}{TAIL}", entry.prefix);
        let line = format!("value {secret} end");
        let redacted = redact_secrets(&line);
        assert!(
            !redacted.contains(TAIL),
            "prefix {:?} is listed but not redacted: {redacted}",
            entry.prefix
        );
    }
}

/// The prefixes the regex lacked before CO-07 are now redacted.
#[test]
fn test_credential_prefixes_new_regex_arms() {
    for secret in [
        format!("github_pat_{TAIL}"),
        format!("xoxs-{TAIL}"),
        format!("AIza{TAIL}"),
        format!("ya29.{TAIL}"),
        format!("jwt {TAIL}"),
    ] {
        assert!(!redact_secrets(&secret).contains(TAIL), "{secret}");
    }
}
