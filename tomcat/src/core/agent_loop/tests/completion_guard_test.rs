//! C1 completion guard：EXEC 下计划没收口时，模型不能靠一段文字结束回合。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::super::turn_finalize::{
    finalize_turn_after_text, TurnOutcome, MAX_COMPLETION_GUARD_INJECTIONS,
};
use super::super::{AgentLoop, AgentLoopConfig};
use super::mocks::{test_binding, MockLlmProvider, MockPrimitiveExecutor};
use crate::core::compaction::preheat::Preheat;
use crate::core::llm::{ChatMessage, MessageKind};
use crate::core::plan_runtime::file_store::{
    plan_path_for_id, write_plan, PlanFile, PlanFileFrontmatter, PlanFileState, TodoItem,
    TodoStatus,
};
use crate::core::plan_runtime::review::Finding;
use crate::core::plan_runtime::PlanRuntime;
use crate::core::session::manager::ContextState;
use crate::infra::event_bus::DefaultEventBus;

/// 计划文件写在 `$HOME/.tomcat/plans` 下，和 plan_tool 那批测试共享同一个进程环境变量，
/// 所以必须用同一把锁串行化，否则会互相把 HOME 抽走。
fn home_guard() -> crate::test_support::TestLockGuard<'static> {
    crate::test_support::home_env_lock().lock().unwrap()
}

fn unique_plan_id(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}

fn write_plan_file(plan_id: &str, state: PlanFileState, todos: Vec<TodoItem>) -> PathBuf {
    let path = plan_path_for_id(plan_id).unwrap();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let plan = PlanFile {
        frontmatter: PlanFileFrontmatter {
            plan_id: plan_id.to_string(),
            goal: "test".to_string(),
            state,
            session_key: Some("sess-guard".to_string()),
            session_id: Some("sid-guard".to_string()),
            created_at: "2026-07-27T00:00:00Z".to_string(),
            schema_version: 1,
            todos,
            unknown: serde_yaml::Mapping::new(),
        },
        body: "## body\n".to_string(),
    };
    write_plan(&path, &plan, 1_000).unwrap();
    path
}

fn cleanup_plan_file(path: &Path) {
    let _ = std::fs::remove_file(path);
    let lock = path.with_file_name(format!(
        "{}.lock",
        path.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_file(lock);
}

fn todo(id: &str, status: TodoStatus) -> TodoItem {
    TodoItem {
        id: id.to_string(),
        content: format!("work on {id}"),
        status,
    }
}

fn build_agent(plan_runtime: Option<Arc<PlanRuntime>>) -> AgentLoop {
    let mut agent = AgentLoop::new(
        test_binding(Arc::new(MockLlmProvider::new(vec![])), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            session_id: "sess-guard".to_string(),
            plan_runtime,
            ..Default::default()
        },
        CancellationToken::new(),
    );
    agent.set_context_state(Some(ContextState {
        messages: vec![],
        estimate_context_chars: 0,
        context_budget_chars: 100_000,
        context_budget_tokens: 25_000,
        last_api_usage: None,
        post_usage_appended_chars: 0,
        transcript_path: PathBuf::new(),
        latest_plan_event: None,
        resume_control: Default::default(),
        preheat: Preheat::new(),
        session_obs: Default::default(),
        live: Default::default(),
    }));
    agent
}

async fn finalize(agent: &mut AgentLoop, messages: &mut Vec<ChatMessage>) -> TurnOutcome {
    finalize_turn_after_text(
        agent,
        messages,
        "I have finished M1, let me know what to do next.",
        0,
        Some("stop".to_string()),
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("finalize should not fail")
}

#[tokio::test]
async fn guard_blocks_handback_while_todos_remain() {
    let _home = home_guard();
    let plan_id = unique_plan_id("guard_remaining");
    let plan_path = write_plan_file(
        &plan_id,
        PlanFileState::Executing,
        vec![
            todo("t1", TodoStatus::Completed),
            todo("t2", TodoStatus::InProgress),
            todo("t3", TodoStatus::Pending),
        ],
    );
    let plan_runtime = PlanRuntime::new("sess-guard");
    plan_runtime.set_executing_for_test(plan_id.clone());

    let mut agent = build_agent(Some(plan_runtime));
    let mut messages = vec![ChatMessage::user("start building")];
    let outcome = finalize(&mut agent, &mut messages).await;

    assert_eq!(outcome, TurnOutcome::Continue, "计划没收口不得结束回合");
    let injected = messages.last().unwrap();
    assert_eq!(injected.kind, MessageKind::Steering);
    let text = injected.text_content().unwrap_or("");
    assert!(text.contains("2 of 3 todos are not done"), "text={text}");
    assert!(text.contains("in_progress: t2"), "text={text}");

    cleanup_plan_file(&plan_path);
}

#[tokio::test]
async fn guard_blocks_handback_when_todos_done_but_review_pushed_back() {
    // 41 项 todo 全勾完、计划文件仍是 executing —— 只可能是 code review 没过。
    let _home = home_guard();
    let plan_id = unique_plan_id("guard_review");
    let plan_path = write_plan_file(
        &plan_id,
        PlanFileState::Executing,
        vec![
            todo("t1", TodoStatus::Completed),
            todo("t2", TodoStatus::Completed),
        ],
    );
    let plan_runtime = PlanRuntime::new("sess-guard");
    plan_runtime.set_executing_for_test(plan_id.clone());
    let findings = vec![
        Finding::new("concern".into(), "logic".into(), "missing null check".into()),
        Finding::new("nit".into(), "tests".into(), "no regression test".into()),
    ];
    plan_runtime.set_unresolved_findings(&plan_id, findings.clone());

    let mut agent = build_agent(Some(plan_runtime));
    let mut messages = vec![ChatMessage::user("start building")];
    let outcome = finalize(&mut agent, &mut messages).await;

    assert_eq!(outcome, TurnOutcome::Continue);
    let text = messages.last().unwrap().text_content().unwrap_or("");
    assert!(text.contains("code review has not passed"), "text={text}");
    let expected_ids = format!("{}, {}", findings[0].id, findings[1].id);
    assert!(text.contains(&expected_ids), "text={text}");

    cleanup_plan_file(&plan_path);
}

#[tokio::test]
async fn guard_stops_after_the_injection_cap_and_hands_back() {
    let _home = home_guard();
    let plan_id = unique_plan_id("guard_cap");
    let plan_path = write_plan_file(
        &plan_id,
        PlanFileState::Executing,
        vec![todo("t1", TodoStatus::Pending)],
    );
    let plan_runtime = PlanRuntime::new("sess-guard");
    plan_runtime.set_executing_for_test(plan_id.clone());

    let mut agent = build_agent(Some(plan_runtime));
    let mut messages = vec![ChatMessage::user("start building")];
    for round in 0..MAX_COMPLETION_GUARD_INJECTIONS {
        assert_eq!(
            finalize(&mut agent, &mut messages).await,
            TurnOutcome::Continue,
            "第 {round} 轮仍应注入"
        );
    }
    assert_eq!(
        finalize(&mut agent, &mut messages).await,
        TurnOutcome::Finished,
        "触顶后必须交还用户，不能无限打转"
    );

    cleanup_plan_file(&plan_path);
}

#[tokio::test]
async fn guard_does_not_fire_once_the_plan_file_leaves_executing() {
    let _home = home_guard();
    let plan_id = unique_plan_id("guard_completed");
    let plan_path = write_plan_file(
        &plan_id,
        PlanFileState::Completed,
        vec![todo("t1", TodoStatus::Completed)],
    );
    let plan_runtime = PlanRuntime::new("sess-guard");
    plan_runtime.set_executing_for_test(plan_id.clone());

    let mut agent = build_agent(Some(plan_runtime));
    let mut messages = vec![ChatMessage::user("start building")];
    assert_eq!(
        finalize(&mut agent, &mut messages).await,
        TurnOutcome::Finished
    );

    cleanup_plan_file(&plan_path);
}

#[tokio::test]
async fn guard_never_fires_outside_exec() {
    let mut agent = build_agent(Some(PlanRuntime::new("sess-guard")));
    let mut messages = vec![ChatMessage::user("just chatting")];
    assert_eq!(
        finalize(&mut agent, &mut messages).await,
        TurnOutcome::Finished,
        "CHAT 模式不注入"
    );

    let planning = PlanRuntime::new("sess-guard");
    planning.enter_planning().unwrap();
    let mut agent = build_agent(Some(planning));
    let mut messages = vec![ChatMessage::user("write me a plan")];
    assert_eq!(
        finalize(&mut agent, &mut messages).await,
        TurnOutcome::Finished,
        "PLAN 模式不注入"
    );
}
