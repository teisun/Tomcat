use serde_json::{json, Value};

use crate::core::llm::files_api::FilesApiAdapter;
use crate::core::llm::multimodal::degrade_placeholder;
use crate::core::llm::replay_policy::{
    plan_scoped, replay_requirement_for_profile, ProviderCompatProfile, ReplayAction, ReplayWindow,
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
    MessageBlock {
        message_idx: usize,
        block_idx: usize,
    },
}

struct RenderedMessages {
    system: Vec<Value>,
    messages: Vec<Value>,
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
        request.ephemeral_tail_count,
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
    apply_cache_breakpoints(
        &mut rendered.system,
        &mut rendered.messages,
        tools.as_deref_mut(),
        cache_breakpoint_candidates,
    );
    let thinking_request =
        resolve_anthropic_request(thinking_cfg, thinking_format, request.max_tokens);

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
    ephemeral_tail_count: usize,
) -> RenderedMessages {
    let mut system_chunks = Vec::new();
    let mut out = Vec::new();
    let mut last_non_ephemeral_block = None;
    let mut last_compaction_summary_block = None;
    let window = ReplayWindow::compute(messages);
    let first_ephemeral_idx = messages.len().saturating_sub(ephemeral_tail_count);

    for (idx, original) in messages.iter().enumerate() {
        let is_ephemeral = idx >= first_ephemeral_idx;
        let action = if continuity_enabled {
            plan_scoped(target, original, window.contains(idx))
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
                    if let Some(out_idx) = push_role_message(&mut out, "user", content) {
                        let last_block =
                            last_content_block_index(&out, out_idx).map(|idx| (out_idx, idx));
                        if !is_ephemeral {
                            last_non_ephemeral_block = last_block;
                        }
                        if matches!(msg.kind, crate::core::llm::MessageKind::CompactionSummary) {
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
                        if !is_ephemeral {
                            last_non_ephemeral_block =
                                last_content_block_index(&out, out_idx).map(|idx| (out_idx, idx));
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
                    if !is_ephemeral {
                        last_non_ephemeral_block =
                            last_content_block_index(&out, out_idx).map(|idx| (out_idx, idx));
                    }
                }
            }
        }
    }

    let mut cache_breakpoint_candidates = Vec::new();
    if let Some(last_system_idx) = system_chunks.len().checked_sub(1) {
        cache_breakpoint_candidates.push(CacheBreakpoint::SystemBlock(last_system_idx));
    }
    if let Some((message_idx, block_idx)) = last_non_ephemeral_block {
        cache_breakpoint_candidates.push(CacheBreakpoint::MessageBlock {
            message_idx,
            block_idx,
        });
    }
    if let Some((message_idx, block_idx)) = last_compaction_summary_block {
        cache_breakpoint_candidates.push(CacheBreakpoint::MessageBlock {
            message_idx,
            block_idx,
        });
    }
    RenderedMessages {
        system: system_chunks,
        messages: out,
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
) {
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

    use super::{build_request_body, final_stream_events, response_to_chat_response};
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
    fn build_request_body_extracts_system_and_user_messages() {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("be helpful"),
                ChatMessage::user("hello"),
            ],
            model: "ignored".to_string(),
            temperature: Some(0.2),
            max_tokens: Some(4096),
            stream: Some(true),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            ephemeral_tail_count: 0,
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
    fn last_breakpoint_precedes_the_ephemeral_tail() {
        let mut summary = ChatMessage::user("compacted history");
        summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("stable system"),
                summary,
                ChatMessage::user("latest persisted input"),
                ChatMessage::user("<system_reminder>runtime state</system_reminder>"),
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
            ephemeral_tail_count: 1,
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
        assert_eq!(content[0]["cache_control"]["type"], "ephemeral");
        assert_eq!(content[1]["cache_control"]["type"], "ephemeral");
        assert!(
            content[2].get("cache_control").is_none(),
            "the synthetic tail must not become a cache breakpoint"
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
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system("stable system"),
                first_summary,
                second_summary,
                latest_summary,
                ChatMessage::user("latest persisted input"),
                ChatMessage::user("<system_reminder>runtime state</system_reminder>"),
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
            ephemeral_tail_count: 1,
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

        assert_eq!(count_cache_controls(&body), 4);
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
        assert_eq!(content[2]["cache_control"]["type"], "ephemeral");
        assert_eq!(content[3]["cache_control"]["type"], "ephemeral");
        assert!(
            content[4].get("cache_control").is_none(),
            "the synthetic tail must never become a cache breakpoint"
        );
    }

    #[test]
    fn cache_breakpoints_handle_empty_system_and_tools() {
        let mut summary = ChatMessage::user("compacted history");
        summary.kind = crate::core::llm::MessageKind::CompactionSummary;
        let request = ChatRequest {
            messages: vec![
                summary,
                ChatMessage::user("<system_reminder>runtime state</system_reminder>"),
            ],
            model: "ignored".to_string(),
            ephemeral_tail_count: 1,
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

        assert_eq!(count_cache_controls(&body), 1);
        let content = body["messages"][0]["content"]
            .as_array()
            .expect("contiguous user messages merge into blocks");
        assert_eq!(content[0]["cache_control"]["type"], "ephemeral");
        assert!(content[1].get("cache_control").is_none());
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
        let request = ChatRequest {
            messages: vec![ChatMessage::user(
                "<system_reminder>runtime-only state</system_reminder>",
            )],
            ephemeral_tail_count: 1,
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
            stream: Some(false),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            ephemeral_tail_count: 0,
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
            stream: Some(false),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            ephemeral_tail_count: 0,
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
            stream: Some(false),
            model_override: None,
            thinking_level: None,
            cache_key: None,
            ephemeral_tail_count: 0,
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
