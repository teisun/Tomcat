use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::{info, Level};

use crate::core::llm::files_api::FilesApiAdapter;
use crate::core::llm::multimodal::degrade_placeholder;
use crate::core::llm::replay_policy::{
    plan, replay_requirement_for_profile, ProviderCompatProfile, ReplayAction,
};
use crate::core::llm::thinking_policy::{resolve_anthropic_request, ThinkingFormat};
use crate::core::llm::types::{
    ChatMessage, ChatMessageContent, ChatMessageContentPart, ChatMessageRole, ChatRequest,
    ChatResponse, ChatResponseChoice, ContinuityMetadata, FileSource, ImageSource,
    ReasoningContinuation, ReasoningFormat, StreamEvent, TokenUsage,
};
use crate::core::llm::Capabilities;
use crate::infra::config::ThinkingConfig;

/// Anthropic rejects requests with more than four explicit cache breakpoints.
///
/// Keep this as a hard provider limit rather than a tuning parameter: callers
/// must safely degrade by dropping lower-priority candidates.
const ANTHROPIC_MAX_CACHE_BREAKPOINTS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
enum CacheBreakpoint {
    LastTool,
    SystemBlock(usize),
    /// End of a complete wire message. This must never be a message that also
    /// contains a volatile ephemeral tail.
    MessageEnd(usize),
    MessageBlock {
        message_idx: usize,
        block_idx: usize,
    },
}

struct RenderedMessages {
    system: Vec<Value>,
    messages: Vec<Value>,
    /// Number of volatile tail blocks appended to each merged wire message.
    /// The data is diagnostic-only; cache eligibility remains decided by
    /// `MessageKind::EphemeralTail` while rendering.
    ephemeral_tail_blocks: Vec<(usize, usize)>,
    /// Ordered from highest to lowest priority, excluding the tool candidate.
    cache_breakpoint_candidates: Vec<CacheBreakpoint>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_request_body(
    request: &ChatRequest,
    model: &str,
    thinking_cfg: &ThinkingConfig,
    thinking_format: ThinkingFormat,
    continuity_enabled: bool,
    stream: bool,
    capabilities: &Capabilities,
    files_adapter: Option<&dyn FilesApiAdapter>,
) -> Value {
    let target = ProviderCompatProfile::anthropic_messages(model);
    let mut rendered = build_messages(
        &request.messages,
        &target,
        continuity_enabled,
        capabilities,
        files_adapter,
    );
    let mut tools = request
        .tools
        .as_ref()
        .map(|tools| convert_tools(tools))
        .filter(|tools| !tools.is_empty());

    // A breakpoint budget is shared across tools, system blocks and messages.
    // Do not add controls at their individual construction sites: compaction
    // recovery may legitimately contain multiple summaries, which would
    // otherwise overflow Anthropic's hard limit.
    let mut cache_breakpoint_candidates = Vec::new();
    if tools.as_ref().is_some_and(|tools| !tools.is_empty()) {
        cache_breakpoint_candidates.push(CacheBreakpoint::LastTool);
    }
    cache_breakpoint_candidates.append(&mut rendered.cache_breakpoint_candidates);
    let selected_cache_breakpoints = apply_cache_breakpoints(
        &mut rendered.system,
        &mut rendered.messages,
        tools.as_deref_mut(),
        cache_breakpoint_candidates.clone(),
    );
    log_prompt_prefix_fingerprint(
        model,
        request,
        &rendered,
        tools.as_deref(),
        &cache_breakpoint_candidates,
        &selected_cache_breakpoints,
    );
    // Model capabilities are resolved per request by `ResolvedCall`, not kept
    // in this connection-scoped provider. Direct provider tests/calls without
    // that resolved value conservatively use the unknown-Anthropic fallback in
    // `resolve_anthropic_request`.
    let (request_max_tokens, model_max_output_tokens) = match request.resolved_output_limit {
        Some(limit) => (Some(limit), Some(limit)),
        None => (request.max_tokens, None),
    };
    let thinking_request = resolve_anthropic_request(
        thinking_cfg,
        thinking_format,
        request_max_tokens,
        model_max_output_tokens,
    );

    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), Value::String(model.to_string()));
    body.insert("messages".to_string(), Value::Array(rendered.messages));
    body.insert(
        "max_tokens".to_string(),
        Value::Number(thinking_request.max_tokens.into()),
    );
    body.insert("stream".to_string(), Value::Bool(stream));
    if !rendered.system.is_empty() {
        body.insert("system".to_string(), Value::Array(rendered.system));
    }
    if let Some(temperature) = request
        .temperature
        .map(|value| value as f64)
        .and_then(serde_json::Number::from_f64)
    {
        body.insert("temperature".to_string(), Value::Number(temperature));
    }
    if let Some(tools) = tools {
        body.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(thinking) = thinking_request.thinking {
        body.insert("thinking".to_string(), thinking);
    }
    if let Some(effort) = thinking_request.effort {
        body.insert(
            "output_config".to_string(),
            serde_json::json!({
                "effort": effort,
            }),
        );
    }
    Value::Object(body)
}

/// Emit hashes, never prompt text, so adjacent requests can be compared without
/// exposing user content in diagnostics. Cache invalidation is a prefix problem:
/// the first differing hash identifies which stable-looking section drifted.
fn log_prompt_prefix_fingerprint(
    model: &str,
    request: &ChatRequest,
    rendered: &RenderedMessages,
    tools: Option<&[Value]>,
    candidates: &[CacheBreakpoint],
    selected: &[CacheBreakpoint],
) {
    if !prompt_prefix_fingerprint_enabled()
        || !tracing::enabled!(target: "tomcat_chat_diag", Level::INFO)
    {
        return;
    }
    let tail_hashes = request
        .messages
        .iter()
        .filter(|message| matches!(message.kind, crate::core::llm::MessageKind::EphemeralTail))
        .map(|message| fingerprint(&serde_json::to_value(message).unwrap_or(Value::Null)))
        .collect::<Vec<_>>();
    let system_hashes = rendered.system.iter().map(fingerprint).collect::<Vec<_>>();
    let last_message_breakpoint = selected
        .iter()
        .filter_map(|breakpoint| match breakpoint {
            CacheBreakpoint::MessageEnd(index)
            | CacheBreakpoint::MessageBlock {
                message_idx: index, ..
            } => Some(*index),
            CacheBreakpoint::LastTool | CacheBreakpoint::SystemBlock(_) => None,
        })
        .max();
    let cacheable_messages = last_message_breakpoint
        .and_then(|index| rendered.messages.get(..=index))
        .unwrap_or_default();
    let message_hashes = cacheable_messages
        .iter()
        .map(fingerprint)
        .collect::<Vec<_>>();
    let message_prefix_hashes = rolling_array_fingerprints(cacheable_messages);
    let user_message_hashes = rendered
        .messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.get("role").and_then(Value::as_str) == Some("user"))
        .map(|(message_idx, message)| {
            let tail_blocks = rendered
                .ephemeral_tail_blocks
                .iter()
                .filter(|(idx, _)| *idx == message_idx)
                .map(|(_, count)| *count)
                .sum();
            let (with_tail, without_tail) = user_message_tail_fingerprints(message, tail_blocks);
            (message_idx, with_tail, without_tail)
        })
        .collect::<Vec<_>>();
    let tool_hash = tools.map(|tools| fingerprint(&Value::Array(tools.to_vec())));
    info!(
        target: "tomcat_chat_diag",
        phase = "prompt_prefix_fingerprint",
        model,
        request_id = ?request.diagnostic_request_id,
        tool_hash = ?tool_hash,
        ?system_hashes,
        ?message_hashes,
        ?message_prefix_hashes,
        ?user_message_hashes,
        ?tail_hashes,
        cache_prefix_message_end = ?last_message_breakpoint,
        cache_breakpoint_candidates = ?candidates,
        cache_breakpoints_selected = ?selected,
    );
}

const PROMPT_PREFIX_FINGERPRINT_ENV: &str = "TOMCAT_PROMPT_PREFIX_FINGERPRINT";

fn prompt_prefix_fingerprint_enabled() -> bool {
    std::env::var(PROMPT_PREFIX_FINGERPRINT_ENV)
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

fn fingerprint(value: &Value) -> String {
    fingerprint_bytes(&serde_json::to_vec(value).unwrap_or_default())
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))[..16].to_string()
}

/// Hash JSON array prefixes incrementally. Snapshotting the hasher after
/// appending `]` produces the exact same digest as serializing that prefix as
/// a standalone `Value::Array`, without cloning and reserializing every
/// earlier message.
fn rolling_array_fingerprints(messages: &[Value]) -> Vec<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"[");
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            if index > 0 {
                hasher.update(b",");
            }
            hasher.update(serde_json::to_vec(message).unwrap_or_default());
            let mut snapshot = hasher.clone();
            snapshot.update(b"]");
            format!("{:x}", snapshot.finalize())[..16].to_string()
        })
        .collect()
}

fn user_message_tail_fingerprints(message: &Value, tail_blocks: usize) -> (String, String) {
    let with_tail = fingerprint(message);
    if tail_blocks == 0 {
        return (with_tail.clone(), with_tail);
    }
    let mut without_tail_message = message.clone();
    if let Some(blocks) = without_tail_message
        .get_mut("content")
        .and_then(Value::as_array_mut)
    {
        blocks.truncate(blocks.len().saturating_sub(tail_blocks));
    }
    (with_tail, fingerprint(&without_tail_message))
}

pub(super) fn response_to_chat_response(
    raw: &Value,
    source_profile: &ProviderCompatProfile,
    continuity_enabled: bool,
) -> ChatResponse {
    let parsed = parse_assistant_content(
        raw.get("content").and_then(Value::as_array),
        source_profile,
        continuity_enabled,
    );
    let mut message = if parsed.tool_calls.is_empty() {
        ChatMessage::assistant(parsed.text.as_str())
    } else {
        let content = (!parsed.text.is_empty()).then_some(parsed.text.as_str());
        ChatMessage::assistant_with_tool_calls(content, parsed.tool_calls)
    };
    message.thinking_text = parsed.thinking_text.clone();
    message.reasoning_continuation = parsed.reasoning_continuation.clone();
    message.continuity = parsed.continuity.clone();
    ChatResponse {
        id: raw.get("id").and_then(Value::as_str).map(str::to_string),
        choices: vec![ChatResponseChoice {
            index: 0,
            message,
            finish_reason: raw
                .get("stop_reason")
                .and_then(Value::as_str)
                .map(normalize_finish_reason),
        }],
        usage: usage_from_value(raw.get("usage")),
    }
}

pub(super) fn final_stream_events(
    source_profile: &ProviderCompatProfile,
    continuity_enabled: bool,
    thinking_blocks: Vec<Value>,
    thinking_text: Option<String>,
    had_tool_call: bool,
    usage: Option<TokenUsage>,
    stop_reason: Option<String>,
) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    let reasoning_continuation = if continuity_enabled && !thinking_blocks.is_empty() {
        Some(ReasoningContinuation {
            source_provider: source_profile.provider.clone(),
            source_api: source_profile.api_family.clone(),
            source_model: source_profile.model_family.clone(),
            format: ReasoningFormat::AnthropicThinkingBlocks,
            opaque_payload: Value::Array(thinking_blocks),
            fallback_text: thinking_text.clone(),
            provider_refs: None,
        })
    } else {
        None
    };
    if thinking_text.is_some() || reasoning_continuation.is_some() {
        events.push(StreamEvent::ReasoningSnapshot {
            thinking_text,
            reasoning_continuation: reasoning_continuation.clone(),
            continuity: reasoning_continuation.as_ref().map(|_| ContinuityMetadata {
                had_tool_call,
                replay_requirement: replay_requirement_for_profile(source_profile, had_tool_call),
            }),
        });
    }
    if let Some(usage) = usage {
        events.push(StreamEvent::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            text_tokens: usage.text_tokens,
        });
    }
    if let Some(stop_reason) = stop_reason {
        let finish_reason = normalize_finish_reason(&stop_reason);
        // Anthropic 的 `max_tokens` 是正常 stream terminal，不会像 Responses 那样
        // 自动带用户可见 notice。没有这条，界面只能看到一个看似正常的 stop，
        // 很难区分“模型完成”与“输出预算耗尽”。
        if stop_reason == "max_tokens" {
            events.push(StreamEvent::LlmNotice {
                finish_reason: finish_reason.clone(),
                message: "模型达到本轮最大输出长度，回复可能不完整。".to_string(),
            });
        }
        events.push(StreamEvent::FinishReason {
            reason: finish_reason,
        });
    }
    events
}

pub(super) fn parse_assistant_content(
    content: Option<&Vec<Value>>,
    source_profile: &ProviderCompatProfile,
    continuity_enabled: bool,
) -> ParsedAssistantContent {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut thinking_blocks = Vec::new();
    let mut thinking_chunks = Vec::new();

    if let Some(content) = content {
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(chunk) = block.get("text").and_then(Value::as_str) {
                        text.push_str(chunk);
                    }
                }
                Some("thinking") => {
                    let thinking = block
                        .get("thinking")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if !thinking.trim().is_empty() {
                        thinking_chunks.push(thinking.clone());
                    }
                    thinking_blocks.push(json!({
                        "type": "thinking",
                        "thinking": thinking,
                        "signature": block.get("signature").cloned().unwrap_or(Value::Null),
                    }));
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
                    tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string()),
                        }
                    }));
                }
                _ => {}
            }
        }
    }

    let thinking_text = if thinking_chunks.is_empty() {
        None
    } else {
        Some(thinking_chunks.join("\n\n"))
    };
    let had_tool_call = !tool_calls.is_empty();
    let reasoning_continuation = if continuity_enabled && !thinking_blocks.is_empty() {
        Some(ReasoningContinuation {
            source_provider: source_profile.provider.clone(),
            source_api: source_profile.api_family.clone(),
            source_model: source_profile.model_family.clone(),
            format: ReasoningFormat::AnthropicThinkingBlocks,
            opaque_payload: Value::Array(thinking_blocks),
            fallback_text: thinking_text.clone(),
            provider_refs: None,
        })
    } else {
        None
    };
    let continuity = reasoning_continuation.as_ref().map(|_| ContinuityMetadata {
        had_tool_call,
        replay_requirement: replay_requirement_for_profile(source_profile, had_tool_call),
    });
    ParsedAssistantContent {
        text,
        tool_calls,
        thinking_text,
        reasoning_continuation,
        continuity,
    }
}

pub(super) struct ParsedAssistantContent {
    pub text: String,
    pub tool_calls: Vec<Value>,
    pub thinking_text: Option<String>,
    pub reasoning_continuation: Option<ReasoningContinuation>,
    pub continuity: Option<ContinuityMetadata>,
}

fn build_messages(
    messages: &[ChatMessage],
    target: &ProviderCompatProfile,
    continuity_enabled: bool,
    capabilities: &Capabilities,
    files_adapter: Option<&dyn FilesApiAdapter>,
) -> RenderedMessages {
    let mut system_chunks = Vec::new();
    let mut out: Vec<Value> = Vec::new();
    let mut last_non_ephemeral_message = None;
    let mut last_complete_message_before_ephemeral = None;
    let mut last_compaction_summary_block = None;
    let mut ephemeral_message_indices = Vec::new();
    let mut ephemeral_tail_blocks = Vec::new();
    for original in messages {
        let is_ephemeral = matches!(original.kind, crate::core::llm::MessageKind::EphemeralTail);
        if is_ephemeral && last_complete_message_before_ephemeral.is_none() {
            // Consecutive user messages are merged for Anthropic/Bedrock
            // compatibility. When this tail joins a preceding user/tool-result
            // message, cache only the *previous* complete message.
            last_complete_message_before_ephemeral = match out.last() {
                Some(last) if last.get("role").and_then(Value::as_str) == Some("user") => {
                    out.len().checked_sub(2)
                }
                Some(_) => out.len().checked_sub(1),
                None => None,
            };
        }
        let action = if continuity_enabled {
            plan(target, original)
        } else {
            ReplayAction::StripOpaque
        };
        let keep_opaque = matches!(action, ReplayAction::KeepOpaque);
        let msg = match action {
            ReplayAction::KeepOpaque | ReplayAction::StripOpaque => {
                original.without_completion_metadata()
            }
        };
        match msg.role {
            ChatMessageRole::System => {
                let text = flatten_message_text(&msg);
                if !text.trim().is_empty() {
                    system_chunks.push(json!({
                        "type": "text",
                        "text": text,
                    }));
                }
            }
            ChatMessageRole::User => {
                let content = user_content_blocks(&msg, capabilities, files_adapter);
                if !content.is_empty() {
                    let ephemeral_block_count = is_ephemeral.then_some(content.len());
                    if let Some(out_idx) = push_role_message(&mut out, "user", content) {
                        if is_ephemeral {
                            ephemeral_message_indices.push(out_idx);
                            if let Some(block_count) = ephemeral_block_count {
                                ephemeral_tail_blocks.push((out_idx, block_count));
                            }
                        }
                        if !is_ephemeral {
                            last_non_ephemeral_message = Some(out_idx);
                        }
                        if matches!(msg.kind, crate::core::llm::MessageKind::CompactionSummary) {
                            let last_block =
                                last_content_block_index(&out, out_idx).map(|idx| (out_idx, idx));
                            last_compaction_summary_block = last_block;
                        }
                    }
                }
            }
            ChatMessageRole::Assistant => {
                let mut content = Vec::new();
                if continuity_enabled && keep_opaque {
                    if let Some(blocks) = original
                        .reasoning_continuation
                        .as_ref()
                        .and_then(continuation_blocks)
                    {
                        content.extend(blocks);
                    }
                }
                let text = flatten_message_text(&msg);
                if !text.trim().is_empty() {
                    content.push(json!({
                        "type": "text",
                        "text": text,
                    }));
                }
                if let Some(tool_calls) = msg.tool_calls.as_ref() {
                    for tool_call in tool_calls {
                        let id = tool_call
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let function = tool_call.get("function").cloned().unwrap_or(Value::Null);
                        let name = function
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let arguments = function
                            .get("arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}");
                        content.push(json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": parse_json_string(arguments),
                        }));
                    }
                }
                if !content.is_empty() {
                    if let Some(out_idx) = push_role_message(&mut out, "assistant", content) {
                        if is_ephemeral {
                            ephemeral_message_indices.push(out_idx);
                        }
                        if !is_ephemeral {
                            last_non_ephemeral_message = Some(out_idx);
                        }
                    }
                }
            }
            ChatMessageRole::Tool => {
                let tool_use_id = msg.tool_call_id.clone().unwrap_or_default();
                let text = flatten_message_text(&msg);
                if let Some(out_idx) = push_role_message(
                    &mut out,
                    "user",
                    vec![json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": text,
                    })],
                ) {
                    if is_ephemeral {
                        ephemeral_message_indices.push(out_idx);
                    }
                    if !is_ephemeral {
                        last_non_ephemeral_message = Some(out_idx);
                    }
                }
            }
        }
    }

    let mut cache_breakpoint_candidates = Vec::new();
    if let Some(last_system_idx) = system_chunks.len().checked_sub(1) {
        cache_breakpoint_candidates.push(CacheBreakpoint::SystemBlock(last_system_idx));
    }
    let last_cacheable_message = if ephemeral_message_indices.is_empty() {
        last_non_ephemeral_message
    } else {
        last_complete_message_before_ephemeral
    };
    if let Some(message_idx) = last_cacheable_message {
        cache_breakpoint_candidates.push(CacheBreakpoint::MessageEnd(message_idx));
    }
    if let Some((message_idx, block_idx)) = last_compaction_summary_block
        .filter(|(message_idx, _)| !ephemeral_message_indices.contains(message_idx))
    {
        cache_breakpoint_candidates.push(CacheBreakpoint::MessageBlock {
            message_idx,
            block_idx,
        });
    }
    RenderedMessages {
        system: system_chunks,
        messages: out,
        ephemeral_tail_blocks,
        cache_breakpoint_candidates,
    }
}

fn continuation_blocks(continuation: &ReasoningContinuation) -> Option<Vec<Value>> {
    match continuation.format {
        ReasoningFormat::AnthropicThinkingBlocks => continuation
            .opaque_payload
            .as_array()
            .cloned()
            .map(|items| {
                items
                    .into_iter()
                    .filter(|item| {
                        item.get("type")
                            .and_then(Value::as_str)
                            .map(|ty| ty == "thinking")
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>()
            }),
        _ => None,
    }
}

fn convert_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let function = tool.get("function")?;
            let name = function.get("name")?.as_str()?;
            let description = function
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string);
            let input_schema = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            let mut out = serde_json::Map::new();
            out.insert("name".to_string(), Value::String(name.to_string()));
            if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
                out.insert("description".to_string(), Value::String(description));
            }
            out.insert("input_schema".to_string(), input_schema);
            Some(Value::Object(out))
        })
        .collect()
}

fn usage_from_value(usage: Option<&Value>) -> Option<TokenUsage> {
    let usage = usage?;
    let prompt = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let completion = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .map(|value| value as u32);
    let cache_write_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .map(|value| value as u32);
    if prompt == 0 && completion == 0 && cache_read_tokens.is_none() && cache_write_tokens.is_none()
    {
        None
    } else {
        Some(TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            cache_read_tokens,
            cache_write_tokens,
            total_tokens: Some(prompt + completion),
            reasoning_tokens: None,
            text_tokens: None,
        })
    }
}

fn push_role_message(out: &mut Vec<Value>, role: &str, content: Vec<Value>) -> Option<usize> {
    if content.is_empty() {
        return None;
    }
    if let Some(last) = out.last_mut() {
        let same_role = last
            .get("role")
            .and_then(Value::as_str)
            .map(|existing| existing == role)
            .unwrap_or(false);
        if same_role {
            if let Some(existing) = last.get_mut("content").and_then(Value::as_array_mut) {
                existing.extend(content);
                return Some(out.len() - 1);
            }
        }
    }
    out.push(json!({
        "role": role,
        "content": content,
    }));
    Some(out.len() - 1)
}

fn add_cache_control(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
    }
}

fn apply_cache_breakpoints(
    system: &mut [Value],
    messages: &mut [Value],
    mut tools: Option<&mut [Value]>,
    candidates: Vec<CacheBreakpoint>,
) -> Vec<CacheBreakpoint> {
    let mut selected = Vec::with_capacity(ANTHROPIC_MAX_CACHE_BREAKPOINTS);
    for candidate in candidates {
        if selected.len() == ANTHROPIC_MAX_CACHE_BREAKPOINTS {
            break;
        }
        if selected.contains(&candidate) {
            continue;
        }
        let applied = match candidate {
            CacheBreakpoint::LastTool => tools
                .as_deref_mut()
                .and_then(|tools| tools.last_mut())
                .map(add_cache_control)
                .is_some(),
            CacheBreakpoint::SystemBlock(index) => {
                system.get_mut(index).map(add_cache_control).is_some()
            }
            CacheBreakpoint::MessageEnd(message_idx) => messages
                .get_mut(message_idx)
                .and_then(|message| message.get_mut("content"))
                .and_then(Value::as_array_mut)
                .and_then(|blocks| blocks.last_mut())
                .map(add_cache_control)
                .is_some(),
            CacheBreakpoint::MessageBlock {
                message_idx,
                block_idx,
            } => messages
                .get_mut(message_idx)
                .and_then(|message| message.get_mut("content"))
                .and_then(Value::as_array_mut)
                .and_then(|blocks| blocks.get_mut(block_idx))
                .map(add_cache_control)
                .is_some(),
        };
        if applied {
            selected.push(candidate);
        }
    }
    selected
}

fn last_content_block_index(messages: &[Value], message_idx: usize) -> Option<usize> {
    messages
        .get(message_idx)
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.len().checked_sub(1))
}

fn user_content_blocks(
    message: &ChatMessage,
    capabilities: &Capabilities,
    files_adapter: Option<&dyn FilesApiAdapter>,
) -> Vec<Value> {
    match &message.content {
        Some(ChatMessageContent::Text(text)) => vec![json!({
            "type": "text",
            "text": text,
        })],
        Some(ChatMessageContent::Parts(parts)) => parts
            .iter()
            .map(|part| content_part_to_block(part, capabilities, files_adapter))
            .collect::<Vec<_>>(),
        None => vec![json!({
            "type": "text",
            "text": "",
        })],
    }
}

fn content_part_to_block(
    part: &ChatMessageContentPart,
    capabilities: &Capabilities,
    files_adapter: Option<&dyn FilesApiAdapter>,
) -> Value {
    match part {
        ChatMessageContentPart::InputText { text } => json!({
            "type": "text",
            "text": text,
        }),
        ChatMessageContentPart::InputReference { reference } => json!({
            "type": "text",
            "text": reference.to_prompt_text(),
        }),
        ChatMessageContentPart::InputImage { source, .. } => {
            if !capabilities.vision {
                return json!({
                    "type": "text",
                    "text": degrade_placeholder(part),
                });
            }
            match source {
                ImageSource::Inline(source) => json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": source.mime_type,
                        "data": source.data,
                    }
                }),
                ImageSource::Uploaded(source) => {
                    if !capabilities.files {
                        return json!({
                            "type": "text",
                            "text": degrade_placeholder(part),
                        });
                    }
                    let Some(adapter) = files_adapter else {
                        return json!({
                            "type": "text",
                            "text": degrade_placeholder(part),
                        });
                    };
                    json!({
                        "type": "image",
                        "source": {
                            "type": "file",
                            "file_id": adapter.reference_token(&source.file_id),
                        }
                    })
                }
            }
        }
        ChatMessageContentPart::InputFile { source } => {
            if !capabilities.files {
                return json!({
                    "type": "text",
                    "text": degrade_placeholder(part),
                });
            }
            match source {
                FileSource::Inline(source) => {
                    let mut block = json!({
                        "type": "document",
                        "source": {
                            "type": "base64",
                            "media_type": source.mime_type,
                            "data": source.data,
                        }
                    });
                    if !source.filename.trim().is_empty() {
                        block["title"] = Value::String(source.filename.clone());
                    }
                    block
                }
                FileSource::Uploaded(source) => {
                    let Some(adapter) = files_adapter else {
                        return json!({
                            "type": "text",
                            "text": degrade_placeholder(part),
                        });
                    };
                    let mut block = json!({
                        "type": "document",
                        "source": {
                            "type": "file",
                            "file_id": adapter.reference_token(&source.file_id),
                        }
                    });
                    if let Some(filename) = source
                        .filename
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                    {
                        block["title"] = Value::String(filename.to_string());
                    }
                    block
                }
            }
        }
    }
}

fn flatten_message_text(message: &ChatMessage) -> String {
    match &message.content {
        Some(ChatMessageContent::Text(text)) => text.clone(),
        Some(ChatMessageContent::Parts(parts)) => {
            let mut text = String::new();
            for part in parts {
                match part {
                    ChatMessageContentPart::InputText { text: chunk } => text.push_str(chunk),
                    ChatMessageContentPart::InputReference { reference } => {
                        text.push_str(&reference.to_prompt_text());
                    }
                    ChatMessageContentPart::InputImage { .. }
                    | ChatMessageContentPart::InputFile { .. } => {
                        text.push_str(&degrade_placeholder(part));
                    }
                }
            }
            text
        }
        None => String::new(),
    }
}

fn parse_json_string(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| json!({ "_raw": raw }))
}

fn normalize_finish_reason(reason: &str) -> String {
    match reason {
        "end_turn" => "stop".to_string(),
        "tool_use" => "tool_calls".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::{json, Value};

    use super::{
        build_request_body, final_stream_events, fingerprint, response_to_chat_response,
        rolling_array_fingerprints, user_message_tail_fingerprints,
    };
    use crate::core::llm::files_api::FilesApiAdapter;
    use crate::core::llm::openai_files::{FilePurpose, OpenAiFileMeta};
    use crate::core::llm::replay_policy::ProviderCompatProfile;
    use crate::core::llm::thinking_policy::ThinkingFormat;
    use crate::core::llm::types::{
        ChatMessage, ChatMessageContentPart, ChatRequest, ReasoningFormat, StreamEvent,
    };
    use crate::core::llm::Capabilities;
    use crate::infra::config::ThinkingConfig;
    use crate::infra::error::AppError;

    #[derive(Debug)]
    struct StaticFilesAdapter {
        prefix: &'static str,
    }

    #[async_trait]
    impl FilesApiAdapter for StaticFilesAdapter {
        async fn upload(
            &self,
            _purpose: FilePurpose,
            _filename: &str,
            _mime_type: &str,
            _bytes: &[u8],
        ) -> Result<OpenAiFileMeta, AppError> {
            unreachable!("upload should not be called in wire tests");
        }

        async fn delete(&self, _file_id: &str) -> Result<(), AppError> {
            unreachable!("delete should not be called in wire tests");
        }

        fn expires_after_seconds(&self) -> u64 {
            3600
        }

        fn reference_token(&self, file_id: &str) -> String {
            format!("{}{}", self.prefix, file_id)
        }
    }

    fn count_cache_controls(value: &Value) -> usize {
        match value {
            Value::Array(items) => items.iter().map(count_cache_controls).sum(),
            Value::Object(entries) => {
                usize::from(entries.contains_key("cache_control"))
                    + entries.values().map(count_cache_controls).sum::<usize>()
            }
            _ => 0,
        }
    }

    #[test]
    fn rolling_prefix_hashes_match_the_legacy_serialized_array_hashes() {
        let messages = vec![
            json!({"role": "user", "content": [{"type": "text", "text": "one"}]}),
            json!({"role": "assistant", "content": [{"type": "text", "text": "two"}]}),
            json!({"role": "user", "content": [{"type": "text", "text": "three"}]}),
        ];
        let legacy = (1..=messages.len())
            .map(|end| fingerprint(&Value::Array(messages[..end].to_vec())))
            .collect::<Vec<_>>();

        assert_eq!(rolling_array_fingerprints(&messages), legacy);
    }

    #[test]
    fn user_tail_hash_pair_distinguishes_the_volatile_tail() {
        let message = json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "persisted input"},
                {"type": "text", "text": "<system_reminder>volatile</system_reminder>"}
            ]
        });
        let (with_tail, without_tail) = user_message_tail_fingerprints(&message, 1);
        assert_ne!(with_tail, without_tail);
        assert_eq!(
            without_tail,
            fingerprint(&json!({
                "role": "user",
                "content": [{"type": "text", "text": "persisted input"}]
            }))
        );
    }

    #[test]
    fn build_request_body_extracts_system_and_user_messages() {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("be helpful"),
                ChatMessage::user("hello"),
            ],
            model: "ignored".to_string(),
            temperature: Some(0.2),
            max_tokens: Some(4096),
            resolved_output_limit: None,
            diagnostic_request_id: None,
            stream: Some(true),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            tools: None,
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(body["model"], "claude-opus-4-6");
        assert_eq!(body["system"][0]["type"], "text");
        assert_eq!(body["system"][0]["text"], "be helpful");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["text"], "hello");
        assert_eq!(body["stream"], true);
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "high");
        let temperature = body["temperature"]
            .as_f64()
            .expect("temperature serialized as number");
        assert!((temperature - 0.2).abs() < 1e-6);
    }

    #[test]
    fn model_output_limit_drives_anthropic_wire_max_tokens() {
        let request = ChatRequest {
            messages: vec![ChatMessage::user("hello")],
            model: "ignored".to_string(),
            temperature: None,
            max_tokens: None,
            resolved_output_limit: Some(128_000),
            diagnostic_request_id: None,
            stream: Some(true),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            tools: None,
        };
        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );
        assert_eq!(body["max_tokens"], 128_000);

        let capped = build_request_body(
            &ChatRequest {
                max_tokens: Some(256_000),
                resolved_output_limit: Some(128_000),
                diagnostic_request_id: None,
                ..request
            },
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );
        assert_eq!(capped["max_tokens"], 128_000);
    }

    #[test]
    fn anthropic_wire_uses_a_legal_conservative_cap_without_resolved_limits() {
        let request = ChatRequest {
            messages: vec![ChatMessage::user("hello")],
            model: "ignored".to_string(),
            ..Default::default()
        };
        let body = build_request_body(
            &request,
            "unknown-anthropic-model",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );
        assert_eq!(body["max_tokens"], 32_000);
    }

    #[test]
    fn last_breakpoint_precedes_the_ephemeral_tail() {
        let mut summary = ChatMessage::user("compacted history");
        summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let mut tail = ChatMessage::user("<system_reminder>runtime state</system_reminder>");
        tail.kind = crate::core::llm::MessageKind::EphemeralTail;
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("stable system"),
                summary,
                ChatMessage::user("latest persisted input"),
                tail,
            ],
            model: "ignored".to_string(),
            tools: Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "read",
                    "description": "read a file",
                    "parameters": {"type": "object"},
                }
            })]),
            ..Default::default()
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["tools"][0]["cache_control"]["type"], "ephemeral");
        let content = body["messages"][0]["content"]
            .as_array()
            .expect("merged user content");
        assert!(
            content.iter().all(|block| block.get("cache_control").is_none()),
            "a message which contains the volatile tail must not receive any message-level cache breakpoint"
        );
    }

    #[test]
    fn cache_breakpoint_uses_the_prior_complete_message_when_tail_merges_with_tool_results() {
        let mut assistant = ChatMessage::assistant("I will inspect it");
        assistant.tool_calls = Some(vec![json!({
            "id": "call-1",
            "function": {"name": "read", "arguments": "{\"path\":\"src/lib.rs\"}"}
        })]);
        let mut tail = ChatMessage::user("<system_reminder>runtime state</system_reminder>");
        tail.kind = crate::core::llm::MessageKind::EphemeralTail;
        let request = ChatRequest {
            messages: vec![
                ChatMessage::user("inspect the file"),
                assistant,
                ChatMessage::tool("call-1", "file contents"),
                tail,
            ],
            model: "ignored".to_string(),
            ..Default::default()
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(
            body["messages"][1]["content"][1]["cache_control"]["type"], "ephemeral",
            "the complete assistant tool-use message is the cache boundary"
        );
        assert!(
            body["messages"][2]["content"]
                .as_array()
                .expect("tool result and tail merge as a user turn")
                .iter()
                .all(|block| block.get("cache_control").is_none()),
            "the volatile tail's merged user message must remain outside the cached prefix"
        );
    }

    #[test]
    fn rendered_anthropic_messages_never_repeat_adjacent_roles() {
        let mut assistant = ChatMessage::assistant("calling a tool");
        assistant.tool_calls = Some(vec![json!({
            "id": "call-1",
            "function": {"name": "read", "arguments": "{\"path\":\"src/lib.rs\"}"}
        })]);
        let mut tail = ChatMessage::user("<system_reminder>volatile state</system_reminder>");
        tail.kind = crate::core::llm::MessageKind::EphemeralTail;
        let request = ChatRequest {
            messages: vec![
                ChatMessage::user("first user part"),
                ChatMessage::user("second user part"),
                ChatMessage::assistant("first assistant part"),
                assistant,
                ChatMessage::tool("call-1", "file contents"),
                tail,
                ChatMessage::assistant("final answer"),
                ChatMessage::assistant("additional answer text"),
            ],
            model: "ignored".to_string(),
            ..Default::default()
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );
        let roles = body["messages"]
            .as_array()
            .expect("wire messages")
            .iter()
            .map(|message| message["role"].as_str().expect("role"))
            .collect::<Vec<_>>();
        assert!(
            roles.windows(2).all(|pair| pair[0] != pair[1]),
            "Anthropic's strict alternation must survive all merge paths: {roles:?}"
        );
    }

    #[test]
    fn cache_breakpoints_never_exceed_the_provider_budget_with_many_summaries() {
        let mut first_summary = ChatMessage::user("oldest compacted history");
        first_summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let mut second_summary = ChatMessage::user("middle compacted history");
        second_summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let mut latest_summary = ChatMessage::user("latest compacted history");
        latest_summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let mut tail = ChatMessage::user("<system_reminder>runtime state</system_reminder>");
        tail.kind = crate::core::llm::MessageKind::EphemeralTail;
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("stable system"),
                first_summary,
                second_summary,
                latest_summary,
                ChatMessage::user("latest persisted input"),
                tail,
            ],
            model: "ignored".to_string(),
            tools: Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "read",
                    "description": "read a file",
                    "parameters": {"type": "object"},
                }
            })]),
            ..Default::default()
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(count_cache_controls(&body), 2);
        let content = body["messages"][0]["content"]
            .as_array()
            .expect("contiguous user messages merge into blocks");
        assert!(
            content[0].get("cache_control").is_none(),
            "the oldest summary is lower priority than the four provider-supported breakpoints"
        );
        assert!(
            content[1].get("cache_control").is_none(),
            "only the latest summary may receive the summary breakpoint"
        );
        assert!(
            content
                .iter()
                .all(|block| block.get("cache_control").is_none()),
            "a merged tail makes the entire user message ineligible as a cache breakpoint"
        );
    }

    #[test]
    fn cache_breakpoints_handle_empty_system_and_tools() {
        let mut summary = ChatMessage::user("compacted history");
        summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let mut tail = ChatMessage::user("<system_reminder>runtime state</system_reminder>");
        tail.kind = crate::core::llm::MessageKind::EphemeralTail;
        let request = ChatRequest {
            messages: vec![summary, tail],
            model: "ignored".to_string(),
            ..Default::default()
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(count_cache_controls(&body), 0);
        let content = body["messages"][0]["content"]
            .as_array()
            .expect("contiguous user messages merge into blocks");
        assert!(content
            .iter()
            .all(|block| block.get("cache_control").is_none()));
    }

    #[test]
    fn cache_breakpoints_allow_an_empty_history() {
        let body = build_request_body(
            &ChatRequest::default(),
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(count_cache_controls(&body), 0);
        assert_eq!(body["messages"], json!([]));
        assert!(body.get("system").is_none());
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn cache_breakpoints_never_mark_an_ephemeral_only_history() {
        let mut tail = ChatMessage::user("<system_reminder>runtime-only state</system_reminder>");
        tail.kind = crate::core::llm::MessageKind::EphemeralTail;
        let request = ChatRequest {
            messages: vec![tail],
            ..Default::default()
        };
        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            true,
            &Capabilities::default(),
            None,
        );

        assert_eq!(count_cache_controls(&body), 0);
        assert!(body["messages"][0]["content"][0]
            .get("cache_control")
            .is_none());
    }

    #[test]
    fn build_request_body_serializes_inline_pdf_for_anthropic() {
        let request = ChatRequest {
            messages: vec![ChatMessage::user_with_parts(vec![
                ChatMessageContentPart::file_base64_data("notes.pdf", "application/pdf", "cGRm")
                    .expect("valid inline pdf"),
            ])],
            model: "ignored".to_string(),
            temperature: None,
            max_tokens: None,
            resolved_output_limit: None,
            diagnostic_request_id: None,
            stream: Some(false),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            tools: None,
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            false,
            &Capabilities {
                vision: true,
                files: true,
                ..Capabilities::default()
            },
            None,
        );

        assert_eq!(body["messages"][0]["content"][0]["type"], "document");
        assert_eq!(
            body["messages"][0]["content"][0]["source"]["type"],
            "base64"
        );
        assert_eq!(
            body["messages"][0]["content"][0]["source"]["media_type"],
            "application/pdf"
        );
        assert_eq!(body["messages"][0]["content"][0]["title"], "notes.pdf");
    }

    #[test]
    fn build_request_body_serializes_uploaded_image_and_pdf_file_ids() {
        let adapter = StaticFilesAdapter { prefix: "anth-" };
        let request = ChatRequest {
            messages: vec![ChatMessage::user_with_parts(vec![
                ChatMessageContentPart::image_file_id("file-img").expect("valid image id"),
                ChatMessageContentPart::file_file_id("file-doc", Some("report.pdf".to_string()))
                    .expect("valid file id"),
            ])],
            model: "ignored".to_string(),
            temperature: None,
            max_tokens: None,
            resolved_output_limit: None,
            diagnostic_request_id: None,
            stream: Some(false),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            tools: None,
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            false,
            &Capabilities {
                vision: true,
                files: true,
                ..Capabilities::default()
            },
            Some(&adapter),
        );

        assert_eq!(body["messages"][0]["content"][0]["type"], "image");
        assert_eq!(body["messages"][0]["content"][0]["source"]["type"], "file");
        assert_eq!(
            body["messages"][0]["content"][0]["source"]["file_id"],
            "anth-file-img"
        );
        assert_eq!(body["messages"][0]["content"][1]["type"], "document");
        assert_eq!(body["messages"][0]["content"][1]["source"]["type"], "file");
        assert_eq!(
            body["messages"][0]["content"][1]["source"]["file_id"],
            "anth-file-doc"
        );
        assert_eq!(body["messages"][0]["content"][1]["title"], "report.pdf");
    }

    #[test]
    fn build_request_body_degrades_when_anthropic_files_or_vision_disabled() {
        let request = ChatRequest {
            messages: vec![ChatMessage::user_with_parts(vec![
                ChatMessageContentPart::image_file_id("file-img").expect("valid image id"),
                ChatMessageContentPart::file_file_id("file-doc", Some("report.pdf".to_string()))
                    .expect("valid file id"),
            ])],
            model: "ignored".to_string(),
            temperature: None,
            max_tokens: None,
            resolved_output_limit: None,
            diagnostic_request_id: None,
            stream: Some(false),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            tools: None,
        };

        let body = build_request_body(
            &request,
            "claude-opus-4-6",
            &ThinkingConfig::default(),
            ThinkingFormat::AnthropicAdaptive,
            true,
            false,
            &Capabilities {
                vision: false,
                files: false,
                ..Capabilities::default()
            },
            None,
        );

        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
        assert_eq!(
            body["messages"][0]["content"][0]["text"],
            "[图片已省略：当前模型不支持图片输入]"
        );
        assert_eq!(body["messages"][0]["content"][1]["type"], "text");
        assert_eq!(
            body["messages"][0]["content"][1]["text"],
            "[文件已省略：当前模型不支持文件输入]"
        );
    }

    #[test]
    fn final_stream_events_notices_anthropic_max_tokens() {
        let events = final_stream_events(
            &ProviderCompatProfile::anthropic_messages("claude-opus-4-6"),
            true,
            vec![],
            None,
            false,
            None,
            Some("max_tokens".to_string()),
        );
        assert!(events.iter().any(|event| {
            matches!(
                event,
                StreamEvent::LlmNotice {
                    finish_reason,
                    message,
                } if finish_reason == "max_tokens" && message.contains("最大输出长度")
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                StreamEvent::FinishReason { reason } if reason == "max_tokens"
            )
        }));
    }

    #[test]
    fn response_to_chat_response_preserves_thinking_and_tool_calls() {
        let profile = ProviderCompatProfile::anthropic_messages("claude-opus-4-6");
        let raw = json!({
            "id": "msg_1",
            "content": [
                {
                    "type": "thinking",
                    "thinking": "reason step",
                    "signature": "sig_1",
                },
                {
                    "type": "text",
                    "text": "Need one tool call.",
                },
                {
                    "type": "tool_use",
                    "id": "tool_1",
                    "name": "read_file",
                    "input": {
                        "path": "README.md"
                    }
                }
            ],
            "stop_reason": "tool_use",
            "usage": {
                "input_tokens": 11,
                "output_tokens": 22
            }
        });

        let response = response_to_chat_response(&raw, &profile, true);
        let choice = &response.choices[0];
        let message = &choice.message;

        assert_eq!(choice.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(message.thinking_text.as_deref(), Some("reason step"));
        assert_eq!(
            message
                .tool_calls
                .as_ref()
                .and_then(|calls| calls.first())
                .and_then(|call| call["function"]["name"].as_str()),
            Some("read_file")
        );
        assert_eq!(
            message
                .reasoning_continuation
                .as_ref()
                .map(|continuation| continuation.format.clone()),
            Some(ReasoningFormat::AnthropicThinkingBlocks)
        );
        assert_eq!(
            response.usage.as_ref().map(|usage| usage.total_tokens),
            Some(Some(33))
        );
    }
}
