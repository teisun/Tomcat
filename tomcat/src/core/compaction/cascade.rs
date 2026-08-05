//! Layer 3: 在服务端确认 Context Overflow 后，强制删除最旧的完整 turn。

use crate::core::session::manager::{estimate_msg_chars, ContextState};
use tracing::info;

/// 服务端确认 context overflow 后，至少删除一个最旧的**完整** turn；随后仅在本地估算仍
/// 高于阈值时继续删除。返回 `(删轮数, 删除字符数之和)`。
///
/// `usage_ratio` 是主动压缩的启发式，但不能推翻服务端刚刚返回的 overflow 事实。最后一个
/// turn 永远保留：删空消息会制造一个更无效的重试请求。
pub fn force_drop_oldest_after_confirmed_overflow(state: &mut ContextState) -> (usize, usize) {
    // 必须先与「当前 messages + estimate_context_chars」对齐：若仍保留上一轮
    // `last_api_usage`，`estimated_token_count()` 会沿用巨大的 `prompt_tokens`，
    // 与正在 drain 的 `messages` 脱节，`usage_ratio()` 长期 ≥ 0.5，
    // 会把 **全部** messages 删空。
    state.invalidate_api_usage();
    let mut turns_removed = 0usize;
    let mut chars_removed = 0usize;
    let mut dropped_message_ids = Vec::new();

    while let Some(turn_end) = state
        .messages
        .iter()
        .enumerate()
        .skip(1) // first message marks the start of the oldest turn
        .find(|(_, m)| m.starts_logical_turn())
        .map(|(i, _)| i)
    {
        // Find the end of the oldest turn: everything from the start up to (but not including)
        // the next turn-start boundary. MessageKind retains the historical distinction
        // between normal input, steering, and completion nudges across restarts.
        let dropped: Vec<_> = state.messages.drain(..turn_end).collect();
        let chars: usize = dropped.iter().map(estimate_msg_chars).sum();
        dropped_message_ids.extend(
            dropped
                .iter()
                .filter_map(|message| message.msg_id.as_deref().map(str::to_string)),
        );
        chars_removed += chars;
        turns_removed += 1;
        state.estimate_context_chars = state.estimate_context_chars.saturating_sub(chars);

        if state.usage_ratio() < 0.50 {
            break;
        }
    }
    if chars_removed > 0 {
        info!(
            target: "tomcat_chat_diag",
            phase = "history_rewritten",
            operation = "force_drop_oldest_after_confirmed_overflow",
            turns_removed,
            chars_freed = chars_removed,
            ?dropped_message_ids,
        );
    }
    (turns_removed, chars_removed)
}
