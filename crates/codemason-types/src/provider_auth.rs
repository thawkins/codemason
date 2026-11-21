//! Read access to stored provider credentials (#10, LC-08).
//!
//! `codemason-llm` needs one thing from the credential store: the API key
//! saved for a provider id. Depending on this trait instead of
//! `codemason_storage::Storage` keeps the `llm -> storage` crate edge out of
//! the graph; `codemason-storage` implements it for `Storage`.

/// A source of stored provider API keys.
pub trait ProviderAuthSource: Send + Sync {
    /// Return the API key stored for `provider_id`.
    ///
    /// # Arguments
    ///
    /// * `provider_id` - the provider identifier, e.g. `"openrouter"`.
    ///
    /// # Returns
    ///
    /// `Ok(Some(key))` when a key is stored, `Ok(None)` when none is.
    ///
    /// # Errors
    ///
    /// Returns an error when the store cannot be read or a stored value
    /// cannot be decoded.
    fn get_provider_auth(&self, provider_id: &str) -> anyhow::Result<Option<String>>;
}
