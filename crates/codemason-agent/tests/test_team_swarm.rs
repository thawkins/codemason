//! Integration tests for `codemason-team` swarm decomposition parsing & prompting.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/team/swarm.rs`
//! (T-006 of the testconsolidate spec). All tested functions are public via
//! `codemason_agent::team::` re-exports.

use codemason_agent::team::{
    build_decomposition_user_prompt, parse_decomposition, parse_decomposition_with_default,
};

#[test]
fn test_parse_clean_json() {
    let input = r#"{"tasks":[{"id":"s1","title":"Setup","description":"Do setup","depends_on":[]},{"id":"s2","title":"Build","description":"Build it","depends_on":["s1"]}]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks.len(), 2);
    assert_eq!(dec.tasks[0].id, "s1");
    assert_eq!(dec.tasks[1].depends_on, vec!["s1"]);
}

#[test]
fn test_parse_with_markdown_fences() {
    let input = r#"```json
{"tasks":[{"id":"s1","title":"Only task","description":"Do it","depends_on":[]}]}
```"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks.len(), 1);
}

#[test]
fn test_parse_with_trailing_commas() {
    let input = r#"{"tasks":[{"id":"s1","title":"A","description":"B","depends_on":[],},]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks.len(), 1);
}

#[test]
fn test_parse_invalid_json() {
    let input = "not json at all";
    assert!(parse_decomposition(input).is_err());
}

#[test]
fn test_parse_normalises_agent_type() {
    let input = r#"{"tasks":[{"id":"s1","title":"Write tests","description":"Add unit tests for parser","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("coder"));
}

#[test]
fn test_parse_strips_embedded_agent_hint() {
    let input = r#"{"tasks":[{"id":"s1","title":"Review","description":"Audit [agent: code-review] the API","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition(input).unwrap();
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("code-review"));
    assert!(!dec.tasks[0].description.contains("[agent:"));
}

#[test]
fn test_parse_respects_default_agent_type_override() {
    let input = r#"{"tasks":[{"id":"s1","title":"Vague","description":"Do something","depends_on":[],"agent_type":null}]}"#;
    let dec = parse_decomposition_with_default(input, Some("explore")).unwrap();
    assert_eq!(dec.tasks[0].agent_type.as_deref(), Some("explore"));
}

#[test]
fn test_build_user_prompt() {
    let prompt = build_decomposition_user_prompt("Build a REST API");
    assert!(prompt.contains("Build a REST API"));
    assert!(prompt.contains("Decompose"));
}

// -- build_swarm_task_prompt / swarm_prompt_with_results ------------------------

use codemason_agent::team::{
    MAX_PREDECESSOR_RESULT_BYTES, SwarmSubtask, Task, TaskList, build_swarm_task_prompt,
    swarm_prompt_with_results,
};

fn subtask(id: &str, depends_on: &[&str]) -> SwarmSubtask {
    SwarmSubtask {
        id: id.to_string(),
        title: format!("Title {id}"),
        description: format!("Do the {id} work"),
        depends_on: depends_on.iter().map(|d| d.to_string()).collect(),
        agent_type: None,
        model: None,
    }
}

fn task_with_result(id: &str, result: Option<&str>) -> Task {
    let mut task = Task::new(id, format!("Task {id}"));
    task.result = result.map(str::to_owned);
    task
}

#[test]
fn test_swarm_prompt_without_dependencies_has_no_results_section() {
    let prompt = build_swarm_task_prompt(&subtask("s1", &[]), &[]);
    assert!(prompt.contains("**Task ID:** s1"));
    assert!(prompt.contains("Do the s1 work"));
    assert!(!prompt.contains("Results from dependencies"));
    assert!(prompt.contains("pass `result`"));
}

#[test]
fn test_swarm_prompt_includes_predecessor_results() {
    let dep = task_with_result("s1", Some("Found 3 handlers in src/auth/"));
    let prompt = build_swarm_task_prompt(&subtask("s2", &["s1"]), &[&dep]);
    assert!(prompt.contains("## Results from dependencies"));
    assert!(prompt.contains("### s1 - Task s1"));
    assert!(prompt.contains("Found 3 handlers in src/auth/"));
}

#[test]
fn test_swarm_prompt_marks_missing_predecessor_result() {
    let dep = task_with_result("s1", None);
    let prompt = build_swarm_task_prompt(&subtask("s2", &["s1"]), &[&dep]);
    assert!(prompt.contains("(no result recorded"));
}

#[test]
fn test_swarm_prompt_cuts_long_predecessor_result_on_char_boundary() {
    // Multi-byte characters make a byte cut land mid-character unless it is
    // moved to a boundary.
    let long = "\u{e9}".repeat(MAX_PREDECESSOR_RESULT_BYTES);
    let dep = task_with_result("s1", Some(&long));
    let prompt = build_swarm_task_prompt(&subtask("s2", &["s1"]), &[&dep]);
    assert!(prompt.contains("(result cut;"));
    let kept = prompt.matches('\u{e9}').count() * 2;
    assert!(kept <= MAX_PREDECESSOR_RESULT_BYTES);
}

#[test]
fn test_swarm_prompt_with_results_picks_dependencies_from_task_list() {
    let mut list = TaskList::default();
    list.tasks.push(task_with_result("s1", Some("result one")));
    list.tasks.push(task_with_result("s3", Some("unrelated")));
    let prompt = swarm_prompt_with_results(&subtask("s2", &["s1", "missing"]), Some(&list));
    assert!(prompt.contains("result one"));
    assert!(!prompt.contains("unrelated"));
    assert!(!prompt.contains("missing -"));

    let no_list = swarm_prompt_with_results(&subtask("s2", &["s1"]), None);
    assert!(!no_list.contains("Results from dependencies"));
}

// -- build_synthesis_user_prompt -------------------------------------------------

use codemason_agent::team::{TaskStatus, build_synthesis_user_prompt};

#[test]
fn test_synthesis_prompt_lists_goal_status_and_results() {
    let mut done = task_with_result("s1", Some("Added rate limiting to login.rs"));
    done.status = TaskStatus::Completed;
    let mut dropped = task_with_result("s2", None);
    dropped.status = TaskStatus::Cancelled;
    let prompt = build_synthesis_user_prompt("Harden auth", &[done, dropped]);
    assert!(prompt.starts_with("## Goal\n\nHarden auth"));
    assert!(prompt.contains("### s1 - Task s1 (completed)"));
    assert!(prompt.contains("Added rate limiting to login.rs"));
    assert!(prompt.contains("### s2 - Task s2 (cancelled)"));
    assert!(prompt.contains("(no result recorded)"));
}

#[test]
fn test_synthesis_prompt_marks_missing_goal() {
    let prompt = build_synthesis_user_prompt("  ", &[]);
    assert!(prompt.contains("(goal not recorded)"));
}
