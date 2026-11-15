//! Additional coverage for `codemason_app_core::widgets::message_widget`.
//!
//! `test_widgets_and_events_extra.rs` covers the widget rendering arms and a
//! subset of the `tool_input_summary` / `tool_result_summary` match arms. This
//! file fills the remaining summary arms (git / gitlab / github / lsp / codeindex
//! / spec / team / cron / memory) and the message-widget render branches those
//! tests leave untouched (multiedit per-file stats, agent_complete summary,
//! wait_agents task table, error without detail, red-marker continuation lines,
//! thinking-placeholder edges, and the `Widget` impl).
//!
//! Tests only - no production logic is touched.

use codemason_app_core::message_text::tool_result_summary;
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn result_summary(tool: &str, out: serde_json::Value, input: serde_json::Value) -> String {
    tool_result_summary(tool, &Some(out), &input, "/home/user/project")
        .unwrap_or_else(|| format!("<none:{tool}>"))
}

// ===========================================================================
// tool_result_summary arms not covered by test_widgets_and_events_extra
// ===========================================================================

#[test]
fn result_summary_subagent_arms() {
    assert_eq!(
        result_summary(
            "new_agent",
            json!({"agent": "explore", "status": "completed", "task_id": "abcdef0123456789"}),
            json!({})
        ),
        "completed agent explore → completed (abcdef01)"
    );
    assert_eq!(
        result_summary(
            "new_agent",
            json!({"agent": "build", "background": true, "task_id": "abcdef0123456789"}),
            json!({})
        ),
        "spawned build agent (abcdef01)"
    );
    assert_eq!(
        result_summary("cancel_agent", json!({"cancelled": true}), json!({})),
        "task cancelled"
    );
    assert_eq!(
        result_summary("cancel_agent", json!({"cancelled": false}), json!({})),
        "already completed"
    );
    assert_eq!(
        result_summary("list_agents", json!({"count": 3}), json!({})),
        "3 tasks"
    );
    assert_eq!(
        result_summary(
            "wait_agents",
            json!({"count": 2, "success": true}),
            json!({})
        ),
        "waited on 2 task(s)"
    );
}

#[test]
fn result_summary_team_arms() {
    assert_eq!(
        result_summary("team_create", json!({"name": "alpha"}), json!({})),
        "created alpha"
    );
    assert_eq!(
        result_summary(
            "team_spawn",
            json!({"teammate_name": "reviewer"}),
            json!({})
        ),
        "spawned reviewer"
    );
    assert_eq!(
        result_summary("team_status", json!({"members": 3}), json!({})),
        "3 member(s)"
    );
    assert_eq!(
        result_summary(
            "team_idle",
            json!({"idle_blocked": true, "blocked_by_task": "t-9"}),
            json!({})
        ),
        "blocked by task 't-9'"
    );
    assert_eq!(
        result_summary("team_idle", json!({}), json!({})),
        "marked idle"
    );
    assert_eq!(
        result_summary("team_cleanup", json!({}), json!({})),
        "cleanup complete"
    );
    assert_eq!(
        result_summary("team_broadcast", json!({}), json!({})),
        "broadcast sent"
    );
    assert_eq!(
        result_summary("team_message", json!({}), json!({})),
        "message sent"
    );
    assert_eq!(
        result_summary("team_read_messages", json!({"count": 2}), json!({})),
        "2 messages new"
    );
    assert_eq!(
        result_summary("team_task_create", json!({"task_id": "t-1"}), json!({})),
        "created task 't-1'"
    );
    assert_eq!(
        result_summary("team_task_list", json!({"count": 4}), json!({})),
        "4 tasks"
    );
    assert_eq!(
        result_summary(
            "team_task_claim",
            json!({"claimed": true, "task_id": "t-2"}),
            json!({})
        ),
        "claimed task 't-2'"
    );
    assert_eq!(
        result_summary(
            "team_task_claim",
            json!({"already_in_progress": true, "task_id": "t-2"}),
            json!({})
        ),
        "already has task 't-2'"
    );
    assert_eq!(
        result_summary("team_task_claim", json!({}), json!({})),
        "no tasks available"
    );
    assert_eq!(
        result_summary(
            "team_task_complete",
            json!({"completed": true, "task_id": "t-3"}),
            json!({})
        ),
        "completed task 't-3'"
    );
    assert_eq!(
        result_summary("team_task_complete", json!({}), json!({})),
        "task not found or already completed"
    );
    assert_eq!(
        result_summary(
            "team_assign_task",
            json!({"task_id": "t-4", "to": "bob"}),
            json!({})
        ),
        "assigned 't-4' to bob"
    );
    assert_eq!(
        result_summary("team_submit_plan", json!({}), json!({})),
        "plan submitted"
    );
    assert_eq!(
        result_summary("team_approve_plan", json!({"approved": true}), json!({})),
        "plan approved"
    );
    assert_eq!(
        result_summary("team_approve_plan", json!({"approved": false}), json!({})),
        "plan rejected"
    );
    assert_eq!(
        result_summary("team_memory_read", json!({"found": true}), json!({})),
        "memory read"
    );
    assert_eq!(
        result_summary("team_memory_read", json!({}), json!({})),
        "memory not found"
    );
    assert_eq!(
        result_summary("team_memory_write", json!({}), json!({})),
        "memory written"
    );
    assert_eq!(
        result_summary("team_wait", json!({"count": 2, "success": true}), json!({})),
        "waited on 2 agent(s)"
    );
    assert_eq!(
        result_summary(
            "team_wait",
            json!({"count": 2, "success": false}),
            json!({})
        ),
        "timeout waiting for 2 agent(s)"
    );
    assert_eq!(
        result_summary("team_shutdown_teammate", json!({}), json!({})),
        "shutdown requested"
    );
    assert_eq!(
        result_summary("team_shutdown_ack", json!({}), json!({})),
        "shutdown acknowledged"
    );
}

#[test]
fn result_summary_lsp_arms() {
    assert_eq!(
        result_summary("lsp_definition", json!({"count": 2}), json!({})),
        "2 locations found"
    );
    assert_eq!(
        result_summary("lsp_references", json!({"count": 1}), json!({})),
        "1 location found"
    );
    assert_eq!(
        result_summary(
            "lsp_symbols",
            json!({"symbol_count": 3, "path": "/home/user/project/src/lib.rs"}),
            json!({})
        ),
        "3 symbols in src/lib.rs"
    );
    assert_eq!(
        result_summary("lsp_hover", json!({"lines": 4}), json!({})),
        "4 lines of info"
    );
    assert_eq!(
        result_summary("lsp_diagnostics", json!({"total": 5}), json!({})),
        "5 diagnostics found"
    );
}

#[test]
fn result_summary_codeindex_arms() {
    assert_eq!(
        result_summary("codeindex_search", json!({"total_results": 7}), json!({})),
        "7 results found"
    );
    assert_eq!(
        result_summary("codeindex_symbols", json!({"total_results": 2}), json!({})),
        "2 symbols found"
    );
    assert_eq!(
        result_summary(
            "codeindex_references",
            json!({"total_results": 4}),
            json!({})
        ),
        "4 references found"
    );
    assert_eq!(
        result_summary(
            "codeindex_status",
            json!({"files_indexed": 10, "total_symbols": 200}),
            json!({})
        ),
        "10 files, 200 symbols"
    );
    assert_eq!(
        result_summary(
            "codeindex_reindex",
            json!({"files_added": 1, "files_updated": 2, "files_removed": 3}),
            json!({})
        ),
        "+1 ~2 -3 files"
    );
    assert_eq!(
        result_summary("codeindex_dependencies", json!({"count": 6}), json!({})),
        "6 dependencies found"
    );
    assert_eq!(
        result_summary("codeindex_godnodes", json!({"total_results": 5}), json!({})),
        "5 god nodes found"
    );
    assert_eq!(
        result_summary("codeindex_path", json!({"hops": 3}), json!({})),
        "3 hops found"
    );
    assert_eq!(
        result_summary(
            "codeindex_explain",
            json!({"name": "Foo", "degree": 9}),
            json!({})
        ),
        "Foo (degree 9)"
    );
    assert_eq!(
        result_summary(
            "codeindex_communities",
            json!({"total_communities": 4}),
            json!({})
        ),
        "4 communities found"
    );
}

#[test]
fn result_summary_spec_arms() {
    assert_eq!(
        result_summary("spec_list", json!({"count": 3}), json!({})),
        "3 specs listed"
    );
    assert_eq!(
        result_summary(
            "spec_read",
            json!({"requirement_count": 5, "task_count": 8}),
            json!({})
        ),
        "5 req, 8 tasks"
    );
    assert_eq!(
        result_summary("spec_search", json!({"count": 2}), json!({})),
        "2 results found"
    );
    assert_eq!(
        result_summary(
            "spec_task_update",
            json!({"spec_id": "auth", "task_id": "T-1", "status": "completed"}),
            json!({})
        ),
        "auth T-1 → completed"
    );
    assert_eq!(
        result_summary(
            "spec_coverage",
            json!({"coverage_pct": 87.5, "requirement_count": 8, "task_count": 8}),
            json!({})
        ),
        "87.5% (8 req, 8 tasks)"
    );
}

#[test]
fn result_summary_git_arms() {
    assert_eq!(
        result_summary("git_add", json!({"all": true}), json!({})),
        "add -A"
    );
    assert_eq!(
        result_summary("git_add", json!({"update": true}), json!({})),
        "add -u"
    );
    assert_eq!(
        result_summary("git_add", json!({"paths": ["a", "b"]}), json!({})),
        "add 2 path(s)"
    );
    assert_eq!(
        result_summary("git_branch", json!({"all": true}), json!({})),
        "branch list (all)"
    );
    assert_eq!(
        result_summary("git_branch", json!({"all": false}), json!({})),
        "branch list (local)"
    );
    assert_eq!(
        result_summary("git_checkout", json!({"branch": "main"}), json!({})),
        "checkout main"
    );
    assert_eq!(
        result_summary("git_checkout", json!({"paths": ["a"]}), json!({})),
        "checkout 1 path(s)"
    );
    assert_eq!(
        result_summary("git_checkout", json!({}), json!({})),
        "checkout"
    );
    assert_eq!(
        result_summary("git_cherry_pick", json!({"commits": ["a", "b"]}), json!({})),
        "cherry-pick 2 commit(s)"
    );
    assert_eq!(
        result_summary(
            "git_clone",
            json!({"url": "https://example.org/x.git"}),
            json!({})
        ),
        "clone https://example.org/x.git"
    );
    assert_eq!(
        result_summary("git_commit", json!({"message": "fix: thing"}), json!({})),
        "commit: fix: thing"
    );
    assert_eq!(
        result_summary("git_commit", json!({"amend": true}), json!({})),
        "commit --amend"
    );
    assert_eq!(
        result_summary("git_diff", json!({"target": "staged"}), json!({})),
        "diff staged"
    );
    assert_eq!(
        result_summary(
            "git_diff",
            json!({"target": "HEAD", "stat": true}),
            json!({})
        ),
        "diff --stat HEAD"
    );
    assert_eq!(
        result_summary("git_fetch", json!({"remote": "upstream"}), json!({})),
        "fetch upstream"
    );
    assert_eq!(
        result_summary("git_fetch", json!({"all": true}), json!({})),
        "fetch --all"
    );
    assert_eq!(
        result_summary("git_log", json!({"limit": 5, "branch": "main"}), json!({})),
        "log main (5)"
    );
    assert_eq!(result_summary("git_log", json!({}), json!({})), "log (20)");
    assert_eq!(
        result_summary("git_merge", json!({"branch": "feature"}), json!({})),
        "merge feature"
    );
    assert_eq!(
        result_summary("git_pull", json!({"remote": "origin"}), json!({})),
        "pull origin"
    );
    assert_eq!(
        result_summary(
            "git_pull",
            json!({"remote": "origin", "rebase": true}),
            json!({})
        ),
        "pull --rebase origin"
    );
    assert_eq!(
        result_summary(
            "git_push",
            json!({"remote": "origin", "tags": true}),
            json!({})
        ),
        "push --tags origin"
    );
    assert_eq!(
        result_summary(
            "git_push",
            json!({"remote": "origin", "force": true, "branch": "main"}),
            json!({})
        ),
        "push --force-with-lease origin/main"
    );
    assert_eq!(
        result_summary(
            "git_push",
            json!({"remote": "origin", "force": true}),
            json!({})
        ),
        "push --force-with-lease origin"
    );
    assert_eq!(
        result_summary(
            "git_push",
            json!({"remote": "origin", "branch": "main"}),
            json!({})
        ),
        "push origin/main"
    );
    assert_eq!(
        result_summary("git_push", json!({}), json!({})),
        "push origin"
    );
    assert_eq!(
        result_summary("git_remote", json!({"action": "list"}), json!({})),
        "remote -v"
    );
    assert_eq!(
        result_summary(
            "git_remote",
            json!({"action": "add", "name": "origin"}),
            json!({})
        ),
        "remote add origin"
    );
    assert_eq!(
        result_summary(
            "git_remote",
            json!({"action": "remove", "name": "origin"}),
            json!({})
        ),
        "remote remove origin"
    );
    assert_eq!(
        result_summary(
            "git_remote",
            json!({"action": "set-url", "name": "origin"}),
            json!({})
        ),
        "remote set-url origin"
    );
    assert_eq!(
        result_summary(
            "git_remote",
            json!({"action": "rename", "name": "x"}),
            json!({})
        ),
        "remote rename x"
    );
    assert_eq!(
        result_summary("git_remote", json!({"action": "rename"}), json!({})),
        "remote rename"
    );
    assert_eq!(
        result_summary(
            "git_reset",
            json!({"mode": "hard", "target": "HEAD~1"}),
            json!({})
        ),
        "reset --hard HEAD~1"
    );
    assert_eq!(
        result_summary("git_reset", json!({"paths": ["a", "b"]}), json!({})),
        "reset 2 path(s)"
    );
    assert_eq!(
        result_summary("git_reset", json!({"mode": "soft"}), json!({})),
        "reset --soft"
    );
    assert_eq!(
        result_summary("git_show", json!({"ref": "v1.0"}), json!({})),
        "show v1.0"
    );
    assert_eq!(
        result_summary("git_show", json!({}), json!({})),
        "show HEAD"
    );
    assert_eq!(
        result_summary("git_stash", json!({"action": "pop"}), json!({})),
        "stash pop"
    );
    assert_eq!(
        result_summary("git_status", json!({"short": true}), json!({})),
        "status --short"
    );
    assert_eq!(result_summary("git_status", json!({}), json!({})), "status");
    assert_eq!(
        result_summary("git_tag", json!({"action": "list"}), json!({})),
        "tag -l"
    );
    assert_eq!(
        result_summary(
            "git_tag",
            json!({"action": "create", "name": "v1"}),
            json!({})
        ),
        "tag v1"
    );
    assert_eq!(
        result_summary(
            "git_tag",
            json!({"action": "delete", "name": "v1"}),
            json!({})
        ),
        "tag -d v1"
    );
    assert_eq!(
        result_summary(
            "git_tag",
            json!({"action": "show", "name": "v1"}),
            json!({})
        ),
        "tag show v1"
    );
    assert_eq!(
        result_summary("git_tag", json!({"action": "show"}), json!({})),
        "tag show"
    );
}

#[test]
fn result_summary_gitlab_arms() {
    assert_eq!(
        result_summary("gitlab_list_issues", json!({"state": "opened"}), json!({})),
        "issues (opened)"
    );
    assert_eq!(
        result_summary("gitlab_get_issue", json!({"iid": 12}), json!({})),
        "issue #12"
    );
    assert_eq!(
        result_summary("gitlab_create_issue", json!({"title": "Bug"}), json!({})),
        "new issue: Bug"
    );
    assert_eq!(
        result_summary("gitlab_comment_issue", json!({"iid": 3}), json!({})),
        "comment on #3"
    );
    assert_eq!(
        result_summary("gitlab_close_issue", json!({"iid": 4}), json!({})),
        "close #4"
    );
    assert_eq!(
        result_summary("gitlab_list_mrs", json!({"state": "merged"}), json!({})),
        "MRs (merged)"
    );
    assert_eq!(
        result_summary("gitlab_get_mr", json!({"iid": 7}), json!({})),
        "MR #7"
    );
    assert_eq!(
        result_summary("gitlab_create_mr", json!({"title": "Feat"}), json!({})),
        "new MR: Feat"
    );
    assert_eq!(
        result_summary("gitlab_merge_mr", json!({"iid": 8}), json!({})),
        "merge !8"
    );
    assert_eq!(
        result_summary("gitlab_approve_mr", json!({"iid": 9}), json!({})),
        "approve !9"
    );
    assert_eq!(
        result_summary(
            "gitlab_list_pipelines",
            json!({"status": "failed"}),
            json!({})
        ),
        "pipelines (failed)"
    );
    assert_eq!(
        result_summary("gitlab_list_pipelines", json!({}), json!({})),
        "pipelines"
    );
    assert_eq!(
        result_summary("gitlab_get_pipeline", json!({"pipeline_id": 5}), json!({})),
        "pipeline #5"
    );
    assert_eq!(
        result_summary("gitlab_list_jobs", json!({"pipeline_id": 5}), json!({})),
        "jobs for pipeline #5"
    );
    assert_eq!(
        result_summary("gitlab_get_job", json!({"job_id": 6}), json!({})),
        "job #6"
    );
    assert_eq!(
        result_summary("gitlab_get_job_log", json!({"job_id": 6}), json!({})),
        "log for job #6"
    );
    assert_eq!(
        result_summary("gitlab_retry_job", json!({"job_id": 6}), json!({})),
        "retry job #6"
    );
    assert_eq!(
        result_summary("gitlab_cancel_job", json!({"job_id": 6}), json!({})),
        "cancel job #6"
    );
    assert_eq!(
        result_summary(
            "gitlab_retry_pipeline",
            json!({"pipeline_id": 5}),
            json!({})
        ),
        "retry pipeline #5"
    );
    assert_eq!(
        result_summary(
            "gitlab_cancel_pipeline",
            json!({"pipeline_id": 5}),
            json!({})
        ),
        "cancel pipeline #5"
    );
}

#[test]
fn result_summary_github_arms() {
    assert_eq!(
        result_summary("github_list_issues", json!({"state": "open"}), json!({})),
        "issues listed (open)"
    );
    assert_eq!(
        result_summary("github_get_issue", json!({"number": 11}), json!({})),
        "issue #11 retrieved"
    );
    assert_eq!(
        result_summary("github_create_issue", json!({"number": 12}), json!({})),
        "issue #12 created"
    );
    assert_eq!(
        result_summary("github_create_issue", json!({}), json!({})),
        "issue created"
    );
    assert_eq!(
        result_summary("github_comment_issue", json!({"number": 13}), json!({})),
        "commented on #13"
    );
    assert_eq!(
        result_summary("github_close_issue", json!({"number": 14}), json!({})),
        "issue #14 closed"
    );
    assert_eq!(
        result_summary("github_list_prs", json!({"state": "closed"}), json!({})),
        "PRs listed (closed)"
    );
    assert_eq!(
        result_summary("github_get_pr", json!({"number": 21}), json!({})),
        "PR #21 retrieved"
    );
    assert_eq!(
        result_summary("github_create_pr", json!({"number": 22}), json!({})),
        "PR #22 created"
    );
    assert_eq!(
        result_summary("github_create_pr", json!({}), json!({})),
        "PR created"
    );
    assert_eq!(
        result_summary("github_merge_pr", json!({"number": 23}), json!({})),
        "PR #23 merged"
    );
    assert_eq!(
        result_summary("github_review_pr", json!({"number": 24}), json!({})),
        "reviewed #24"
    );
}

#[test]
fn result_summary_misc_arms() {
    assert_eq!(
        result_summary("metadata", json!({}), json!({})),
        "metadata extracted"
    );
    assert_eq!(
        result_summary(
            "truncate",
            json!({"original": 100, "truncated": 40}),
            json!({})
        ),
        "100 → 40 chars"
    );
    assert_eq!(
        result_summary(
            "read_line_range",
            json!({"start_line": 5, "end_line": 9}),
            json!({})
        ),
        "lines 5-9"
    );
    assert_eq!(
        result_summary("read_line_range", json!({"start_line": 5}), json!({})),
        "from line 5"
    );
    assert_eq!(
        result_summary("read_line_range", json!({}), json!({})),
        "line range"
    );
    assert_eq!(
        result_summary("memory_recall", json!({"count": 4}), json!({})),
        "4 memories recalled"
    );
    assert_eq!(
        result_summary("memory_forget", json!({"deleted": 2}), json!({})),
        "2 memories forgotten"
    );
    assert_eq!(
        result_summary("memory_forget", json!({"deleted": 0}), json!({})),
        "nothing to forget"
    );
    assert_eq!(
        result_summary("cron_add", json!({"id": "nightly"}), json!({})),
        "`nightly` scheduled"
    );
    assert_eq!(
        result_summary(
            "cron_remove",
            json!({"removed": true, "id": "nightly"}),
            json!({})
        ),
        "`nightly` removed"
    );
    assert_eq!(
        result_summary(
            "cron_remove",
            json!({"removed": false, "id": "nightly"}),
            json!({})
        ),
        "`nightly` not found"
    );
    assert_eq!(
        result_summary("cron_list", json!({"count": 3}), json!({})),
        "3 events listed"
    );
    assert_eq!(
        result_summary(
            "cron_enable",
            json!({"enabled": true, "id": "nightly"}),
            json!({})
        ),
        "`nightly` enabled"
    );
    assert_eq!(
        result_summary(
            "cron_disable",
            json!({"enabled": false, "id": "nightly"}),
            json!({})
        ),
        "`nightly` disabled"
    );
    assert_eq!(
        result_summary("spec_coverage", json!({"coverage_pct": 0.0}), json!({})),
        "0.0% (0 req, 0 tasks)"
    );
}

#[test]
fn result_summary_pdf_write_uses_path() {
    let r = result_summary(
        "pdf_write",
        json!({"pages": 3}),
        json!({"path": "/home/user/project/out/report.pdf"}),
    );
    assert!(r.contains("out/report.pdf"), "got: {r}");
}

// ===========================================================================
// tool_input_summary arms not covered elsewhere
// ===========================================================================

// ===========================================================================
// MessageWidget render arms not covered elsewhere
// ===========================================================================

// ---------------------------------------------------------------------------
// tool_input_summary arms (restored: app-core logic the terminal tests reached
// through a `widgets::message_widget` re-export)
// ---------------------------------------------------------------------------

#[test]
fn input_summary_git_arms() {
    use codemason_app_core::message_text::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary("git_add", &json!({"all": true}), cwd),
        "add -A"
    );
    assert_eq!(
        tool_input_summary("git_add", &json!({"update": true}), cwd),
        "add -u"
    );
    assert_eq!(
        tool_input_summary("git_branch", &json!({"all": false}), cwd),
        "branch list (local)"
    );
    assert_eq!(
        tool_input_summary("git_checkout", &json!({"branch": "main"}), cwd),
        "checkout main"
    );
    assert_eq!(
        tool_input_summary(
            "git_clone",
            &json!({"url": "https://example.org/x.git"}),
            cwd
        ),
        "clone https://example.org/x.git"
    );
    assert_eq!(
        tool_input_summary("git_commit", &json!({"message": "fix"}), cwd),
        "commit: fix"
    );
    assert_eq!(
        tool_input_summary("git_diff", &json!({"target": "staged"}), cwd),
        "diff staged"
    );
    assert_eq!(
        tool_input_summary("git_push", &json!({"remote": "origin"}), cwd),
        "push origin"
    );
    assert_eq!(
        tool_input_summary("git_status", &json!({"short": true}), cwd),
        "status --short"
    );
    assert_eq!(
        tool_input_summary("git_show", &json!({"ref": "HEAD"}), cwd),
        "show HEAD"
    );
    assert_eq!(
        tool_input_summary("git_stash", &json!({"action": "pop"}), cwd),
        "stash pop"
    );
    assert_eq!(tool_input_summary("git_tag", &json!({}), cwd), "tag -l");
}

#[test]
fn input_summary_gitlab_arms() {
    use codemason_app_core::message_text::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary("gitlab_list_issues", &json!({"state": "opened"}), cwd),
        "issues (opened)"
    );
    assert_eq!(
        tool_input_summary("gitlab_get_mr", &json!({"iid": 7}), cwd),
        "MR #7"
    );
    assert_eq!(
        tool_input_summary("gitlab_create_mr", &json!({"title": "Feat"}), cwd),
        "new MR: Feat"
    );
    assert_eq!(
        tool_input_summary("gitlab_merge_mr", &json!({"iid": 8}), cwd),
        "merge !8"
    );
    assert_eq!(
        tool_input_summary("gitlab_get_job", &json!({"job_id": 9}), cwd),
        "job #9"
    );
    assert_eq!(
        tool_input_summary("gitlab_cancel_pipeline", &json!({"pipeline_id": 5}), cwd),
        "cancel pipeline #5"
    );
}

#[test]
fn input_summary_github_arms() {
    use codemason_app_core::message_text::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary("github_list_issues", &json!({"state": "open"}), cwd),
        "issues (open)"
    );
    assert_eq!(
        tool_input_summary("github_get_issue", &json!({"number": 5}), cwd),
        "issue #5"
    );
    assert_eq!(
        tool_input_summary("github_create_issue", &json!({"title": "Bug"}), cwd),
        "new issue: Bug"
    );
    assert_eq!(
        tool_input_summary("github_create_pr", &json!({"title": "Feat"}), cwd),
        "new PR: Feat"
    );
    assert_eq!(
        tool_input_summary("github_merge_pr", &json!({"number": 9}), cwd),
        "merge #9"
    );
    assert_eq!(
        tool_input_summary("github_review_pr", &json!({"number": 9}), cwd),
        "review #9"
    );
}

#[test]
fn input_summary_lsp_codeindex_spec_team_arms() {
    use codemason_app_core::message_text::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary(
            "lsp_definition",
            &json!({"path": "src/lib.rs", "line": 3, "column": 5}),
            cwd
        ),
        "src/lib.rs L3:5"
    );
    assert_eq!(
        tool_input_summary("lsp_symbols", &json!({"path": "src/lib.rs"}), cwd),
        "src/lib.rs"
    );
    assert_eq!(
        tool_input_summary("lsp_diagnostics", &json!({}), cwd),
        "analyze"
    );
    assert_eq!(
        tool_input_summary(
            "codeindex_search",
            &json!({"query": "foo", "kind": "function", "language": "rust"}),
            cwd
        ),
        "\"foo\" kind:function lang:rust"
    );
    assert_eq!(
        tool_input_summary("codeindex_status", &json!({"enabled": false}), cwd),
        "status: off"
    );
    assert_eq!(
        tool_input_summary("codeindex_reindex", &json!({}), cwd),
        "reindex"
    );
    assert_eq!(
        tool_input_summary("codeindex_godnodes", &json!({"n": 5}), cwd),
        "god nodes (top 5)"
    );
    assert_eq!(
        tool_input_summary("codeindex_communities", &json!({}), cwd),
        "communities"
    );
    assert_eq!(
        tool_input_summary("spec_list", &json!({"status": "draft"}), cwd),
        "list (draft)"
    );
    assert_eq!(
        tool_input_summary("spec_read", &json!({"spec_id": "auth"}), cwd),
        "read auth"
    );
    assert_eq!(
        tool_input_summary("spec_search", &json!({"query": "login"}), cwd),
        "search \"login\""
    );
    assert_eq!(
        tool_input_summary("spec_coverage", &json!({"spec_id": "auth"}), cwd),
        "coverage auth"
    );
    assert_eq!(
        tool_input_summary("team_create", &json!({"name": "alpha"}), cwd),
        "create alpha"
    );
    assert_eq!(tool_input_summary("team_status", &json!({}), cwd), "status");
    assert_eq!(
        tool_input_summary("team_task_list", &json!({}), cwd),
        "list tasks"
    );
    assert_eq!(
        tool_input_summary("team_read_messages", &json!({}), cwd),
        "read messages"
    );
    assert_eq!(
        tool_input_summary("team_submit_plan", &json!({}), cwd),
        "submit plan"
    );
    assert_eq!(
        tool_input_summary("cancel_agent", &json!({"task_id": "abcdef0123456789"}), cwd),
        "cancel abcdef01"
    );
    assert_eq!(
        tool_input_summary("list_agents", &json!({"status": "running"}), cwd),
        "filter: running"
    );
}

#[test]
fn input_summary_misc_arms() {
    use codemason_app_core::message_text::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(tool_input_summary("metadata", &json!({}), cwd), "metadata");
    assert_eq!(tool_input_summary("truncate", &json!({}), cwd), "truncate");
    assert_eq!(
        tool_input_summary("read_line_range", &json!({"start_line": 1}), cwd),
        "line range"
    );
    assert_eq!(
        tool_input_summary(
            "pdf_write",
            &json!({"path": "/home/user/project/out.pdf"}),
            cwd
        ),
        "out.pdf"
    );
    assert_eq!(
        tool_input_summary("memory_recall", &json!({"query": "fact"}), cwd),
        "recall \"fact\""
    );
    assert_eq!(
        tool_input_summary("memory_forget", &json!({"id": "3"}), cwd),
        "forget 3"
    );
    assert_eq!(
        tool_input_summary(
            "cron_add",
            &json!({"id": "nightly", "schedule": "every 1h"}),
            cwd
        ),
        "add `nightly` every 1h"
    );
    assert_eq!(
        tool_input_summary("cron_remove", &json!({"id": "nightly"}), cwd),
        "remove `nightly`"
    );
    assert_eq!(
        tool_input_summary("cron_list", &json!({}), cwd),
        "list events"
    );
    assert_eq!(
        tool_input_summary("cron_enable", &json!({"id": "nightly"}), cwd),
        "enable `nightly`"
    );
    assert_eq!(
        tool_input_summary("cron_disable", &json!({"id": "nightly"}), cwd),
        "disable `nightly`"
    );
    assert_eq!(
        tool_input_summary("model_info", &json!({"format": "json"}), cwd),
        "model info (json)"
    );
}

#[test]
fn input_summary_codeindex_dependencies_and_explain() {
    use codemason_app_core::message_text::tool_input_summary;
    let cwd = "/home/user/project";
    let s = tool_input_summary(
        "codeindex_dependencies",
        &json!({"path": "src/lib.rs", "direction": "dependents"}),
        cwd,
    );
    assert_eq!(s, "dependents: src/lib.rs");
    let s = tool_input_summary("codeindex_explain", &json!({"symbol": "Foo"}), cwd);
    assert_eq!(s, "explain Foo");
    let s = tool_input_summary("codeindex_path", &json!({"from": "A", "to": "B"}), cwd);
    assert_eq!(s, "path A → B");
    let s = tool_input_summary("codeindex_references", &json!({"symbol": "Foo"}), cwd);
    assert_eq!(s, "refs: Foo");
}
