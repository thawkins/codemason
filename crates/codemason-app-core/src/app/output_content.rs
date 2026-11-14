//! What the tool-output overlay (`App::output_view`) shows, for a frontend
//! that draws it from the message list rather than from the terminal's
//! wrapped-line cache.
//!
//! The terminal frontend builds the same title, target session and message
//! source inline in its renderer; [`App::output_view_content`] is that
//! decision as a read-only query, so the GUI shows the same transcript for
//! the same target.
//!
//! Depends on `codemason_agent`'s `Message` and the App's storage.

use std::borrow::Cow;
use std::hash::{Hash, Hasher};

use codemason_agent::message::Message;

use super::state::{App, OutputViewTarget};

/// The transcript the output overlay shows now.
#[derive(Debug, Clone)]
pub struct OutputViewContent<'a> {
    /// The overlay's title, e.g. `Output: build` or `Output: alice [tm-001]`.
    pub title: String,
    /// The session whose messages are shown, when the target has one.
    pub session_id: Option<String>,
    /// Whether that session is the focused one, whose messages are edited
    /// in place as they stream (`edit_seq` advances); other sessions are
    /// read back from storage, which does not keep `edit_seq`.
    pub focused: bool,
    /// The messages to show, oldest first; empty when the target has no
    /// output yet.
    pub messages: Cow<'a, [Message]>,
    /// A key that changes whenever the shown output changes: the target,
    /// the message count and last `edit_seq`, and the target session's step
    /// and tool-call counters (which advance while a stored session runs).
    pub generation: u64,
}

impl App {
    /// The transcript the open output overlay shows, if it is open.
    ///
    /// A team member's view whose session id is not known yet resolves it
    /// from the team member list, as the terminal renderer does. The focused
    /// session's messages are borrowed; another session's are loaded from
    /// storage (a load error is shown as one assistant message, not hidden).
    ///
    /// # Returns
    ///
    /// The overlay's title, target session, messages and change key, or
    /// `None` when the overlay is closed.
    #[must_use]
    pub fn output_view_content(&self) -> Option<OutputViewContent<'_>> {
        let view = self.output_view.as_ref()?;
        let (title, session_id) = match &view.target {
            OutputViewTarget::Session { session_id, label } => {
                (format!("Output: {label}"), Some(session_id.clone()))
            }
            OutputViewTarget::TeamMember {
                teammate_name,
                agent_id,
                session_id,
                ..
            } => {
                let session_id = session_id.clone().or_else(|| {
                    self.session
                        .team_members
                        .iter()
                        .find(|m| m.agent_id == *agent_id)
                        .and_then(|m| m.session_id.clone())
                });
                (format!("Output: {teammate_name} [{agent_id}]"), session_id)
            }
        };
        let focused = session_id.is_some() && session_id == self.session.session_id;
        let messages: Cow<'_, [Message]> = match session_id.as_deref() {
            None => Cow::Owned(Vec::new()),
            Some(_) if focused => Cow::Borrowed(self.session.messages.as_slice()),
            Some(sid) => match self.storage.get_messages(sid) {
                Ok(messages) => Cow::Owned(messages),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        session_id = %sid,
                        "Failed to load session messages for output view"
                    );
                    Cow::Owned(vec![Message::assistant_text(
                        sid.to_string(),
                        format!("[warn] Failed to load output: {e}"),
                    )])
                }
            },
        };
        let generation = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            session_id.hash(&mut hasher);
            messages.len().hash(&mut hasher);
            messages.last().map(|m| m.edit_seq).hash(&mut hasher);
            if let Some(sid) = session_id.as_deref() {
                self.event_bus.current_step(sid).hash(&mut hasher);
                self.event_bus.current_tool_calls(sid).hash(&mut hasher);
            }
            hasher.finish()
        };
        Some(OutputViewContent {
            title,
            session_id,
            focused,
            messages,
            generation,
        })
    }
}
