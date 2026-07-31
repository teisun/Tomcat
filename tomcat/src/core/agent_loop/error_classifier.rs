//! # Agent Loop 错误分类与 L3 上下文溢出回收
//!
//! 本模块承担两类职责（均为 `AgentLoop` 的 **Attempt Loop** 内部逻辑）：
//!
//! 1. [`classify_error`]：把 LLM 返回的 [`AppError`] 映射为
//!    [`LoopError::Retryable`] 或 [`LoopError::Fatal`]，供第二层循环决定
//!    "指数退避重试"或"立即终止"。
//! 2. [`handle_overflow_retry`]：当 Attempt Loop 捕获 `Retryable(context_overflow)`
//!    时，对 `context_state` 做一次 L3 强制截断（`force_drop_oldest_to_target`）并
//!    用 `build_context_from_state` 重建 `messages`，发送 `ContextOverflowTrimStart/End`
//!    事件，更新压缩计数，供下一轮 Attempt 重试。
//!
//! 历史：原 `classify_error` 位于 `convert.rs`；L3 trim 逻辑内联在 `run.rs` 的
//! `run_attempt_loop` 中（约 90 行）。T2-P0-001 将两者聚合到本文件，一方面为 T2-P0-003
//! `ToolLoopGuard` / T2-P0-002 Compaction prompt 预留一个明确的"错误与回收"领域入口，
//! 另一方面让 `run.rs` 的 Attempt Loop 只关心"调度与收尾判定"。

use tracing::info;

use crate::core::compaction::force_drop_oldest_to_target;
use crate::core::llm::{
    degrade_unsupported_multimodal, Capabilities, ChatMessage, ChatMessageRole,
};
use crate::core::session::manager::{build_context_from_state, estimated_tokens_from_chars};
use crate::infra::error::{
    is_context_overflow, is_deterministic_stream_refusal_text, is_retryable_llm_error,
    is_unsupported_multimodal_text, llm_http_status, llm_stage, llm_summary, AppError,
    LlmErrorStage,
};
use crate::infra::events::AgentEvent;

use super::types::{AgentLoop, LoopError, OverflowTrimStats, UnsupportedMultimodalRetryStats};

fn err_snippet(s: &str) -> String {
    s.chars().take(200).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnsupportedMultimodalKind {
    Vision,
    Files,
    Both,
}

fn is_stream_terminal_error(err: &AppError) -> bool {
    matches!(err, AppError::LlmDetailed(_))
        && llm_stage(err).is_none()
        && llm_http_status(err).is_none()
}

fn unsupported_multimodal_kind_from_text(text: &str) -> Option<UnsupportedMultimodalKind> {
    if !is_unsupported_multimodal_text(text) {
        return None;
    }
    let lower = text.to_lowercase();
    let hits_vision = lower.contains("input_image")
        || lower.contains("image input")
        || lower.contains("image_url");
    let hits_files = lower.contains("input_file")
        || lower.contains("file input")
        || lower.contains("file_data")
        || lower.contains(".pdf")
        || lower.contains(" pdf");
    Some(match (hits_vision, hits_files) {
        (true, false) => UnsupportedMultimodalKind::Vision,
        (false, true) => UnsupportedMultimodalKind::Files,
        _ => UnsupportedMultimodalKind::Both,
    })
}

fn temporary_capabilities_for(kind: UnsupportedMultimodalKind) -> Capabilities {
    let mut capabilities = Capabilities {
        vision: true,
        files: true,
        ..Capabilities::default()
    };
    match kind {
        UnsupportedMultimodalKind::Vision => capabilities.vision = false,
        UnsupportedMultimodalKind::Files => capabilities.files = false,
        UnsupportedMultimodalKind::Both => {
            capabilities.vision = false;
            capabilities.files = false;
        }
    }
    capabilities
}

/// 错误分类：把 `AppError` 映射为 `LoopError::Retryable` / `LoopError::Fatal`。
///
/// 分类顺序严格如下（**次序不可交换**，保证 400 + context_length_exceeded 优先走
/// Retryable → 触发 L3 截断路径；否则 400 会被"400 generic"分支吞掉直接 Fatal）：
///
/// 1. `401`                                  → `Fatal`（鉴权失败）
/// 2. `is_context_overflow(err)`             → `Retryable`（走 L3 trim）
/// 3. `400` 且不属于 #2                      → `Fatal`（请求体错误，重试无用）
/// 4. `429 / 500 / 502 / 503 / 504 / 传输阶段` → `Retryable`（限流 / 网关 / 传输）
/// 5. 其它                                    → `Fatal`（默认收紧）
///
/// 每个分支写入 `target="tomcat_chat_diag"` 的诊断 `info!`，branch 字面值：
/// `fatal_401 / retryable_context_overflow / fatal_400_generic / retryable_rate_or_server
/// / fatal_default`（观测面保持稳定）。
pub(super) fn classify_error(err: AppError) -> LoopError {
    let s = err.to_string();
    let snippet = err_snippet(&s);
    let summary = llm_summary(&err).unwrap_or_else(|| s.clone());
    if let Some(stage) = llm_stage(&err) {
        let branch = match stage {
            LlmErrorStage::Connect
            | LlmErrorStage::Send
            | LlmErrorStage::BodyRead
            | LlmErrorStage::IdleTimeout
            | LlmErrorStage::ReadTimeout => "retryable_llm_transport_stage",
            LlmErrorStage::NonStreamStale => "fatal_llm_non_stream_stale_stage",
            LlmErrorStage::Parse => "fatal_llm_parse_stage",
        };
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch,
            stage = %stage,
            snippet = %snippet
        );
        return match stage {
            LlmErrorStage::Connect
            | LlmErrorStage::Send
            | LlmErrorStage::BodyRead
            | LlmErrorStage::IdleTimeout
            | LlmErrorStage::ReadTimeout => LoopError::Retryable(err),
            LlmErrorStage::NonStreamStale | LlmErrorStage::Parse => LoopError::Fatal(err),
        };
    }
    if llm_http_status(&err) == Some(401) {
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch = "fatal_401",
            snippet = %snippet
        );
        return LoopError::Fatal(err);
    }
    // HTTP 400 + context_length_exceeded 等：须为 Retryable，Attempt loop 才能走 L3 截断。
    if is_context_overflow(&err) {
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch = "retryable_context_overflow",
            snippet = %snippet
        );
        return LoopError::Retryable(err);
    }
    // 这两条分支必须在 generic 400 之前：
    //
    // - `content_filter` 之类是“端点明确拒绝”，重试只会重复同样的拒绝；
    // - 其余流内终局错误（无 stage / 无 http_status）则正是本役要补救的缺口，
    //   需要进入 Attempt Loop，统一享受指数退避与后续降级策略。
    if is_stream_terminal_error(&err) && is_deterministic_stream_refusal_text(&summary) {
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch = "fatal_deterministic_stream_refusal",
            snippet = %snippet
        );
        return LoopError::Fatal(err);
    }
    if is_stream_terminal_error(&err) {
        let branch = if is_unsupported_multimodal_text(&summary) {
            "retryable_stream_terminal_unsupported_multimodal"
        } else {
            "retryable_stream_terminal_error"
        };
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch,
            snippet = %snippet
        );
        return LoopError::Retryable(err);
    }
    if llm_http_status(&err) == Some(400) {
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch = "fatal_400_generic",
            snippet = %snippet
        );
        return LoopError::Fatal(err);
    }
    if is_retryable_llm_error(&err) {
        info!(
            target: "tomcat_chat_diag",
            phase = "classify_error",
            branch = "retryable_rate_or_server",
            snippet = %snippet
        );
        return LoopError::Retryable(err);
    }
    info!(
        target: "tomcat_chat_diag",
        phase = "classify_error",
        branch = "fatal_default",
        snippet = %snippet
    );
    LoopError::Fatal(err)
}

/// L3 强制截断 + 消息重建，仅在 `Retryable` 分支内由 Attempt Loop 调用。
///
/// ## 行为约定
///
/// - 先发 `attempt_loop_retryable` 诊断日志（含 `overflow_hit` / `context_state_some` /
///   `snippet`）——**无论是否命中 overflow 都写**，便于观测哪种路径被触发。
/// - 命中 overflow + `context_state` 存在：
///   1. 发 `ContextOverflowTrimStart { ratio: ratio_before }`
///   2. `force_drop_oldest_to_target` 截断 → 累计 `compaction_tokens_freed` / `+1 compaction_count`
///   3. 用 System prompt（若有）+ `build_context_from_state(ctx_state)` + 原 `messages[tail_start..]`
///      重建 `*messages`；同步 `agent.start_idx = tail_start_in_rebuilt`（治本约束，防 T-017 类幽灵）
///   4. 发 `ContextOverflowTrimEnd { ratio_before, ratio_after, will_retry: true, .. }`
///   5. 写诊断 `l3_trim_done`（含 `compaction_count_after`），返回 `applied: true`
/// - 命中 overflow 但 `context_state.is_none()`：
///   写诊断 `l3_skipped_no_context_state`；返回 `applied: false`、**不发**任何事件。
/// - 未命中 overflow：写诊断 `l3_skipped_not_overflow`；返回 `applied: false`、**不发**任何事件。
///
/// ## 所有权边界
///
/// - `err: &str` 仅用于 `attempt_loop_retryable` 诊断日志的 `snippet` 字段（200 字符截断）。
/// - **不**在本函数内更新 `last_err` 或判断 `attempt == max_attempts` —— 那两个决定仍由
///   `run_attempt_loop` 持有，避免 retry 控制流所有权扩散。
/// - 事件通过 `agent.emit_event(...)`（`pub(super)`）发射；时序严格保持
///   `ContextOverflowTrimStart` → （trim/rebuild） → `ContextOverflowTrimEnd` 各一次。
pub(super) fn handle_overflow_retry(
    agent: &mut AgentLoop,
    messages: &mut Vec<ChatMessage>,
    attempt: u32,
    err: &AppError,
) -> OverflowTrimStats {
    let err_text = err.to_string();
    let overflow_hit = is_context_overflow(err);
    let context_state_some = agent.context_state.is_some();
    let err_snip = err_snippet(&err_text);
    info!(
        target: "tomcat_chat_diag",
        phase = "attempt_loop_retryable",
        attempt,
        overflow_hit,
        context_state_some,
        snippet = %err_snip
    );

    if !overflow_hit {
        info!(
            target: "tomcat_chat_diag",
            phase = "l3_skipped_not_overflow",
            attempt
        );
        return OverflowTrimStats::default();
    }

    if !context_state_some {
        info!(
            target: "tomcat_chat_diag",
            phase = "l3_skipped_no_context_state",
            attempt
        );
        return OverflowTrimStats::default();
    }

    // ratio_before 走**只读借用**——与后续 `if let Some(ref mut ctx_state)` 可变借用
    // 分段隔离，保证借用检查器满意（与原 run.rs:323-386 借用结构一致）。
    let ratio_before = agent
        .context_state
        .as_ref()
        .map(|cs| cs.usage_ratio())
        .unwrap_or(0.0);

    agent.emit_event(AgentEvent::ContextOverflowTrimStart {
        reason: "context_overflow".into(),
        ratio: ratio_before,
    });

    let mut trim_tokens = 0usize;
    let mut trim_turns = 0usize;
    if let Some(ref mut ctx_state) = agent.context_state {
        let (turns_removed, chars_removed) = force_drop_oldest_to_target(ctx_state);
        trim_turns = turns_removed;
        trim_tokens = estimated_tokens_from_chars(chars_removed);
        ctx_state.session_obs.compaction_tokens_freed += trim_tokens;
        ctx_state.session_obs.compaction_count =
            ctx_state.session_obs.compaction_count.saturating_add(1);

        let tail_start = agent.context_tail_start.min(messages.len());
        let tail: Vec<ChatMessage> = messages[tail_start..].to_vec();
        let mut rebuilt: Vec<ChatMessage> = Vec::new();
        if messages
            .first()
            .is_some_and(|m| m.role == ChatMessageRole::System)
        {
            rebuilt.push(messages[0].clone());
        }
        rebuilt.extend(build_context_from_state(ctx_state));
        let tail_start_in_rebuilt = rebuilt.len();
        rebuilt.extend(tail);
        *messages = rebuilt;
        agent.start_idx = tail_start_in_rebuilt;
    }

    let ratio_after = agent
        .context_state
        .as_ref()
        .map(|cs| cs.usage_ratio())
        .unwrap_or(0.0);
    agent.emit_event(AgentEvent::ContextOverflowTrimEnd {
        ratio_before,
        ratio_after,
        will_retry: true,
        estimated_tokens_freed: trim_tokens,
        turns_removed: trim_turns,
    });

    let compaction_count_after = agent
        .context_state
        .as_ref()
        .map(|cs| cs.session_obs.compaction_count)
        .unwrap_or(0);
    info!(
        target: "tomcat_chat_diag",
        phase = "l3_trim_done",
        attempt,
        turns_removed = trim_turns,
        trim_tokens,
        ratio_before,
        ratio_after,
        compaction_count_after
    );

    OverflowTrimStats {
        trim_tokens,
        trim_turns,
        ratio_before,
        ratio_after,
        applied: true,
    }
}

pub(super) fn handle_unsupported_multimodal_retry(
    agent: &mut AgentLoop,
    messages: &mut Vec<ChatMessage>,
    attempt: u32,
    err: &AppError,
) -> UnsupportedMultimodalRetryStats {
    let summary = llm_summary(err).unwrap_or_else(|| err.to_string());
    let Some(kind) = unsupported_multimodal_kind_from_text(&summary) else {
        info!(
            target: "tomcat_chat_diag",
            phase = "unsupported_multimodal_retry_skipped_not_matched",
            attempt
        );
        return UnsupportedMultimodalRetryStats::default();
    };

    let degraded = degrade_unsupported_multimodal(messages, &temporary_capabilities_for(kind));
    let std::borrow::Cow::Owned(next_messages) = degraded else {
        info!(
            target: "tomcat_chat_diag",
            phase = "unsupported_multimodal_retry_skipped_no_change",
            attempt
        );
        return UnsupportedMultimodalRetryStats::default();
    };

    *messages = next_messages;
    let stats = UnsupportedMultimodalRetryStats {
        applied: true,
        degraded_vision: matches!(
            kind,
            UnsupportedMultimodalKind::Vision | UnsupportedMultimodalKind::Both
        ),
        degraded_files: matches!(
            kind,
            UnsupportedMultimodalKind::Files | UnsupportedMultimodalKind::Both
        ),
    };
    agent.emit_event(AgentEvent::LlmNotice {
        finish_reason: "unsupported_multimodal_degraded".to_string(),
        message: "本轮附件未被当前端点接受，已按纯文本发送".to_string(),
    });
    info!(
        target: "tomcat_chat_diag",
        phase = "unsupported_multimodal_retry_applied",
        attempt,
        degraded_vision = stats.degraded_vision,
        degraded_files = stats.degraded_files
    );
    stats
}
