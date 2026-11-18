//! `/undo` and `/redo`: remove or restore the last conversation turn.
//!
//! An undo deletes the last user message and everything after it from the
//! session, in memory and in storage, so the model no longer sees the turn.
//! When the turn has a pre-turn workspace snapshot
//! ([`codemason_agent::session::turn_snapshot`]), the files are restored to it.
//! The removed messages (the transcript's and the stored copies) and the
//! replaced file state are kept on the session's undo stack so `/redo` can put both back. The stack lives in memory and is
//! dropped when the conversation moves on.

use codemason_agent::session::turn_snapshot::SnapshotRepo;
use codemason_types::message::Message;

use crate::app::App;
use crate::app::LogLevel;

/// How far before the transcript's user message a stored user message may be
/// dated and still be the same turn. The transcript message is created first
/// and the processor stores its own copy moments later, so the stored one is
/// normally the later of the two.
const SAME_TURN_SLACK: chrono::Duration = chrono::Duration::seconds(2);

/// One undone turn, kept for `/redo`.
#[derive(Debug, Clone)]
pub struct UndoneTurn {
    /// The messages removed from the transcript, oldest first.
    pub view: Vec<Message>,
    /// The messages deleted from storage, oldest first; the first is the
    /// stored user message. Empty when the turn was never stored.
    pub stored: Vec<Message>,
    /// The workspace tree the undo replaced, when files were restored.
    pub redo_tree: Option<String>,
    /// The id of the last stored message left after the undo (`None` when the
    /// stored history became empty). A redo is valid only while it still is.
    pub anchor: Option<String>,
}

impl App {
    /// Remove the last turn: its messages and, when a snapshot exists, its
    /// file changes.
    ///
    /// The transcript and the session database hold separate copies of a
    /// turn, under different message ids (the transcript's is created when the
    /// prompt is sent, the stored one by the processor). The turn is therefore
    /// resolved on both sides: the transcript loses its last user message and
    /// everything after it, and storage loses its last user message and
    /// everything after it, provided that stored turn is the same turn (same
    /// id, or not older than the transcript's).
    pub(crate) async fn handle_undo(&mut self) {
        let Some(session_id) = self.session.session_id.clone() else {
            self.status = "[warn] No active session to undo".to_string();
            return;
        };
        if self.session.is_processing {
            self.status = "[warn] Cannot undo while the agent is running".to_string();
            return;
        }
        let stored = match self.storage.get_messages(&session_id) {
            Ok(stored) => stored,
            Err(e) => {
                self.status = format!("[warn] Undo failed: {e}");
                return;
            }
        };
        if self.session.messages.is_empty() && stored.is_empty() {
            self.status = "[warn] No messages to undo".to_string();
            return;
        }
        let is_user = |msg: &Message| msg.role == codemason_types::Role::User;
        let view_idx = self.session.messages.iter().rposition(is_user);
        let stored_idx = stored.iter().rposition(is_user).filter(|&idx| {
            view_idx.is_none_or(|view_idx| {
                let view = &self.session.messages[view_idx];
                stored[idx].id == view.id
                    || stored[idx].created_at + SAME_TURN_SLACK >= view.created_at
            })
        });
        if view_idx.is_none() && stored_idx.is_none() {
            self.status = "[warn] No user message found to undo".to_string();
            self.push_log_no_agent(
                LogLevel::Warn,
                "Undo: no user message found in conversation".to_string(),
            );
            return;
        }
        if stored_idx.is_some() && self.other_session_running_in_work_tree() {
            self.status =
                "[warn] Cannot undo while another session is running in this work tree".to_string();
            return;
        }

        let stored_removed: Vec<Message> =
            stored_idx.map_or_else(Vec::new, |idx| stored[idx..].to_vec());
        if !stored_removed.is_empty() {
            let ids: Vec<String> = stored_removed.iter().map(|msg| msg.id.clone()).collect();
            if let Err(e) = self.storage.delete_messages_by_id(&session_id, &ids) {
                self.status = format!("[warn] Undo failed: {e}");
                return;
            }
        }
        let view_removed =
            view_idx.map_or_else(Vec::new, |idx| self.session.messages.split_off(idx));
        let removed_count = if view_removed.is_empty() {
            stored_removed.len()
        } else {
            view_removed.len()
        };
        self.session.scroll_offset = 0;

        let (redo_tree, files) = match stored_removed.first() {
            Some(user) => self.restore_turn_files(&session_id, &user.id).await,
            None => (None, Ok(Vec::new())),
        };
        self.session.undo_stack.push(UndoneTurn {
            view: view_removed,
            anchor: stored[..stored_idx.unwrap_or(stored.len())]
                .last()
                .map(|msg| msg.id.clone()),
            stored: stored_removed,
            redo_tree,
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
        let anchor = match self.storage.get_messages(&session_id) {
            Ok(stored) => stored.last().map(|msg| msg.id.clone()),
            Err(e) => {
                self.status = format!("[warn] Redo failed: {e}");
                return;
            }
        };
        if self
            .session
            .undo_stack
            .last()
            .is_some_and(|turn| turn.anchor != anchor)
        {
            // The stored conversation moved on since the undo; the stack is stale.
            self.session.undo_stack.clear();
        }
        let Some(turn) = self.session.undo_stack.pop() else {
            self.status = "[warn] Nothing to redo".to_string();
            return;
        };
        if turn.redo_tree.is_some() && self.other_session_running_in_work_tree() {
            self.status =
                "[warn] Cannot redo while another session is running in this work tree".to_string();
            self.session.undo_stack.push(turn);
            return;
        }

        // One transaction: the whole turn returns to storage, or none of it.
        if let Err(e) = self.storage.create_messages(&turn.stored) {
            self.status = format!("[warn] Redo failed: {e}");
            self.session.undo_stack.push(turn);
            return;
        }
        let restored_count = if turn.view.is_empty() {
            turn.stored.len()
        } else {
            turn.view.len()
        };
        self.session.messages.extend(turn.view);
        self.session.scroll_offset = 0;

        let mut files: Result<Vec<String>, String> = Ok(Vec::new());
        if let Some(tree) = turn.redo_tree
            && let Some(user) = turn.stored.first()
            && let Some(repo) = SnapshotRepo::for_working_dir(&self.cwd_path)
        {
            files = match repo.restore(&tree).await {
                Ok((pre_turn, changed)) => {
                    // The turn's snapshot is its pre-state again.
                    if let Err(e) = repo.set_turn_tree(&session_id, &user.id, &pre_turn).await {
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

    /// Whether another open session is running a turn in the git work tree a
    /// restore here would rewrite.
    ///
    /// A restore replaces the whole work tree, so it must not run under an
    /// agent that is editing the same tree from another session. The check and
    /// the restore both happen inside one slash-command dispatch, which holds
    /// the application state exclusively, so no session can start a turn from
    /// this application between them.
    fn other_session_running_in_work_tree(&self) -> bool {
        let Some(root) = codemason_agent::session::turn_snapshot::work_tree_root(&self.cwd_path)
        else {
            return false;
        };
        self.background.iter().any(|view| {
            view.is_processing
                && codemason_agent::session::turn_snapshot::work_tree_root(&view.directory)
                    .is_some_and(|other| other == root)
        })
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
