//! The theme editor: edit a theme's colours with a live preview, then save
//! the edit as a theme file or reset a built-in theme to its default.
//!
//! The editor's state is [`ThemeEditorState`](crate::app::ThemeEditorState)
//! on the App; frontends draw it and call these methods. Saving writes the
//! theme back to the file it was loaded from, or, for a built-in theme, to
//! [`user_themes_dir`](crate::theme_tokens::user_themes_dir), where a file of
//! the same name replaces the built-in theme at the next start.
//!
//! Dependencies: [`crate::theme_tokens`] (theme files and the catalog).

use std::sync::Arc;

use crate::app::{App, ThemeEditorState};
use crate::theme_tokens::{Rgb, ThemeTokens, save_theme, theme_file_name, user_themes_dir};

impl App {
    /// Opens the theme editor on the catalog theme called `name`.
    ///
    /// Does nothing for `system` or a name the catalog does not have.
    pub fn open_theme_editor(&mut self, name: &str) {
        let Some(theme) = self.themes.get(name) else {
            return;
        };
        self.theme_editor = Some(ThemeEditorState {
            name: theme.name.clone(),
            draft: (*theme).clone(),
            warnings: theme.contrast_failures(),
            builtin: self.themes.is_builtin(&theme.name),
            original: theme,
        });
    }

    /// Sets the colour at `key` (see [`ThemeTokens::colours`]) in the edited
    /// theme and recomputes its contrast warnings. An unknown key changes
    /// nothing.
    pub fn set_theme_colour(&mut self, key: &str, value: Rgb) {
        let Some(editor) = self.theme_editor.as_mut() else {
            return;
        };
        if editor.draft.set_colour(key, value) {
            editor.warnings = editor.draft.contrast_failures();
        }
    }

    /// Closes the theme editor, dropping the edit.
    pub fn cancel_theme_edit(&mut self) {
        self.theme_editor = None;
    }

    /// Saves the edited theme and closes the editor.
    ///
    /// The theme goes back to the file it was loaded from; a built-in theme
    /// no file replaces goes to the user themes directory. On failure the
    /// editor stays open with the edit, and the status line says why.
    pub fn save_theme_edit(&mut self) {
        let Some(editor) = self.theme_editor.as_ref() else {
            return;
        };
        let name = editor.name.clone();
        let target = self
            .themes
            .source(&name)
            .map(std::path::Path::to_path_buf)
            .or_else(|| user_themes_dir().map(|dir| dir.join(theme_file_name(&name))));
        let Some(target) = target else {
            self.status = "theme not saved: no config directory".to_string();
            return;
        };
        let draft = editor.draft.clone();
        match save_theme(&draft, &target) {
            Ok(()) => {
                self.themes.insert_from(draft, target);
                self.theme_editor = None;
                self.status = format!("theme saved: {name}");
            }
            Err(err) => {
                tracing::warn!(theme = %name, error = %err, "failed to save the theme");
                self.status = format!("theme not saved: {err}");
            }
        }
    }

    /// Resets the edited built-in theme to its default: deletes the file that
    /// replaces it, reloads the catalog and closes the editor.
    ///
    /// Does nothing for a theme that is not built in. A file that cannot be
    /// deleted leaves the editor open, and the status line says why.
    pub fn reset_theme_to_default(&mut self) {
        let Some(editor) = self.theme_editor.as_ref() else {
            return;
        };
        if !editor.builtin {
            return;
        }
        let name = editor.name.clone();
        if let Some(path) = self.themes.source(&name)
            && let Err(err) = std::fs::remove_file(path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(theme = %name, error = %err, "failed to reset the theme");
            self.status = format!("theme not reset: {err}");
            return;
        }
        let (themes, _, _) = Self::load_themes(&self.cwd_path);
        self.themes = themes;
        self.theme_editor = None;
        self.status = format!("theme reset: {name}");
    }

    /// The theme the window draws with: the theme being edited while the
    /// editor is open, otherwise the selected theme.
    ///
    /// `system_dark` is whether the operating system asks for a dark theme.
    #[must_use]
    pub fn active_theme_tokens(&self, system_dark: bool) -> Arc<ThemeTokens> {
        match self.theme_editor.as_ref() {
            Some(editor) => Arc::new(editor.draft.clone()),
            None => self.themes.resolve(&self.theme, system_dark),
        }
    }
}
