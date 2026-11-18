//! `/undo` and `/redo`: remove or restore the last conversation turn.
//!
//! An undo deletes the last user message and everything after it from the
//! session, in memory and in storage, so the model no longer sees the turn.
//! When the turn has a pre-turn workspace snapshot
//! ([`codemason_agent::session::turn_snapshot`]), the files are restored to it.
//! The removed messages and the replaced file state are kept on the session's
//! undo stack so `/redo` can put both back. The stack lives in memory and is
//! dropped when the conversation moves on.

use codemason_agent::session::turn_snapshot::SnapshotRepo;
use codemason_types::message::Message;

use crate::app::App;
use crate::app::LogLevel;

/// One undone turn, kept for `/redo`.
#[derive(Debug, Clone)]
pub struct UndoneTurn {
    /// The removed messages, oldest first; the first is the user message.
    pub messages: Vec<Message>,
    /// The workspace tree the undo replaced, when files were restored.
    pub redo_tree: Option<String>,
    /// The id of the last message left after the undo (`None` when the
    /// conversation became empty). A redo is valid only while it still is.
    pub anchor: Option<String>,
}

impl App {
    /// Remove the last turn: its messages and, when a snapshot exists, its
    /// file changes.
    pub(crate) async fn handle_undo(&mut self) {
        let Some(session_id) = self.session.session_id.clone() else {
            self.status = "[warn] No active session to undo".to_string();
            return;
        };
        if self.session.is_processing {
            self.status = "[warn] Cannot undo while the agent is running".to_string();
            return;
        }
        if self.session.messages.is_empty() {
            self.status = "[warn] No messages to undo".to_string();
            return;
        }
        let Some(user_idx) = self
            .session
            .messages
            .iter()
            .rposition(|msg| msg.role == codemason_types::Role::User)
        else {
            self.status = "[warn] No user message found to undo".to_string();
            self.push_log_no_agent(
                LogLevel::Warn,
                "Undo: no user message found in conversation".to_string(),
            );
            return;
        };

        let ids: Vec<String> = self.session.messages[user_idx..]
            .iter()
            .map(|msg| msg.id.clone())
            .collect();
        if let Err(e) = self.storage.delete_messages_by_id(&session_id, &ids) {
            self.status = format!("[warn] Undo failed: {e}");
            return;
        }
        let removed = self.session.messages.split_off(user_idx);
        let removed_count = removed.len();
        self.session.scroll_offset = 0;

        let (redo_tree, files) = self.restore_turn_files(&session_id, &removed[0].id).await;
        self.session.undo_stack.push(UndoneTurn {
            messages: removed,
            redo_tree,
            anchor: self.session.messages.last().map(|msg| msg.id.clone()),
        });

        self.status = format!(
            "Undid last turn (removed {removed_count} message(s){}); /redo restores it",
            files_note(&files)
        );
        self.push_log_no_agent(
            LogLevel::Info,
            format!(
                "Undo: removed {removed_count} message(s) from end of conversation{}",
                files_log(&files)
            ),
        );
    }

    /// Put back the turn the last `/undo` removed.
    pub(crate) async fn handle_redo(&mut self) {
        let Some(session_id) = self.session.session_id.clone() else {
            self.status = "[warn] No active session to redo".to_string();
            return;
        };
        if self.session.is_processing {
            self.status = "[warn] Cannot redo while the agent is running".to_string();
            return;
        }
        let anchor = self.session.messages.last().map(|msg| msg.id.clone());
        if self
            .session
            .undo_stack
            .last()
            .is_some_and(|turn| turn.anchor != anchor)
        {
            // The conversation moved on since the undo; the stack is stale.
            self.session.undo_stack.clear();
        }
        let Some(turn) = self.session.undo_stack.pop() else {
            self.status = "[warn] Nothing to redo".to_string();
            return;
        };

        for message in &turn.messages {
            if let Err(e) = self.storage.create_message(message) {
                self.status = format!("[warn] Redo failed: {e}");
                self.session.undo_stack.push(turn);
                return;
            }
        }
        let restored_count = turn.messages.len();
        let user_message_id = turn.messages[0].id.clone();
        self.session.messages.extend(turn.messages);
        self.session.scroll_offset = 0;

        let mut files: Result<Vec<String>, String> = Ok(Vec::new());
        if let Some(tree) = turn.redo_tree
            && let Some(repo) = SnapshotRepo::for_working_dir(&self.cwd_path)
        {
            files = match repo.restore(&tree).await {
                Ok((pre_turn, changed)) => {
                    // The turn's snapshot is its pre-state again.
                    if let Err(e) = repo
                        .set_turn_tree(&session_id, &user_message_id, &pre_turn)
                        .await
                    {
                        tracing::warn!(error = %e, "could not re-record the turn snapshot");
                    }
                    Ok(changed)
                }
                Err(e) => Err(e.to_string()),
            };
        }

        self.status = format!(
            "Redid last turn (restored {restored_count} message(s){})",
            files_note(&files)
        );
        self.push_log_no_agent(
            LogLevel::Info,
            format!(
                "Redo: restored {restored_count} message(s){}",
                files_log(&files)
            ),
        );
    }

    /// Restore the workspace to the snapshot of the turn opened by
    /// `user_message_id`.
    ///
    /// Returns the tree that was replaced (for `/redo`) and the changed paths,
    /// or the reason the files were left alone. A turn with no snapshot (it
    /// made no tool call, or snapshots are off) changes nothing.
    async fn restore_turn_files(
        &self,
        session_id: &str,
        user_message_id: &str,
    ) -> (Option<String>, Result<Vec<String>, String>) {
        let Some(repo) = SnapshotRepo::for_working_dir(&self.cwd_path) else {
            return (None, Ok(Vec::new()));
        };
        let Some(tree) = repo.turn_tree(session_id, user_message_id).await else {
            return (None, Ok(Vec::new()));
        };
        match repo.restore(&tree).await {
            Ok((replaced, changed)) => (Some(replaced), Ok(changed)),
            Err(e) => (None, Err(e.to_string())),
        }
    }
}

/// The status-bar clause describing the file outcome.
fn files_note(files: &Result<Vec<String>, String>) -> String {
    match files {
        Ok(changed) if changed.is_empty() => String::new(),
        Ok(changed) => format!(", {} file(s) changed", changed.len()),
        Err(_) => ", files NOT restored - see log".to_string(),
    }
}

/// The log clause listing the changed files or the failure.
fn files_log(files: &Result<Vec<String>, String>) -> String {
    match files {
        Ok(changed) if changed.is_empty() => String::new(),
        Ok(changed) => format!("; files: {}", changed.join(", ")),
        Err(reason) => format!("; file restore failed: {reason}"),
    }
}
