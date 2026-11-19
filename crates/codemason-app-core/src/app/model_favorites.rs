//! Favourite and recent models.
//!
//! Favourites are `provider/id` strings the user marks in the model picker
//! (Ctrl+F toggles the highlighted model); the picker lists them first.
//! Recent models are the last [`RECENT_MODELS_MAX`] distinct models chosen in
//! the picker, newest first; the `model.cycle_recent` action (F2) switches to
//! the next one after the current model.
//!
//! Both lists live in the storage settings table as JSON, under
//! [`FAVORITE_MODELS_KEY`] and [`RECENT_MODELS_KEY`], next to
//! `selected_model`. A setting that fails to load or parse reads as empty; a
//! failed save is logged and the in-memory selection still applies.
//!
//! Depends on the App's storage and `finalize_model_selection`.

use serde::{Deserialize, Serialize};

use crate::app::state::{App, LogLevel, ModelPickerEntry};

/// Storage setting holding the favourite models (JSON array of
/// `provider/id`).
pub const FAVORITE_MODELS_KEY: &str = "favorite_models";

/// Storage setting holding the recent models (JSON array of
/// [`RecentModel`]).
pub const RECENT_MODELS_KEY: &str = "recent_models";

/// Most recent models kept, and cycled by F2.
pub const RECENT_MODELS_MAX: usize = 5;

/// A model chosen in the picker, as remembered for F2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentModel {
    /// The provider id (e.g. `anthropic`).
    pub provider_id: String,
    /// The provider's display name.
    pub provider_name: String,
    /// The model id.
    pub id: String,
    /// The model's display name.
    pub name: String,
    /// The model's context window, in tokens.
    pub context_window: usize,
}

impl RecentModel {
    /// The `provider/id` form `selected_model` uses.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider_id, self.id)
    }
}

/// The `provider/id` key of a picker entry.
fn entry_key(entry: &ModelPickerEntry) -> String {
    format!("{}/{}", entry.provider_id, entry.id)
}

impl App {
    /// Read a JSON list setting; a missing or unreadable one is empty.
    fn read_list_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Vec<T> {
        match self.storage.get_setting(key) {
            Ok(Some(text)) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!(key, error = %e, "ignoring unparsable model list setting");
                Vec::new()
            }),
            Ok(None) => Vec::new(),
            Err(e) => {
                tracing::warn!(key, error = %e, "failed to read model list setting");
                Vec::new()
            }
        }
    }

    /// Save a JSON list setting, logging a failure.
    fn write_list_setting<T: Serialize>(&mut self, key: &str, list: &[T]) {
        let saved = serde_json::to_string(list)
            .map_err(|e| e.to_string())
            .and_then(|json| {
                self.storage
                    .set_setting(key, &json)
                    .map_err(|e| e.to_string())
            });
        if let Err(e) = saved {
            tracing::warn!(key, error = %e, "failed to save model list setting");
            self.push_log_no_agent(LogLevel::Warn, format!("Failed to save '{key}': {e}"));
        }
    }

    /// The favourite models, as `provider/id` strings.
    #[must_use]
    pub fn favorite_models(&self) -> Vec<String> {
        self.read_list_setting(FAVORITE_MODELS_KEY)
    }

    /// Mark or unmark a model as a favourite and save the list.
    ///
    /// # Arguments
    ///
    /// * `provider_id` - the model's provider.
    /// * `model_id` - the model's id.
    ///
    /// # Returns
    ///
    /// `true` when the model is now a favourite.
    pub fn toggle_favorite_model(&mut self, provider_id: &str, model_id: &str) -> bool {
        let key = format!("{provider_id}/{model_id}");
        let mut favorites = self.favorite_models();
        let now_favorite = if let Some(at) = favorites.iter().position(|f| *f == key) {
            favorites.remove(at);
            false
        } else {
            favorites.push(key);
            true
        };
        self.write_list_setting(FAVORITE_MODELS_KEY, &favorites);
        now_favorite
    }

    /// Move the favourite models to the front of `entries`, keeping the
    /// order within favourites and within the rest.
    pub(crate) fn sort_favorites_first(&self, entries: &mut [ModelPickerEntry]) {
        let favorites = self.favorite_models();
        if favorites.is_empty() {
            return;
        }
        entries.sort_by_key(|e| !favorites.contains(&entry_key(e)));
    }

    /// The recent models, newest first.
    #[must_use]
    pub fn recent_models(&self) -> Vec<RecentModel> {
        self.read_list_setting(RECENT_MODELS_KEY)
    }

    /// Put a chosen model at the front of the recent list, keeping at most
    /// [`RECENT_MODELS_MAX`].
    pub(crate) fn remember_recent_model(
        &mut self,
        provider_id: &str,
        provider_name: &str,
        entry: &ModelPickerEntry,
    ) {
        let model = RecentModel {
            provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(),
            id: entry.id.clone(),
            name: entry.name.clone(),
            context_window: entry.context_window,
        };
        let mut recent = self.recent_models();
        recent.retain(|r| r.key() != model.key());
        recent.insert(0, model);
        recent.truncate(RECENT_MODELS_MAX);
        self.write_list_setting(RECENT_MODELS_KEY, &recent);
    }

    /// Switch to the recent model after the current one (wrapping), as F2
    /// does, skipping any whose provider is no longer configured or no longer
    /// lists the model. The recent list keeps its order, so repeated presses
    /// walk it.
    ///
    /// # Returns
    ///
    /// `true` when the model changed; `false` (with a status message) when
    /// there is no other usable recent model.
    pub fn cycle_recent_model(&mut self) -> bool {
        let recent = self.recent_models();
        let current = self.session.selected_model.clone().unwrap_or_default();
        let start = recent
            .iter()
            .position(|r| r.key() == current)
            .map_or(0, |at| at + 1);
        let configured: Vec<String> = Self::get_configured_providers(&self.storage)
            .into_iter()
            .map(|p| p.id)
            .collect();
        let usable = (0..recent.len())
            .map(|step| &recent[(start + step) % recent.len()])
            .filter(|r| r.key() != current && configured.contains(&r.provider_id))
            .find_map(|r| {
                self.models_for_provider(&r.provider_id)
                    .into_iter()
                    .find(|e| e.id == r.id)
                    .map(|entry| (r.clone(), entry))
            });
        let Some((target, entry)) = usable else {
            self.status = "model: no other recent model".to_string();
            self.needs_redraw = true;
            return false;
        };
        let level = Self::default_thinking_level_for_entry(&entry);
        let name = self.finalize_model_selection(
            target.provider_id.clone(),
            target.provider_name.clone(),
            &entry,
            level,
        );
        // Cycling walks the list; it must not reorder it.
        self.write_list_setting(RECENT_MODELS_KEY, &recent);
        self.status = format!("model: {name}");
        self.push_log_no_agent(
            LogLevel::Info,
            format!(
                "Switched to recent model: {name} ({})",
                target.provider_name
            ),
        );
        self.needs_redraw = true;
        true
    }
}
