use serde_json::Value;

use super::super::catalog::{visible_tools_for_mode, visible_tools_for_mode_with_policy};
use crate::core::session::AgentMode;

fn names(values: &[Value]) -> std::collections::BTreeSet<String> {
    values
        .iter()
        .map(|v| v["function"]["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn chat_mode_excludes_create_plan_only() {
    let tools = visible_tools_for_mode(AgentMode::Chat, false);
    let n = names(&tools);
    assert!(
        !n.contains("create_plan"),
        "CHAT mode must hide create_plan"
    );
    for kept in ["update_plan", "todos", "ask_question", "write", "bash"] {
        assert!(n.contains(kept), "CHAT must expose {kept}, got: {n:?}");
    }
}

#[test]
fn planning_mode_hides_whole_file_writers_but_keeps_plan_editing() {
    let tools = visible_tools_for_mode(AgentMode::Plan, false);
    let n = names(&tools);
    for plan_tool in ["create_plan", "update_plan", "todos", "ask_question"] {
        assert!(
            n.contains(plan_tool),
            "PLANNING must expose {plan_tool}, got: {n:?}"
        );
    }
    for hidden in ["write", "delete"] {
        assert!(
            !n.contains(hidden),
            "PLANNING must hide {hidden} at catalog layer, got: {n:?}"
        );
    }
    // 改计划正文还得靠 edit；bash 仍需要用于勘察。
    for kept in ["edit", "bash"] {
        assert!(n.contains(kept), "PLANNING must keep {kept}, got: {n:?}");
    }
}

#[test]
fn executing_mode_keeps_session_and_plan_todo_tools() {
    let tools = visible_tools_for_mode(AgentMode::Chat, true);
    let n = names(&tools);
    for kept in ["todos", "update_plan"] {
        assert!(n.contains(kept), "EXEC must keep {kept}, got: {n:?}");
    }
    for hidden in ["create_plan", "ask_question"] {
        assert!(!n.contains(hidden), "EXEC must hide {hidden}, got: {n:?}");
    }
    assert!(n.contains("write"), "EXEC must keep write at catalog layer");
    assert!(n.contains("bash"), "EXEC must keep bash");
}

#[test]
fn todos_stays_available_in_every_mode() {
    for (mode, executing) in [
        (AgentMode::Chat, false),
        (AgentMode::Plan, false),
        (AgentMode::Chat, true),
    ] {
        let n = names(&visible_tools_for_mode(mode, executing));
        assert!(
            n.contains("todos"),
            "{mode:?}/{executing} must expose todos, got: {n:?}"
        );
    }
}

#[test]
fn parked_plan_view_equals_chat_view() {
    let pending = visible_tools_for_mode(AgentMode::Chat, false);
    let chat = visible_tools_for_mode(AgentMode::Chat, false);
    assert_eq!(names(&pending), names(&chat));
}

#[test]
fn completed_plan_view_equals_chat_view() {
    let done = visible_tools_for_mode(AgentMode::Chat, false);
    let chat = visible_tools_for_mode(AgentMode::Chat, false);
    assert_eq!(names(&done), names(&chat));
}

#[test]
fn old_plan_state_equivalents_preserve_the_tool_visibility_matrix() {
    let cases = [
        ("chat", AgentMode::Chat, false, &["create_plan"][..]),
        ("planning", AgentMode::Plan, false, &["write", "delete"][..]),
        (
            "executing",
            AgentMode::Chat,
            true,
            &["create_plan", "ask_question"][..],
        ),
        ("pending", AgentMode::Chat, false, &["create_plan"][..]),
        ("completed", AgentMode::Chat, false, &["create_plan"][..]),
    ];

    for (old_state, mode, executing, hidden) in cases {
        let visible = names(&visible_tools_for_mode(mode, executing));
        for tool in hidden {
            assert!(
                !visible.contains(*tool),
                "{old_state} equivalent must hide {tool}; visible={visible:?}"
            );
        }
    }
}

#[test]
fn load_skill_can_be_hidden_by_policy() {
    let with_skill = names(&visible_tools_for_mode_with_policy(
        AgentMode::Chat,
        false,
        true,
    ));
    let without_skill = names(&visible_tools_for_mode_with_policy(
        AgentMode::Chat,
        false,
        false,
    ));
    assert!(with_skill.contains("load_skill"));
    assert!(!without_skill.contains("load_skill"));
}
