//! Routing of team lifecycle events to every session view that tracks the
//! team (team-leader-agent spec, sections 4 and 6).
//!
//! A team's manager publishes its events (`TeammateSpawned`, `TeammateIdle`,
//! `TeamCleanedUp`, ...) with the session id of the team's lead, which is a
//! team-leader child session, never the primary that launched it. A view
//! *tracks* a team event when the event's session is the view's own session,
//! or when it is the leader of a team that view launched (see
//! [`App::view_tracks_team_event`]).
//!
//! [`App::handle_event`] runs the full handler (log lines, watchdog
//! progress, Teams pane) once, in one tracking view, and applies the
//! per-view state change ([`SessionView::apply_team_event`]) to every other
//! tracking view, so the primary's mirror follows its leader's team while
//! the user is looking at the leader or a teammate.
//!
//! Depends on `codemason_agent::event::Event`, the processor's team managers
//! and its session manager (for a leader's parent session).

use codemason_agent::event::Event;
use codemason_agent::team::{MemberStatus, TeamMember};

use super::session_view::SessionView;
use super::state::App;

/// Where [`App::handle_event`] sends one team event: the view that runs the
/// full handler and the other views that only mirror its state change.
/// `None` is the focused view, `Some(index)` a background view.
pub struct TeamEventRoute {
    /// The view that runs the full handler.
    pub owner: Option<usize>,
    /// The other views that track the team.
    pub mirrors: Vec<Option<usize>>,
}

impl App {
    /// The lead session id and team name of a team lifecycle event, or
    /// `None` for any other event.
    pub(crate) fn team_event_ids(event: &Event) -> Option<(&str, &str)> {
        match event {
            Event::TeammateSpawned {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateMessage {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateP2PMessage {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateIdle {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateFailed {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateStopped {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateSuspended {
                session_id,
                team_name,
                ..
            }
            | Event::TeammateResumed {
                session_id,
                team_name,
                ..
            }
            | Event::TeamTaskClaimed {
                session_id,
                team_name,
                ..
            }
            | Event::TeamTaskCompleted {
                session_id,
                team_name,
                ..
            }
            | Event::TeamTaskCompletionRejected {
                session_id,
                team_name,
                ..
            }
            | Event::TeamCleanedUp {
                session_id,
                team_name,
            } => Some((session_id.as_str(), team_name.as_str())),
            // Every other event is not a team lifecycle event.
            _ => None,
        }
    }

    /// Whether the focused view tracks a team event published for
    /// `session_id` about `team_name`: the focused session itself, or the
    /// leader of a team it launched.
    #[must_use]
    pub fn tracks_team_event(&self, session_id: &str, team_name: &str) -> bool {
        self.view_tracks_team_event(&self.session, session_id, team_name)
    }

    /// Whether `view` tracks a team event published for `session_id` about
    /// `team_name` (see [`App::tracks_team_event`]).
    pub(crate) fn view_tracks_team_event(
        &self,
        view: &SessionView,
        session_id: &str,
        team_name: &str,
    ) -> bool {
        view.session_id.as_deref() == Some(session_id)
            || self.view_leads_team(view, session_id, team_name)
    }

    /// Whether `session_id` is the team-leader session of `team_name`, a
    /// team `view` tracks: the leader recorded as its active team's lead, or
    /// a child session of `view`'s session that leads its swarm team or the
    /// team's bound manager.
    pub(crate) fn view_leads_team(
        &self,
        view: &SessionView,
        session_id: &str,
        team_name: &str,
    ) -> bool {
        if view
            .active_team
            .as_ref()
            .is_some_and(|t| t.name == team_name && t.lead_session_id == session_id)
        {
            return true;
        }
        let tracked = view
            .swarm_state
            .as_ref()
            .is_some_and(|s| s.team_name == team_name)
            || self
                .session_processor
                .team_managers
                .get(team_name)
                .is_some_and(|m| m.lead_session_id == session_id);
        let Some(current) = view.session_id.as_deref() else {
            return false;
        };
        tracked
            && matches!(
                self.session_processor.session_manager.get_session(session_id),
                Ok(Some(child)) if child.parent_id.as_deref() == Some(current)
            )
    }

    /// Chooses the views a team event reaches. The routed `owner` keeps the
    /// event when it tracks the team; otherwise the first tracking view
    /// (focused first) runs the full handler. Every other tracking view
    /// mirrors the state change.
    pub(crate) fn team_event_route(
        &self,
        owner: Option<usize>,
        session_id: &str,
        team_name: &str,
    ) -> TeamEventRoute {
        let focused = self
            .view_tracks_team_event(&self.session, session_id, team_name)
            .then_some(None);
        let tracking: Vec<Option<usize>> = focused
            .into_iter()
            .chain(
                self.background
                    .iter()
                    .enumerate()
                    .filter(|(_, view)| self.view_tracks_team_event(view, session_id, team_name))
                    .map(|(index, _)| Some(index)),
            )
            .collect();
        let owner = if tracking.contains(&owner) {
            owner
        } else {
            tracking.first().copied().unwrap_or(owner)
        };
        let mirrors = tracking.into_iter().filter(|t| *t != owner).collect();
        TeamEventRoute { owner, mirrors }
    }

    /// Applies a team event's per-view state change to each mirror view.
    pub(crate) fn mirror_team_event(&mut self, mirrors: &[Option<usize>], event: &Event) {
        for target in mirrors {
            match target {
                None => self.session.apply_team_event(event),
                Some(index) => {
                    if let Some(view) = self.background.view_mut(*index) {
                        view.apply_team_event(event);
                    }
                }
            }
        }
    }
}

impl SessionView {
    /// Whether `team_name` is the team this view's member list mirrors, or
    /// no team is active yet.
    fn mirrors_team(&self, team_name: &str) -> bool {
        self.active_team
            .as_ref()
            .is_none_or(|t| t.name == team_name)
    }

    /// Sets the status (and, with `f`, other fields) of the mirrored member
    /// `agent_id` of `team_name`.
    fn update_member(&mut self, team_name: &str, agent_id: &str, f: impl FnOnce(&mut TeamMember)) {
        if !self.mirrors_team(team_name) {
            return;
        }
        if let Some(member) = self
            .team_members
            .iter_mut()
            .find(|m| m.agent_id == agent_id)
        {
            f(member);
        }
    }

    /// Counts one message for its sender and recipient (FR-052); `lead` is
    /// never counted.
    fn count_message(&mut self, team_name: &str, from: &str, to: &str) {
        if from != "lead" {
            let counts = self
                .team_message_counts
                .entry((team_name.to_string(), from.to_string()))
                .or_insert((0, 0));
            counts.0 = counts.0.saturating_add(1);
        }
        if to != "lead" {
            let counts = self
                .team_message_counts
                .entry((team_name.to_string(), to.to_string()))
                .or_insert((0, 0));
            counts.1 = counts.1.saturating_add(1);
        }
    }

    /// Applies the per-view state change of a team lifecycle event: member
    /// status and task, message counters, a spawned member, and the end of
    /// a cleaned-up team and its swarm. Log lines, watchdog progress and the
    /// Teams pane are the full handler's job, not this one's.
    pub(crate) fn apply_team_event(&mut self, event: &Event) {
        match event {
            Event::TeammateSpawned {
                team_name,
                teammate_name,
                agent_id,
                ..
            } => {
                if self.mirrors_team(team_name)
                    && !self.team_members.iter().any(|m| m.agent_id == *agent_id)
                {
                    self.team_members
                        .push(TeamMember::new(teammate_name, agent_id, "teammate"));
                    self.team_message_counts
                        .entry((team_name.clone(), agent_id.clone()))
                        .or_insert((0, 0));
                }
            }
            Event::TeammateMessage {
                team_name,
                from,
                to,
                ..
            }
            | Event::TeammateP2PMessage {
                team_name,
                from,
                to,
                ..
            } => self.count_message(team_name, from, to),
            Event::TeammateIdle {
                team_name,
                agent_id,
                ..
            }
            | Event::TeammateResumed {
                team_name,
                agent_id,
                ..
            } => self.update_member(team_name, agent_id, |m| m.status = MemberStatus::Idle),
            Event::TeammateFailed {
                team_name,
                agent_id,
                error,
                ..
            } => self.update_member(team_name, agent_id, |m| {
                m.status = MemberStatus::Failed;
                m.last_spawn_error = Some(error.clone());
            }),
            Event::TeammateStopped {
                team_name,
                agent_id,
                ..
            } => self.update_member(team_name, agent_id, |m| {
                m.status = MemberStatus::Stopped;
                m.current_task_id = None;
            }),
            Event::TeammateSuspended {
                team_name,
                agent_id,
                ..
            } => self.update_member(team_name, agent_id, |m| {
                m.status = MemberStatus::Suspended;
            }),
            Event::TeamTaskClaimed {
                team_name,
                agent_id,
                task_id,
                ..
            } => self.update_member(team_name, agent_id, |m| {
                m.status = MemberStatus::Working;
                m.current_task_id = Some(task_id.clone());
            }),
            Event::TeamTaskCompleted {
                team_name,
                agent_id,
                ..
            } => self.update_member(team_name, agent_id, |m| m.current_task_id = None),
            Event::TeamCleanedUp { team_name, .. } => {
                if self.mirrors_team(team_name) {
                    self.active_team = None;
                    self.team_members.clear();
                    self.team_message_counts
                        .retain(|(team, _), _| team != team_name);
                }
                if self
                    .focused_teammate
                    .as_ref()
                    .is_some_and(|(team, _)| team == team_name)
                {
                    self.focused_teammate = None;
                }
                if self
                    .swarm_state
                    .as_ref()
                    .is_some_and(|s| s.team_name == *team_name)
                {
                    self.swarm_state = None;
                }
            }
            // A rejected completion changes no view state; every other
            // event is not a team lifecycle event.
            _ => {}
        }
    }
}
