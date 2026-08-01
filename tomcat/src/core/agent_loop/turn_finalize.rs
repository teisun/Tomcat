//! # Reasoning Loop 收束分支：text-only 回合的 timing ⑤ + TurnEnd 发射
//!
//! 当 LLM 本轮**没有产出 tool_calls**（纯文本回复）时，reasoning loop 不再继续，
//! 进入"收束分支"做四步 cleanup：
//!
//! 1. `on_message_appended(content_buf.len())` + `messages.push(assistant)`
//! 2. **Timing ⑤**：L0 cleanup → preheat.try_restart_if_pending → L2
//!    `check_after_reply`（仅 ratio ≥ 0.85）→ preheat.try_start（Idle → Running）
//! 3. 条件发射 `Layer0ContextRelease` / `AutoCompactionStart`
//! 4. `emit_context_metrics()` + `TurnEnd { tool_results: [] }`
//!
//! 历史：原嵌在 `run.rs::run_reasoning_loop` 的 `if tool_calls.is_empty()` 分支
//! 内（约 80 行）。Phase 3 抽出为本文件的自由函数后，`run_reasoning_loop` 主体
//! 只关心"取消预检 / TurnStart / Stream 调度 / Tool Dispatch"四件事，骨架更清晰。

use std::sync::Arc;

use crate::core::compaction::run_layer0_cleanup;
use crate::core::llm::{
    ChatMessage, ContinuityMetadata, MessageKind, ReasoningContinuation, TokenUsage,
};
use crate::core::plan_runtime::file_store::{self, PlanFileState, TodoStatus};
use crate::core::plan_runtime::{PlanRuntime, PlanState};
use crate::core::session::manager::estimated_tokens_from_chars;
use crate::infra::events::{AgentEvent, Message};

use super::types::AgentLoop;

/// 连续注入上限。到顶后停止注入、交还用户，避免模型卡在同一个坎上无限打转。
pub(super) const MAX_COMPLETION_GUARD_INJECTIONS: u32 = 8;

/// text-only 回合的处理结果。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TurnOutcome {
    /// 回合正常结束，reasoning loop 可以返回。
    Finished,
    /// 计划还没做完，已注入继续指令，reasoning loop 应再跑一轮。
    Continue,
}

/// 计划尚未收口时，模型不能靠"回一段文字"结束回合。
///
/// 判据用计划文件的 `state` 而不是"还有没有未完成的 todo"：
/// todo 可能全勾完了但 code review 打回，此时 state 仍是 `executing`，活儿也确实没干完。
fn completion_guard_instruction(plan_runtime: &PlanRuntime) -> Option<String> {
    let PlanState::Executing { plan_id } = plan_runtime.mode() else {
        return None;
    };
    let plan = plan_runtime
        .active_plan_path()
        .and_then(|path| file_store::read_plan(&path).ok())?;
    if plan.frontmatter.state != PlanFileState::Executing {
        return None;
    }

    let unfinished: Vec<&file_store::TodoItem> = plan
        .frontmatter
        .todos
        .iter()
        .filter(|todo| matches!(todo.status, TodoStatus::Pending | TodoStatus::InProgress))
        .collect();

    if !unfinished.is_empty() {
        let in_progress: Vec<&str> = unfinished
            .iter()
            .filter(|todo| todo.status == TodoStatus::InProgress)
            .map(|todo| todo.id.as_str())
            .collect();
        let in_progress = if in_progress.is_empty() {
            "(none)".to_string()
        } else {
            in_progress.join(", ")
        };
        return Some(format!(
            "The plan `{plan_id}` is still `executing`: {} of {} todos are not done yet \
             (in_progress: {in_progress}). Keep working — pick up the next todo and call tools. \
             Do not summarize or hand back until the plan reaches `completed`.",
            unfinished.len(),
            plan.frontmatter.todos.len(),
        ));
    }

    // 所有 todo 都勾完了却仍是 executing —— 只可能是 code review 把计划打了回来。
    let findings = plan_runtime.unresolved_finding_ids(&plan_id);
    let findings = if findings.is_empty() {
        "(see the latest code review result)".to_string()
    } else {
        findings.join(", ")
    };
    Some(format!(
        "All todos of plan `{plan_id}` are checked off but the plan is still `executing`, \
         which means code review has not passed. Unresolved findings: {findings}. \
         Fix them and re-run the review. Do not hand back with findings outstanding."
    ))
}

fn should_apply_completion_guard(agent: &AgentLoop) -> Option<String> {
    if !agent.config.subagent_type.is_root() {
        return None;
    }
    agent
        .config
        .plan_runtime
        .as_ref()
        .and_then(|rt| completion_guard_instruction(rt))
}
/// 处理 text-only 回合的全部副作用：消息落盘、timing ⑤、收束事件发射。
///
/// **必须在 `tool_calls.is_empty()` 分支调用，且仅调用一次**——重复调用会重复
/// `on_message_appended` 计费、重复发 `TurnEnd`。
///
/// `content_buf`：本轮 delta 累积。`turn_index`：作为 `TurnEnd` 的 turn 序号。
///
/// 这是不带 usage 的测试便利包装；生产路径统一调用
/// [`finalize_turn_after_text_with_usage`]，以保留 provider usage。
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) async fn finalize_turn_after_text(
    agent: &mut AgentLoop,
    messages: &mut Vec<ChatMessage>,
    content_buf: &str,
    turn_index: usize,
    finish_reason: Option<String>,
    error_message: Option<String>,
    error_code: Option<String>,
    thinking_text: Option<String>,
    reasoning_continuation: Option<ReasoningContinuation>,
    continuity: Option<ContinuityMetadata>,
) -> Result<TurnOutcome, crate::infra::error::AppError> {
    finalize_turn_after_text_with_usage(
        agent,
        messages,
        content_buf,
        turn_index,
        finish_reason,
        error_message,
        error_code,
        thinking_text,
        reasoning_continuation,
        continuity,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finalize_turn_after_text_with_usage(
    agent: &mut AgentLoop,
    messages: &mut Vec<ChatMessage>,
    content_buf: &str,
    turn_index: usize,
    finish_reason: Option<String>,
    error_message: Option<String>,
    error_code: Option<String>,
    thinking_text: Option<String>,
    reasoning_continuation: Option<ReasoningContinuation>,
    continuity: Option<ContinuityMetadata>,
    usage: Option<TokenUsage>,
) -> Result<TurnOutcome, crate::infra::error::AppError> {
    if let Some(ref mut ctx_state) = agent.context_state {
        ctx_state.on_message_appended(content_buf.len());
    }
    let forced_id = agent.take_or_mint_pending_assistant_entry_id();
    let assistant_message_id = agent.push_message_with_forced_id(
        messages,
        ChatMessage::assistant(content_buf)
            .with_completion_metadata(finish_reason, error_message, error_code)
            .with_reasoning_state(thinking_text, reasoning_continuation, continuity)
            .with_usage(usage),
        &forced_id,
    )?;

    // Completion guard：计划还没收口就想用一段文字收工时，注入继续指令并把回合续上。
    // 放在 timing ⑤ 之前 —— 这个回合根本没结束，不该走收束流程。
    if agent.completion_guard_injections < MAX_COMPLETION_GUARD_INJECTIONS {
        if let Some(instruction) = should_apply_completion_guard(agent) {
            agent.completion_guard_injections += 1;
            let mut nudge = ChatMessage::user(&instruction);
            nudge.kind = MessageKind::Steering;
            if let Some(ref mut ctx_state) = agent.context_state {
                ctx_state.on_message_appended(instruction.len());
            }
            messages.push(nudge);
            return Ok(TurnOutcome::Continue);
        }
    }

    // Timing ⑤: L0 → try_restart → check_after_reply → try_start → metrics
    let compaction_provider = agent.compaction_provider();
    let compaction_emitter = Arc::new(agent.emitter.clone());
    let control_snapshot = agent
        .config
        .plan_runtime
        .as_ref()
        .map(|rt| rt.control_snapshot(Some(agent.wire_model())));
    let mut preheat_started: Option<(usize, f64)> = None;
    let mut layer0_release: Option<(usize, usize)> = None;
    if let Some(ref mut ctx_state) = agent.context_state {
        // Step 1: L0 cleanup
        let l0 = run_layer0_cleanup(
            ctx_state,
            &agent.config.context_config,
            std::path::Path::new(&agent.config.agent_trail_dir),
            &agent.config.session_id,
        );
        for pr in &l0.persisted {
            ctx_state.session_obs.tool_result_chars_persisted += pr.original_chars;
        }
        // 正文已经不在上下文里了，对应的 read stamp 必须一起作废，否则下一次 read
        // 会拿到「和上次一样，参考上次结果」——而上次结果现在是个占位符。
        for tool_call_id in &l0.evicted_tool_call_ids {
            agent
                .config
                .read_file_state
                .invalidate_tool_call(tool_call_id);
        }
        let persist_tok = estimated_tokens_from_chars(l0.persist_chars_freed);
        let placeholder_tok = estimated_tokens_from_chars(l0.placeholder_chars_freed);
        if persist_tok > 0 || placeholder_tok > 0 {
            ctx_state.session_obs.compaction_tokens_freed += persist_tok + placeholder_tok;
            layer0_release = Some((persist_tok, placeholder_tok));
        }

        // Step 2: restore ExhaustedPending → Running
        ctx_state.preheat.try_restart_if_pending(
            ctx_state.usage_ratio(),
            &ctx_state.messages,
            &ctx_state.transcript_path,
            Arc::clone(&compaction_provider),
            &agent.config.context_config,
            Arc::clone(&compaction_emitter),
            control_snapshot.clone(),
        );

        // Step 3: L2 non-blocking poll + apply boundary
        if ctx_state.usage_ratio() >= 0.85 {
            crate::core::compaction::apply::check_after_reply(ctx_state, &agent.emitter);
        }

        // Step 4: Idle → Running (start new preheat if conditions met)
        let ratio = ctx_state.usage_ratio();
        let turn_count = ctx_state.turn_count();
        if ctx_state.preheat.try_start(
            ratio,
            &ctx_state.messages,
            &ctx_state.transcript_path,
            Arc::clone(&compaction_provider),
            &agent.config.context_config,
            Arc::clone(&compaction_emitter),
            control_snapshot.clone(),
        ) {
            preheat_started = Some((turn_count, ratio));
        }
    }

    if let Some((p, ph)) = layer0_release {
        agent.emit_event(AgentEvent::Layer0ContextRelease {
            persist_tokens_freed: p,
            placeholder_tokens_freed: ph,
        });
    }
    if let Some((covered_count, ratio_before)) = preheat_started {
        agent.emit_event(AgentEvent::AutoCompactionStart {
            covered_count,
            ratio_before,
        });
    }

    agent.emit_context_metrics();
    agent.emit_event(AgentEvent::TurnEnd {
        turn_index,
        message: Message(serde_json::json!({})),
        tool_results: vec![],
        assistant_message_id: Some(assistant_message_id),
        tool_call_ids: vec![],
        summary_title: None,
    });
    Ok(TurnOutcome::Finished)
}
