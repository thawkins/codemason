//! Per-team registry of live [`TeamManager`] runtimes.
//!
//! One session processor can drive several teams at once (several
//! team-leader sub-agents, `/swarm`, `serve` mode). Each team gets its own
//! [`TeamManager`], keyed by team name, so binding one team never evicts or
//! reads another team's runtime.
//!
//! Depends on [`super::manager::TeamManager`] and `parking_lot`.

use std::collections::HashMap;
use std::sync::Arc;

use super::manager::TeamManager;

/// Registry of the team runtimes a session processor drives, keyed by team
/// name.
#[derive(Default)]
pub struct TeamManagers(parking_lot::RwLock<HashMap<String, Arc<TeamManager>>>);

impl std::fmt::Debug for TeamManagers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<String> = self.0.read().keys().cloned().collect();
        names.sort();
        f.debug_tuple("TeamManagers").field(&names).finish()
    }
}

impl TeamManagers {
    /// The manager bound for `team_name`, if any.
    #[must_use]
    pub fn get(&self, team_name: &str) -> Option<Arc<TeamManager>> {
        self.0.read().get(team_name).cloned()
    }

    /// Bind `manager` under its team name.
    ///
    /// A stale manager (its team is closed, deleted or not `Active`) for the
    /// same team is replaced.
    ///
    /// # Errors
    /// Returns `manager` back when a *live* manager for the same team is
    /// already bound.
    pub fn bind(&self, manager: Arc<TeamManager>) -> Result<(), Arc<TeamManager>> {
        let mut map = self.0.write();
        if let Some(existing) = map.get(&manager.team_name)
            && existing.team_is_live()
        {
            return Err(manager);
        }
        map.insert(manager.team_name.clone(), manager);
        Ok(())
    }

    /// Unbind `expected` only if it is still the manager bound for its team,
    /// so a manager another task bound in between is not dropped.
    ///
    /// # Returns
    /// `true` when `expected` was bound and has been unbound.
    pub fn remove_if(&self, expected: &Arc<TeamManager>) -> bool {
        let mut map = self.0.write();
        let is_bound = map
            .get(&expected.team_name)
            .is_some_and(|bound| Arc::ptr_eq(bound, expected));
        if is_bound {
            map.remove(&expected.team_name);
        }
        is_bound
    }

    /// Remove and return every manager whose team is no longer live. Live
    /// teams are left untouched.
    pub fn prune_stale(&self) -> Vec<Arc<TeamManager>> {
        let mut map = self.0.write();
        let stale: Vec<String> = map
            .iter()
            .filter(|(_, manager)| !manager.team_is_live())
            .map(|(name, _)| name.clone())
            .collect();
        let mut removed: Vec<Arc<TeamManager>> =
            stale.iter().filter_map(|name| map.remove(name)).collect();
        removed.sort_by(|a, b| a.team_name.cmp(&b.team_name));
        removed
    }

    /// Every bound manager, sorted by team name.
    #[must_use]
    pub fn all(&self) -> Vec<Arc<TeamManager>> {
        let mut managers: Vec<Arc<TeamManager>> = self.0.read().values().cloned().collect();
        managers.sort_by(|a, b| a.team_name.cmp(&b.team_name));
        managers
    }
}
