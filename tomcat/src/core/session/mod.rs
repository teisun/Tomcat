//! 会话管理：元数据 store（sessions.json）与 transcript（pi-mono 相容 JSONL）的 CRUD、上下文组装。

mod append_message_chain;
pub mod attachments;
pub mod context_metrics;
pub(crate) mod manager;
mod model_thinking;
pub(crate) mod resume_index;
pub mod scope;
pub(crate) mod store;
pub(crate) mod subagent_transcript;
pub mod transcript;

pub(crate) use append_message_chain::{
    ResumableAskQuestionResult, assert_active_tool_result_integrity,
    classify_resumable_ask_question_result, collect_recent_chat_messages_from_tail,
    find_dangling_tail_tool_call_ids, find_dangling_tail_tool_calls,
    has_dangling_tool_calls_in_messages, is_tool_call_pending,
};
pub use context_metrics::{ContextLiveMetrics, ContextMetrics};
pub use manager::{
    AgentMode, ApiUsage, CompactionResult, ContextState, INTERRUPTED_TOOL_RESULT_TEXT,
    MessageAppendSink, PENDING_TOOL_RESULT_TEXT, PlanEventKind, PlanEventRef, ResumeControlState,
    SessionManager, UNKNOWN_RESTART_TOOL_RESULT_TEXT, build_context_from_state, compound_turn_id,
    estimate_msg_chars, init_context_state,
};
pub use model_thinking::ModelThinkingStore;
pub use scope::{
    SessionMode, fnv1a_hex, project_root, resolve_session_mode, session_key_for,
    session_key_for_agent,
};
pub use store::{DEFAULT_SESSION_KEY, SessionEntry, SessionStore, load_store, save_store};
pub use transcript::{
    BranchSummaryEntry, ErrorEntry, MessageEntry, MessageSummaryTitleRewrite, MessageTextRewrite,
    SessionHeader, ThinkingTraceEntry, TranscriptEntry, append_entry, append_line,
    insert_entry_after_message_id, mark_message_entries_after_anchor_superseded,
    mark_tool_result_entries_by_tool_call_id_superseded, mark_trailing_user_messages_superseded,
    read_entries_tail, read_header, remove_branch_summary_entry_by_id,
    revive_trailing_failed_user_messages, rewrite_message_summary_titles_by_id,
    rewrite_message_text_entries_by_id, set_branch_summary_entry_is_boundary_true, write_header,
};

#[cfg(test)]
mod tests;
