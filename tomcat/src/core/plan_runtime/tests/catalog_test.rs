use serde_json::Value;

use super::super::catalog::{visible_tools_for_mode, visible_tools_for_mode_with_policy};
use super::super::state::PlanState;

fn names(values: &[Value]) -> std::collections::BTreeSet<String> {
    values
        .iter()
        .map(|v| v["function"]["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn chat_mode_excludes_create_plan_only() {
    let tools = visible_tools_for_mode(&PlanState::Chat);
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
    let tools = visible_tools_for_mode(&PlanState::Planning);
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
fn executing_mode_leaves_update_plan_as_the_only_progress_authority() {
    let tools = visible_tools_for_mode(&PlanState::Executing {
        plan_id: "demo".into(),
    });
    let n = names(&tools);
    assert!(n.contains("update_plan"), "EXEC must keep update_plan");
    // todos 会开出第二份进度清单，与计划文件互相矛盾；EXEC 下只留 update_plan。
    for hidden in ["create_plan", "ask_question", "todos"] {
        assert!(!n.contains(hidden), "EXEC must hide {hidden}, got: {n:?}");
    }
    assert!(n.contains("write"), "EXEC must keep write at catalog layer");
    assert!(n.contains("bash"), "EXEC must keep bash");
}

#[test]
fn todos_stays_available_outside_exec() {
    for mode in [
        PlanState::Chat,
        PlanState::Planning,
        PlanState::Pending {
            plan_id: "demo".into(),
        },
        PlanState::Completed {
            plan_id: "demo".into(),
        },
    ] {
        let n = names(&visible_tools_for_mode(&mode));
        assert!(n.contains("todos"), "{mode:?} must expose todos, got: {n:?}");
    }
}

#[test]
fn pending_mode_view_equals_chat_view() {
    let pending = visible_tools_for_mode(&PlanState::Pending {
        plan_id: "demo".into(),
    });
    let chat = visible_tools_for_mode(&PlanState::Chat);
    assert_eq!(names(&pending), names(&chat));
}

#[test]
fn completed_mode_view_equals_chat_view() {
    let done = visible_tools_for_mode(&PlanState::Completed {
        plan_id: "demo".into(),
    });
    let chat = visible_tools_for_mode(&PlanState::Chat);
    assert_eq!(names(&done), names(&chat));
}

#[test]
fn load_skill_can_be_hidden_by_policy() {
    let with_skill = names(&visible_tools_for_mode_with_policy(&PlanState::Chat, true));
    let without_skill = names(&visible_tools_for_mode_with_policy(&PlanState::Chat, false));
    assert!(with_skill.contains("load_skill"));
    assert!(!without_skill.contains("load_skill"));
}
