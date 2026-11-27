//! `team_create` - Create a new named team and write its config to disk.

use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::TaskStore;
use crate::team::TeamStore;
use crate::team::manager::HookOutcome;
use crate::tool::metadata::MetadataBuilder;

/// Dispatch a tool invoked from a blueprint seed file through the session's
/// permission gate.
///
/// A blueprint directory is untrusted project content (it is contributed by a
/// repository, a plugin pack, or a marketplace bundle), so a seed entry naming
/// `Bash`/`Write`/`rm`/`team_spawn` must not execute directly: that bypassed
/// `dispatch_tool_with_permissions` entirely - no permission-category check, no
/// prompt, no repeat guard (SEC-codemason-team-001, SEC-codemason-agent-008).
///
/// The gate applied here is the subset of the agent-loop decision that is
/// meaningful outside the loop:
///
/// - a tool with no permission category (or `none`) is inert and runs;
/// - a tool the session policy explicitly denies never runs;
/// - a tool with an explicit `Allow` rule runs without a prompt - the user
///   already granted that category, exactly as for an ordinary call;
/// - anything else (`Ask`, or no permission checker at all) fails closed with
///   an error naming the tool, so the lead sees why the seed did not run
///   instead of the call silently executing.
///
/// # Errors
///
/// Returns an error when the permission decision is not an explicit allow, or
/// when the tool itself fails.
async fn dispatch_seed_tool(
    tool: &std::sync::Arc<dyn Tool>,
    input: Value,
    ctx: &ToolContext,
    tool_name: &str,
) -> Result<ToolOutput> {
    let category = tool.permission_category();
    if category.is_empty() || category == "none" {
        return tool.execute(input, ctx).await;
    }

    let resource = crate::session::permissions::extract_resource_from_input(&input, tool_name);
    let action = match &ctx.permission_checker {
        Some(checker) => checker.read().check(category, &resource),
        // No checker wired in: the call cannot be authorised, so it is refused
        // rather than allowed (fail closed).
        None => {
            return Err(anyhow::anyhow!(
                "blueprint seed refused: '{tool_name}' requires permission '{category}' \
                 but this session has no permission checker"
            ));
        }
    };

    match action {
        crate::permission::PermissionAction::Allow => tool.execute(input, ctx).await,
        crate::permission::PermissionAction::Deny => Err(anyhow::anyhow!(
            "blueprint seed refused: permission '{category}' is denied for '{resource}'"
        )),
        crate::permission::PermissionAction::Ask => Err(anyhow::anyhow!(
            "blueprint seed refused: '{tool_name}' requires permission '{category}' for \
             '{resource}', which is not pre-approved. Blueprint seeds may only invoke \
             tools the session already allows, or team task tools (team_task_create, \
             team_spawn) that carry a team permission category"
        )),
    }
}

/// Stable reason code for a `team_create` call from a non-leader agent.
pub const REASON_NOT_TEAM_LEADER: &str = "not_team_leader";

/// Creates a new team directory and initial config.
pub struct TeamCreateTool;

#[async_trait::async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &'static str {
        "team_create"
    }

    fn description(&self) -> &'static str {
        "Create a new named agent team. Only a team-leader agent may call this \
             (spawn one with new_agent agent=\"team-leader\"); team names are unique. ALWAYS pass 'context' with the user's \
             specific request (e.g. which directory/files to review, what task to \
             perform, where to write output). If a blueprint is provided, all teammates \
             defined in the blueprint's spawn-prompts.json are spawned automatically - \
             do NOT spawn them again. REQUIRED parameters: 'blueprint' (string) and \
             'context' (string). Optional: 'name' (string, lowercase/hyphens OK; auto-generated \
             from blueprint and timestamp if omitted) and 'project_local' (boolean, default true). \
             After team_create, call team_wait to block until all teammates finish their \
             initial work."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "blueprint": {
                    "type": "string",
                    "description": "Blueprint name to seed the team from [PROJECT]/.codemason/blueprints/teams/<name>, ~/.config/codemason/blueprints/teams/<name>, or ~/.codemason/blueprints/teams/<name> - closest wins (required)"
                },
                "context": {
                    "type": "string",
                    "description": "REQUIRED: The specific work context from the user's request - which files/directories to target, what to produce, where to write output. This is prepended to every teammate's spawn prompt so they know exactly what to work on."
                },
                "name": {
                    "type": "string",
                    "description": "Optional team name (lowercase, hyphens OK). If omitted, name will be generated from blueprint and timestamp"
                },
                "project_local": {
                    "type": "boolean",
                    "description": "If true, store team in [PROJECT]/.codemason/teams/; otherwise in ~/.codemason/teams/. Default: true"
                },
                "seed_only": {
                    "type": "boolean",
                    "description": "Apply the blueprint to an existing team instead of creating one. The team must already exist and the calling session must be that team's lead session; the call never re-creates its directory. Used by `/team create`, which has created the team before seeding. Default: false"
                }
            },
            "required": [
                "blueprint",
                "context"
            ],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Creation is reserved for a team-leader agent. `seed_only` skips this
        // check but is instead restricted to the team's own lead session (see
        // `open_team_store`), because the flag is model-visible input.
        let seed_only = input
            .get("seed_only")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !seed_only && ctx.agent_role != crate::agent::AgentRole::TeamLeader {
            return Err(super::param_schema::reason_error(
                REASON_NOT_TEAM_LEADER,
                "team_create may only be called by a team-leader agent. Spawn one with \
                 new_agent agent=\"team-leader\" background=true and pass the blueprint and \
                 context in its task.",
            ));
        }

        // Blueprint is mandatory now. Validate presence.
        let bp = input
            .get("blueprint")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| anyhow::anyhow!("'blueprint' parameter is required"))?;

        // Explicit name (None when generating one).
        let explicit_name = input
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);
        // SEC-codemason-team-002: the blueprint name is path-adjacent too, so
        // the generated base name is slugified rather than interpolated raw.
        let base_name = explicit_name.clone().unwrap_or_else(|| {
            format!(
                "{}-{}",
                crate::team::slugify_team_name(&bp),
                Utc::now().format("%Y%m%d-%H-%M-%S")
            )
        });

        let project_local = input
            .get("project_local")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);

        // Work context from the user's request - prepended to every teammate's
        // spawn prompt so they know which files/directories to target.
        let work_context = input
            .get("context")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);

        // `seed_only` applies a blueprint to a team that already exists: the
        // caller (`/team create`) has already created it, so this call must
        // never re-create the directory. Re-creating it after a `/team clear`
        // or `/team forcecleanup` removed the tree resurrected the directory
        // and its `config.json`/`mailbox`/lock file, which made the removal
        // fail with ENOTEMPTY (CI flake in
        // `test_slash_arms_deep::team_clear_removes_ta` and
        // `test_slash_teams_deep::forcecleanup_confirm_removes_team`).

        let (name, mut store) = open_team_store(
            &base_name,
            explicit_name.is_some(),
            ctx,
            project_local,
            seed_only,
        )?;

        // M8-T2: collect per-prompt blueprint spawn failures so the lead is
        // told which teammates failed to spawn instead of only seeing them
        // silently logged.
        let mut failed_spawns: Vec<serde_json::Value> = Vec::new();

        // Apply the blueprint, when one is found on disk.
        if let Some(bdir) = find_blueprint_dir(&ctx.working_dir, &bp) {
            // If README.md exists in blueprint, copy it to team dir
            let readme = bdir.join("README.md");
            if readme.exists() {
                // INTENTIONAL: best-effort team-directory + readme scaffolding
                let _ = std::fs::create_dir_all(&store.dir);
                let dest = store.dir.join("README.md");
                // INTENTIONAL: best-effort readme copy
                let _ = std::fs::copy(&readme, &dest);
            }

            run_task_seed(&bdir, &store, &name, ctx).await;
            failed_spawns =
                run_spawn_prompts(&bdir, &mut store, &name, ctx, work_context.as_deref()).await;
        }

        // Reload store to get the final member list after blueprint seeding.
        let final_store = TeamStore::load_by_name(&name, &ctx.working_dir).unwrap_or(store);
        let member_summary = member_summary(&final_store);
        let failed_spawn_count = failed_spawns.len();
        let failed_summary = failed_spawn_summary(&failed_spawns);

        Ok(ToolOutput {
            content: format!(
                "Team '{}' created successfully.\nDirectory: {}\nLead session: {}\n\n{}{}\n\n\
                                    Next: call `team_wait` to block until all teammates finish their initial work.",
                name,
                final_store.dir.display(),
                ctx.session_id,
                member_summary,
                failed_summary
            ),
            metadata: MetadataBuilder::new()
                .custom("team_name", &name)
                .custom("team_dir", final_store.dir.to_string_lossy())
                .custom("lead_session_id", &ctx.session_id)
                .custom("project_local", project_local)
                .custom("members_spawned", final_store.config.members.len())
                .custom("auto_named", explicit_name.is_none())
                // M8-T2: include the per-prompt spawn failure list in metadata
                // so programmatic consumers (HTTP API, tests) can detect
                // partial blueprint seeding without parsing the text output.
                .custom("failed_spawn_count", failed_spawn_count)
                .custom("failed_spawns", serde_json::Value::Array(failed_spawns))
                .build(),
        })
    }
}

/// Open the team store `team_create` seeds, returning the final team name.
///
/// `seed_only` loads the existing team and requires the calling session to be
/// that team's lead session. Otherwise the team is created with an atomic
/// create-directory-exclusive (see `TeamStore::create`), so concurrent creates
/// cannot both win. A generated name that collides gets `-2`, `-3`, ... until
/// it is free (bounded); an explicit name whose team directory already exists
/// in any state is refused.
///
/// # Errors
///
/// Returns an error when the team cannot be created or loaded, when an
/// explicit name is already taken, or when `seed_only` is requested by a
/// session other than the team's lead.
fn open_team_store(
    name: &str,
    explicit: bool,
    ctx: &ToolContext,
    project_local: bool,
    seed_only: bool,
) -> Result<(String, TeamStore)> {
    if seed_only {
        let store = TeamStore::load_by_name(name, &ctx.working_dir)?;
        if store.config.lead_session_id != ctx.session_id {
            return Err(super::param_schema::reason_error(
                REASON_NOT_TEAM_LEADER,
                format!("only team '{name}' lead session may seed it with a blueprint."),
            ));
        }
        return Ok((name.to_string(), store));
    }
    /// Upper bound on generated-name suffix attempts.
    const MAX_ATTEMPTS: u32 = 1000;
    let mut candidate = name.to_string();
    for attempt in 2..=MAX_ATTEMPTS + 1 {
        match TeamStore::create(&candidate, &ctx.session_id, &ctx.working_dir, project_local) {
            Ok(store) => return Ok((candidate, store)),
            Err(e) if e.to_string().contains("already exists") => {
                if explicit {
                    return Err(anyhow::anyhow!(
                        "team '{candidate}' already exists; choose a different name or run \
                         `/team cleanup` to remove it"
                    ));
                }
                candidate = format!("{name}-{attempt}");
            }
            Err(e) => return Err(e),
        }
    }
    Err(anyhow::anyhow!(
        "could not find a free team name based on '{name}' after {MAX_ATTEMPTS} attempts"
    ))
}

/// Locate the blueprint directory for `bp`, closest wins:
/// 1. project-local [ancestor]/.codemason/blueprints/teams/<bp>
/// 2. XDG ~/.config/codemason/blueprints/teams/<bp>
/// 3. legacy global ~/.codemason/blueprints/teams/<bp>
fn find_blueprint_dir(working_dir: &std::path::Path, bp: &str) -> Option<std::path::PathBuf> {
    // Walk up to find project .codemason (check the current dir, then parent, etc.)
    let mut cur_opt = Some(working_dir);
    while let Some(cur) = cur_opt {
        let candidate = cur
            .join(".codemason")
            .join("blueprints")
            .join("teams")
            .join(bp);
        if candidate.is_dir() {
            return Some(candidate);
        }
        cur_opt = cur.parent();
    }
    // XDG user-config middle layer.
    if let Some(bp_root) = codemason_config::user_dirs::global_blueprints_dir() {
        let candidate = bp_root.join("teams").join(bp);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    // Fallback to the legacy global ~/.codemason/blueprints/teams/<bp>.
    if let Some(bp_root) =
        codemason_config::user_dirs::legacy_home_dir().map(|h| h.join("blueprints"))
    {
        let candidate = bp_root.join("teams").join(bp);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// Run the blueprint's `task-seed.json`, if present: each entry naming a
/// `tool` is dispatched through the permission gate with `team_name` forced to
/// `name`; an entry with a `title` and no tool becomes a task directly.
/// Failures are logged and never abort team creation.
async fn run_task_seed(bdir: &std::path::Path, store: &TeamStore, name: &str, ctx: &ToolContext) {
    // If task-seed.json exists, parse and execute each seed
    let seed = bdir.join("task-seed.json");
    let Some(items) = read_json_array(&seed) else {
        return;
    };
    let registry = crate::tool::create_default_registry();
    for item in &items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        if let Some(tool_name) = obj.get("tool").and_then(|v| v.as_str()) {
            let args = obj
                .get("args")
                .or_else(|| obj.get("input"))
                .cloned()
                .unwrap_or(serde_json::json!({}));
            // Always override team_name to the actual team name
            // (seed files may contain placeholder/template names)
            let mut args_obj = if args.is_object() {
                args
            } else {
                serde_json::json!({})
            };
            args_obj.as_object_mut().map(|m| {
                m.insert(
                    "team_name".to_string(),
                    serde_json::Value::String(name.to_string()),
                )
            });
            if let Some(tool) = registry.get(tool_name) {
                // SEC: tool args are attacker-supplied and may carry
                // secrets - log the argument keys only, never values.
                let arg_keys = args_obj
                    .as_object()
                    .map(|m| m.keys().cloned().collect::<Vec<_>>().join(","))
                    .unwrap_or_default();
                tracing::info!(tool = %tool_name, team = %name, session = %ctx.session_id, team_manager_present = %ctx.team_manager.is_some(), "Invoking seed tool");
                // SEC-codemason-team-001: a blueprint is untrusted
                // project content, so a seeded tool is dispatched
                // through the session's permission gate rather than
                // executed directly.
                match dispatch_seed_tool(&tool, args_obj, ctx, tool_name).await {
                    Ok(_out) => {
                        tracing::debug!(tool = %tool_name, "Seed tool executed successfully");
                    }
                    Err(e) => {
                        tracing::error!(tool = %tool_name, error = %e, arg_keys = %arg_keys, "Seed tool execution failed");
                    }
                }
            }
        } else if obj.get("title").is_some() {
            // If no tool specified but task fields present, create a task directly
            seed_task(obj, store, name).await;
        }
    }
}

/// Parse `path` as a JSON array, or `None` when it is missing, unreadable or
/// not an array.
fn read_json_array(path: &std::path::Path) -> Option<Vec<serde_json::Value>> {
    if !path.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<serde_json::Value>(&raw).ok()? {
        serde_json::Value::Array(items) => Some(items),
        _ => None,
    }
}

/// Add one task-seed entry (a `title` with an optional `description`) to the
/// team's task list, removing it again when the `TaskCreated` hook rejects it.
async fn seed_task(
    obj: &serde_json::Map<String, serde_json::Value>,
    store: &TeamStore,
    name: &str,
) {
    let title = obj
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("untitled")
        .to_string();
    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    // FR-017: the add fires the `TaskCreated`
    // hook from the mutation, so a seeded task
    // runs the same gate a `team_task_create`
    // call does. A rejection removes the seed.
    // AA-04: the id is allocated under the
    // task-store lock, so a concurrent creator
    // cannot take it first.
    let task_outcome = store
        .add_task_with_next_id(|id| {
            let mut task = crate::team::task::Task::new(&id, &title);
            if let Some(desc) = description {
                task.description = desc;
            }
            task
        })
        .await;
    match task_outcome {
        Ok((task, HookOutcome::Feedback(feedback))) => {
            if let Ok(ts) = TaskStore::open(&store.dir) {
                // INTENTIONAL: best-effort store cleanup
                let _ = ts.remove_task(&task.id);
            }
            // The feedback is the hook's own stdout: redact
            // it before it reaches the log (audit finding).
            tracing::warn!(
                task_id = %task.id,
                feedback = %codemason_types::sanitize::redact_secrets(&feedback),
                "TaskCreated hook rejected seeded task"
            );
        }
        Ok((_, HookOutcome::Allow)) => {}
        Err(e) => {
            tracing::warn!(
                team = %name,
                title = %title,
                error = %e,
                "could not add seeded task"
            );
        }
    }
}

/// Run the blueprint's `spawn-prompts.json`, if present: each entry is turned
/// into spawn-tool arguments and dispatched through the permission gate.
/// `store` is refreshed after each spawn so `next_agent_id()` stays accurate.
///
/// Returns one record per spawn that failed (M8-T2), so the lead is told which
/// teammates are missing.
async fn run_spawn_prompts(
    bdir: &std::path::Path,
    store: &mut TeamStore,
    name: &str,
    ctx: &ToolContext,
    work_context: Option<&str>,
) -> Vec<serde_json::Value> {
    let mut failed_spawns: Vec<serde_json::Value> = Vec::new();
    // If spawn-prompts.json exists, parse entries and execute listed tools with args
    let prompts_json = bdir.join("spawn-prompts.json");
    let Some(items) = read_json_array(&prompts_json) else {
        return failed_spawns;
    };
    let registry = crate::tool::create_default_registry();
    for (i, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        // Support either {"tool": "name", "args": {...}} or
        // flattened formats using keys like tool_name/team_name/teammate_name
        let Some(tool_name) = obj
            .get("tool")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("tool_name").and_then(|v| v.as_str()))
        else {
            continue;
        };
        let args = spawn_prompt_args(obj, i, name, work_context);
        let Some(tool) = registry.get(tool_name) else {
            continue;
        };
        tracing::info!(tool = %tool_name, team = %name, session = %ctx.session_id, team_manager_present = %ctx.team_manager.is_some(), "Invoking spawn tool from blueprint (spawn-prompts.json)");
        // SEC-codemason-team-001: a seeded spawn is
        // dispatched through the session's permission
        // gate, like the task-seed path above.
        match dispatch_seed_tool(&tool, args.clone(), ctx, tool_name).await {
            Ok(out) => {
                tracing::debug!(tool = %tool_name, "Spawn tool executed");
                if let Some(meta) = out.metadata
                    && let Some(status) = meta.get("status").and_then(|v| v.as_str())
                {
                    if status == "pending_manager" {
                        tracing::warn!(tool = %tool_name, team = %name, "Spawn returned pending_manager; recording spawning member in team config");
                        record_spawning_member(&args, name, ctx);
                        // Keep local store in sync for next_agent_id() calculations.
                        if let Ok(fresh) = TeamStore::load_by_name(name, &ctx.working_dir) {
                            *store = fresh;
                        }
                    } else if status == "spawned" {
                        // spawn_teammate_internal already persisted the member with the
                        // correct session_id and Working status.  Adding a new TeamMember
                        // record here (with default Spawning status and no session_id)
                        // would overwrite that correct state and leave the member stuck in
                        // "spawning" forever.  Just refresh the local store reference so
                        // next_agent_id() stays accurate.
                        if let Ok(fresh) = TeamStore::load_by_name(name, &ctx.working_dir) {
                            *store = fresh;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(tool = %tool_name, error = %e, "Spawn tool execution failed");
                // M8-T2: record the failure so it can be
                // surfaced to the lead in the tool output.
                let teammate_name = args
                    .get("teammate_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto")
                    .to_string();
                failed_spawns.push(serde_json::json!({
                    "index": i,
                    "teammate_name": teammate_name,
                    "tool": tool_name,
                    "error": e.to_string(),
                }));
            }
        }
    }
    failed_spawns
}

/// Build the spawn-tool arguments for spawn-prompts entry `i`: the entry's
/// `args` object plus any flattened keys, `profile` as an `agent_type` alias,
/// a default `team_name` and `teammate_name`, and the work context prepended
/// to the prompt.
fn spawn_prompt_args(
    obj: &serde_json::Map<String, serde_json::Value>,
    i: usize,
    name: &str,
    work_context: Option<&str>,
) -> serde_json::Value {
    // Extract args if present, else build from flattened fields
    let mut args = obj.get("args").cloned().unwrap_or(serde_json::json!({}));
    if !args.is_object() {
        args = serde_json::json!({});
    }

    // If spawn-prompts.json uses flattened keys, copy them into args
    // common flattened keys: team_name, teammate_name, prompt, agent_type, model, memory
    for key in &[
        "team_name",
        "teammate_name",
        "prompt",
        "agent_type",
        "model",
        "memory",
    ] {
        if args.get(*key).is_none()
            && let Some(v) = obj.get(*key)
        {
            args.as_object_mut()
                .map(|m| m.insert((*key).to_string(), v.clone()));
        }
    }

    // Allow "profile" as an alias for "agent_type" so
    // blueprints can reference declarative agent profiles.
    if args.get("agent_type").is_none()
        && let Some(v) = obj.get("profile")
    {
        args.as_object_mut()
            .map(|m| m.insert("agent_type".to_string(), v.clone()));
    }

    // Ensure team_name is present
    if args.get("team_name").is_none() {
        args.as_object_mut().map(|m| {
            m.insert(
                "team_name".to_string(),
                serde_json::Value::String(name.to_string()),
            )
        });
    }
    // Ensure teammate_name is present, otherwise auto-generate
    if args.get("teammate_name").is_none() {
        let teammate_name = format!("auto-{}", i + 1);
        args.as_object_mut().map(|m| {
            m.insert(
                "teammate_name".to_string(),
                serde_json::Value::String(teammate_name),
            )
        });
    }

    // Prepend work context to the spawn prompt so
    // teammates know which code to target.
    if let Some(ctx_text) = work_context {
        let original_prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let combined = format!("## Work Context\n{ctx_text}\n\n## Your Role\n{original_prompt}");
        args.as_object_mut()
            .map(|m| m.insert("prompt".to_string(), serde_json::Value::String(combined)));
    }
    args
}

/// Record a `Spawning` member for a spawn that returned `pending_manager`, so
/// the team config reflects the queued teammate. The store is reloaded fresh
/// before mutating so state written by previous tool calls is not clobbered,
/// and an existing member of the same name is left alone (idempotency).
fn record_spawning_member(args: &serde_json::Value, name: &str, ctx: &ToolContext) {
    let teammate_name = args
        .get("teammate_name")
        .and_then(|v| v.as_str())
        .unwrap_or("auto")
        .to_string();
    let agent_type_str = args
        .get("agent_type")
        .and_then(|v| v.as_str())
        .unwrap_or("general")
        .to_string();
    let prompt_str = args
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let model_override = args.get("model").and_then(|v| v.as_str()).and_then(|s| {
        s.split_once('/')
            .or_else(|| s.split_once(':'))
            .map(|(p, m)| crate::agent::ModelRef {
                provider_id: p.to_string(),
                model_id: m.to_string(),
            })
    });
    let memory_scope = match args.get("memory").and_then(|v| v.as_str()) {
        Some("user") => crate::team::MemoryScope::User,
        Some("project") => crate::team::MemoryScope::Project,
        _ => crate::team::MemoryScope::None,
    };
    if let Ok(mut fresh_store) = TeamStore::load_by_name(name, &ctx.working_dir) {
        // Only add if not already present (idempotency).
        if fresh_store.config.member_by_name(&teammate_name).is_none() {
            let agent_id = fresh_store.next_agent_id();
            let mut member =
                crate::team::config::TeamMember::new(&teammate_name, &agent_id, &agent_type_str);
            if !prompt_str.is_empty() {
                member.spawn_prompt = Some(prompt_str);
            }
            member.model_override = model_override;
            member.memory_scope = memory_scope;
            // INTENTIONAL: best-effort store rebuild
            let _ = fresh_store.add_member(member);
        }
    }
}

/// The roster line `team_create` reports: every member with its agent id and
/// status word, or a hint to use `team_spawn` when there are none.
fn member_summary(store: &TeamStore) -> String {
    let member_list: Vec<String> = store
        .config
        .members
        .iter()
        // FR-033: the roster this report hands back spells each state the
        // way the Teams panel does, not with the wire token.
        .map(|m| {
            format!(
                "  - {} ({}, {})",
                m.name,
                m.agent_id,
                crate::team::status_word(m.status)
            )
        })
        .collect();
    if member_list.is_empty() {
        "No teammates spawned yet. Use team_spawn to add teammates.".to_string()
    } else {
        format!(
            "Blueprint spawned {} teammate(s) - do NOT spawn these again:\n{}",
            member_list.len(),
            member_list.join("\n")
        )
    }
}

/// M8-T2: surface any per-prompt blueprint spawn failures to the lead.
/// Failed spawns are still logged via `tracing::error!`, but without
/// this summary the lead only sees a success response and silently
/// missing teammates. Empty when nothing failed.
fn failed_spawn_summary(failed_spawns: &[serde_json::Value]) -> String {
    if failed_spawns.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = failed_spawns
        .iter()
        .map(|f| {
            let name = f
                .get("teammate_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let tool = f.get("tool").and_then(|v| v.as_str()).unwrap_or("?");
            let err = f.get("error").and_then(|v| v.as_str()).unwrap_or("");
            format!("  - {name} ({tool}): {err}")
        })
        .collect();
    format!(
        "\n\n[!] {} blueprint spawn(s) failed:\n{}\n\
                         These teammates were not created. Re-spawn them manually with `team_spawn` if needed.",
        failed_spawns.len(),
        lines.join("\n")
    )
}
