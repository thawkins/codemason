//! SSRF guard gaps closed by audit TL-07.
//!
//! A trailing dot used to bypass the metadata and DNS-rebinding name checks,
//! the carrier-grade NAT and NAT64 ranges were not blocked, and nothing
//! checked the address a hostname actually resolves to.

#![allow(unsafe_code)] // Rust 2024 `remove_var` for the SSRF bypass guard below.

use codemason_tools_extended::masterfetch::security::{SsrfSafeResolver, validate_url};
use reqwest::dns::Resolve;

const BYPASS: &str = "CODEMASON_TOOLS_EXTENDED_TEST_NO_SSRF";

/// Holds the binary-wide env lock with the test-only SSRF bypass cleared, so
/// these checks run against the real guard; restores the bypass on drop.
struct RealGuard {
    saved: Option<std::ffi::OsString>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl RealGuard {
    fn new() -> Self {
        let lock = crate::global_env_lock();
        let saved = std::env::var_os(BYPASS);
        // SAFETY: the binary-wide env lock is held for the guard's lifetime.
        unsafe { std::env::remove_var(BYPASS) };
        Self { saved, _lock: lock }
    }
}

impl Drop for RealGuard {
    fn drop(&mut self) {
        if let Some(value) = self.saved.take() {
            // SAFETY: still under the env lock held by `_lock`.
            unsafe { std::env::set_var(BYPASS, value) };
        }
    }
}

#[test]
fn test_mf_ssrf_gaps_trailing_dot_does_not_bypass_name_checks() {
    let _real = RealGuard::new();
    assert!(validate_url("http://metadata.google.internal./").is_err());
    assert!(validate_url("http://METADATA.azure.com./x").is_err());
    assert!(validate_url("http://10.0.0.1.nip.io./").is_err());
}

#[test]
fn test_mf_ssrf_gaps_carrier_grade_nat_and_nat64_are_blocked() {
    let _real = RealGuard::new();
    // 100.100.100.200 is Alibaba Cloud's metadata address (100.64.0.0/10).
    assert!(validate_url("http://100.100.100.200/").is_err());
    assert!(validate_url("http://100.64.0.1/").is_err());
    assert!(validate_url("http://100.127.255.254/").is_err());
    // NAT64 of 169.254.169.254.
    assert!(validate_url("http://[64:ff9b::a9fe:a9fe]/").is_err());
    // Neighbours stay allowed.
    assert!(validate_url("http://100.128.0.1/").is_ok());
    assert!(validate_url("http://100.63.255.255/").is_ok());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_mf_ssrf_gaps_resolver_refuses_a_name_that_resolves_to_loopback() {
    let _real = RealGuard::new();
    // `localhost` resolves to loopback on every host; a public name that a
    // DNS record points at a private address is refused the same way.
    let name: reqwest::dns::Name = "localhost".parse().expect("name");
    let result = SsrfSafeResolver.resolve(name).await;
    assert!(result.is_err(), "loopback must not be resolved for a fetch");
}
