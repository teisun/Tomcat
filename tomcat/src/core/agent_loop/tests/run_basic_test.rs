//! # 基础 Run 路径测试（正向 + 重试 + 工具循环 + 边界）
//!
//! 覆盖最朴素的四条路径：
//!
//! - text-only：LLM 一次返回纯文本，run 退出携带 final_text；
//! - 重试：第 1 次 chat_stream 返回 429，第 2 次成功；
//! - 工具循环：第 1 次 LLM 返回 tool_call，工具执行后第 2 次返回纯文本；
//! - 空消息：messages=[] 不崩溃，run 仍能 Ok 返回。

use std::sync::Arc;
use std::sync::Mutex;

use base64::Engine as _;
use tokio_util::sync::CancellationToken;

use crate::core::agent_loop::{AgentLoop, AgentLoopConfig, AgentRunOutcome};
use crate::core::llm::multimodal::UNSUPPORTED_FILE_INPUT_PLACEHOLDER;
use crate::core::llm::{
    ChatMessage, ChatMessageContent, ChatMessageContentPart, MessageKind, StreamEvent,
};
use crate::core::session::manager::{estimate_msg_chars, ContextState, MessageAppendSink};
use crate::infra::error::{llm_http_status_error, AppError};
use crate::infra::event_bus::EventBus;
use crate::infra::{wire, DefaultEventBus, EventContext};

use super::mocks::{
    test_binding, MockLlmProvider, MockPrimitiveExecutor, RecordingStreamLlmProvider,
};

fn unsupported_file_stream() -> Vec<Result<StreamEvent, AppError>> {
    vec![
        Ok(StreamEvent::LlmError {
            reason: "error:invalid_request_error".to_string(),
            message:
                "[OneOfParam] [input[0].content[1]] [invalid_enum_value] Invalid value: 'input_file'. Supported values are: 'input_text'."
                    .to_string(),
            code: Some("invalid_request_error".to_string()),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "error:invalid_request_error".to_string(),
        }),
    ]
}

fn ok_text_stream(text: &str) -> Vec<Result<StreamEvent, AppError>> {
    vec![
        Ok(StreamEvent::ContentDelta {
            delta: text.to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ]
}

fn pdf_user_message() -> ChatMessage {
    let pdf_b64 = base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4\n%%EOF\n");
    ChatMessage::user_with_parts(vec![
        ChatMessageContentPart::text("summarize file"),
        ChatMessageContentPart::file_base64_data("notes.pdf", "application/pdf", pdf_b64)
            .expect("pdf part"),
    ])
}

fn overbudget_context_state(messages: Vec<ChatMessage>) -> ContextState {
    let estimate_context_chars = messages.iter().map(estimate_msg_chars).sum();
    ContextState {
        messages,
        estimate_context_chars,
        context_budget_chars: 10_000,
        context_budget_tokens: 2_500,
        last_api_usage: None,
        post_usage_appended_chars: 0,
        transcript_path: std::path::PathBuf::new(),
        latest_plan_event: None,
        resume_control: Default::default(),
        preheat: crate::core::compaction::preheat::Preheat::new(),
        session_obs: Default::default(),
        live: Default::default(),
    }
}

fn large_user_turn(label: &str) -> ChatMessage {
    ChatMessage::user(format!("{label}: {}", "x".repeat(3_200)))
}

#[derive(Default)]
struct RecordingAppendSink {
    next_id: Mutex<u32>,
    custom_entries: Mutex<Vec<serde_json::Value>>,
}

impl MessageAppendSink for RecordingAppendSink {
    fn append_message(&self, _value: serde_json::Value) -> Result<String, AppError> {
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        Ok(format!("msg-{}", *next))
    }

    fn append_custom_entry(&self, extra: serde_json::Value) -> Result<(), AppError> {
        self.custom_entries.lock().unwrap().push(extra);
        Ok(())
    }

    fn append_message_with_id(
        &self,
        _value: serde_json::Value,
        forced_id: &str,
    ) -> Result<String, AppError> {
        Ok(forced_id.to_string())
    }
}

#[tokio::test]
async fn run_returns_text_when_llm_returns_text_only() {
    let stream1: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "Hello".to_string(),
        }),
        Ok(StreamEvent::ContentDelta {
            delta: " world".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let llm = Arc::new(MockLlmProvider::new(vec![stream1]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let messages = vec![ChatMessage::user("hi")];
    let result = loop_.run(messages).await.unwrap();
    assert_eq!(result.final_text, "Hello world");
}

#[tokio::test]
async fn run_refuses_to_send_an_assistant_tailed_request() {
    let llm = Arc::new(MockLlmProvider::new(vec![]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        AgentLoopConfig {
            session_id: "assistant-tail".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_
        .run(vec![
            ChatMessage::user("hello"),
            ChatMessage::assistant("partial"),
        ])
        .await;
    assert!(matches!(
        outcome,
        AgentRunOutcome::Failed(error)
            if error.to_string().contains("tail is not a user input or completed tool result")
    ));
}

#[tokio::test]
async fn outbound_invariant_allows_every_legal_path() {
    let mut completion_nudge = ChatMessage::user("continue until the plan is complete");
    completion_nudge.kind = MessageKind::Nudge;
    let mut background_signal = ChatMessage::user("background task finished");
    background_signal.kind = MessageKind::Signal;
    let steering = ChatMessage::steering("answer in Chinese");
    let mut tool_call = ChatMessage::assistant("");
    tool_call.tool_calls = Some(vec![serde_json::json!({
        "id": "read-1",
        "type": "function",
        "function": { "name": "read", "arguments": "{}" },
    })]);
    let legal_paths = vec![
        (
            "completion guard continuation",
            vec![ChatMessage::user("start"), completion_nudge],
        ),
        (
            "background follow-up injection",
            vec![ChatMessage::user("start"), background_signal],
        ),
        (
            "mid-turn steering injection",
            vec![ChatMessage::user("start"), steering],
        ),
        (
            "completed tool round",
            vec![
                ChatMessage::user("read the file"),
                tool_call,
                ChatMessage::tool("read-1", "file contents"),
            ],
        ),
    ];

    for (path, messages) in legal_paths {
        let llm = Arc::new(MockLlmProvider::new(vec![ok_text_stream("accepted")]));
        let mut loop_ = AgentLoop::new(
            test_binding(llm, "gpt-4"),
            Arc::new(MockPrimitiveExecutor),
            Arc::new(DefaultEventBus::new()),
            AgentLoopConfig {
                session_id: format!("legal-tail-{path}"),
                ..Default::default()
            },
            CancellationToken::new(),
        );
        let outcome = loop_.run(messages).await;
        assert!(
            outcome.is_ok(),
            "{path} must remain legal after the outbound tail invariant"
        );
    }
}

/// 重试：Mock LLM 先返回 429 再返回成功 -> 自动重试后得到文本。
#[tokio::test]
async fn run_retries_on_429_then_succeeds() {
    let stream_err = vec![Err(llm_http_status_error("mock", 429, "rate limit"))];
    let stream_ok: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "OK".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let llm = Arc::new(MockLlmProvider::new(vec![stream_err, stream_ok]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        max_attempts: 3,
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let messages = vec![ChatMessage::user("hi")];
    let result = loop_.run(messages).await.unwrap();
    assert_eq!(result.final_text, "OK");
}

#[tokio::test]
async fn run_stops_after_one_request_when_overflow_trim_cannot_shrink_payload() {
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![
        vec![Err(llm_http_status_error(
            "mock",
            400,
            r#"{"error":{"code":"context_length_exceeded"}}"#,
        ))],
        ok_text_stream("must not send"),
    ]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            max_attempts: 4,
            retry_base_delay_ms: 0,
            session_id: "overflow-no-progress".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_.run(vec![ChatMessage::user("no context state")]).await;

    assert!(
        matches!(outcome, AgentRunOutcome::Failed(_)),
        "without ContextState the overflow retry cannot reduce its payload"
    );
    assert_eq!(
        requests.0.lock().unwrap().len(),
        1,
        "a retry with applied=false must fail honestly instead of resending the same payload"
    );
}

#[tokio::test]
async fn run_second_overflow_collapses_and_strictly_shrinks_main_requests() {
    let initial_messages = vec![
        large_user_turn("oldest"),
        large_user_turn("middle"),
        large_user_turn("latest"),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![
        vec![Err(llm_http_status_error(
            "mock",
            400,
            r#"{"error":{"code":"context_length_exceeded"}}"#,
        ))],
        vec![Err(llm_http_status_error(
            "mock",
            400,
            r#"{"error":{"code":"context_length_exceeded"}}"#,
        ))],
        ok_text_stream("recovered after collapse"),
    ]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            max_attempts: 4,
            retry_base_delay_ms: 0,
            session_id: "overflow-collapse-progress".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );
    loop_.set_context_state(Some(overbudget_context_state(initial_messages.clone())));

    let outcome = loop_.run(initial_messages).await;

    assert!(
        matches!(outcome, AgentRunOutcome::Completed(_)),
        "the third request should receive the collapsed context and succeed: {outcome:?}"
    );
    let recorded = requests.0.lock().unwrap().clone();
    assert_eq!(recorded.len(), 3, "original + L3 retry + collapse retry");
    let request_chars = |index: usize| {
        recorded[index]
            .messages
            .iter()
            .map(estimate_msg_chars)
            .sum::<usize>()
    };
    assert!(
        recorded[1].messages.len() < recorded[0].messages.len()
            && request_chars(1) < request_chars(0),
        "first overflow must make strict L3 progress"
    );
    assert!(
        recorded[2].messages.len() < recorded[1].messages.len()
            && request_chars(2) < request_chars(1),
        "second overflow must take Collapse and send a strictly smaller payload"
    );

    let context = loop_
        .take_context_state()
        .expect("context state remains available after recovery");
    assert_eq!(
        context.session_obs.compaction_count, 2,
        "one L3 reduction plus one Collapse must be recorded"
    );
    assert!(
        matches!(
            context.messages.as_slice(),
            [message] if message.kind == MessageKind::CompactionSummary
        ),
        "second overflow must replace retained history with a compaction summary"
    );
}

#[test]
fn retry_delay_uses_jitter_window_and_cap() {
    let min_delay = super::super::run::compute_retry_delay_ms(500, 2, 0);
    let max_delay = super::super::run::compute_retry_delay_ms(500, 2, 40);
    let capped = super::super::run::compute_retry_delay_ms(500, 20, 40);
    assert_eq!(min_delay, 400, "attempt=2 最小 jitter 应为 base 的 80%");
    assert_eq!(max_delay, 600, "attempt=2 最大 jitter 应为 base 的 120%");
    assert_eq!(capped, 8_000, "指数退避应被上限 cap 到 8s");
}

#[tokio::test]
async fn run_respects_configured_max_attempts() {
    let llm = Arc::new(MockLlmProvider::new(vec![
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        vec![
            Ok(StreamEvent::ContentDelta {
                delta: "UNREACHABLE".to_string(),
            }),
            Ok(StreamEvent::FinishReason {
                reason: "stop".to_string(),
            }),
        ],
    ]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        max_attempts: 2,
        retry_base_delay_ms: 0,
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let outcome = loop_.run(vec![ChatMessage::user("hi")]).await;
    assert!(
        matches!(outcome, AgentRunOutcome::Failed(_)),
        "max_attempts=2 时第 3 次成功不应被消费"
    );
}

#[tokio::test]
async fn run_honors_larger_configured_attempt_budget() {
    let stream_ok: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "AFTER_RETRIES".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let llm = Arc::new(MockLlmProvider::new(vec![
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        stream_ok,
    ]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        max_attempts: 5,
        retry_base_delay_ms: 0,
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let outcome = loop_.run(vec![ChatMessage::user("hi")]).await;
    let result = match outcome {
        AgentRunOutcome::Completed(result) => result,
        other => panic!("max_attempts=5 应允许第 5 次成功，实际: {other:?}"),
    };
    assert_eq!(result.final_text, "AFTER_RETRIES");
}

#[tokio::test]
async fn run_retries_unsupported_file_once_then_degrades_before_next_attempt() {
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![
        unsupported_file_stream(),
        unsupported_file_stream(),
        ok_text_stream("DEGRADED_OK"),
    ]);
    let llm = Arc::new(provider);
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let retry_events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let degrade_notices: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let retry_events = Arc::clone(&retry_events);
        event_bus.on(
            wire::WIRE_AUTO_RETRY_START,
            Box::new(move |_ctx: EventContext| {
                retry_events.lock().unwrap().push("retry".to_string());
                Ok(())
            }),
        );
    }
    {
        let degrade_notices = Arc::clone(&degrade_notices);
        event_bus.on(
            wire::WIRE_LLM_NOTICE,
            Box::new(move |ctx: EventContext| {
                if let Some(message) = ctx
                    .payload
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                {
                    degrade_notices.lock().unwrap().push(message.to_string());
                }
                Ok(())
            }),
        );
    }
    let config = AgentLoopConfig {
        max_attempts: 4,
        retry_base_delay_ms: 0,
        session_id: "s-unsupported-file".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let result = loop_.run(vec![pdf_user_message()]).await.unwrap();

    assert_eq!(result.final_text, "DEGRADED_OK");
    assert_eq!(
        retry_events.lock().unwrap().len(),
        2,
        "first failure should raw-retry once, second failure should start the degraded retry",
    );
    assert_eq!(
        degrade_notices.lock().unwrap().as_slice(),
        ["本轮附件未被当前端点接受，已按纯文本发送"],
    );

    let recorded = requests.0.lock().unwrap().clone();
    assert_eq!(
        recorded.len(),
        3,
        "expected original + raw retry + degraded retry"
    );
    for raw_request in &recorded[..2] {
        let user_message = raw_request
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, crate::core::llm::ChatMessageRole::User))
            .expect("user message");
        let Some(ChatMessageContent::Parts(parts)) = &user_message.content else {
            panic!(
                "expected multimodal user parts, got {:?}",
                user_message.content
            );
        };
        assert!(
            parts
                .iter()
                .any(|part| matches!(part, ChatMessageContentPart::InputFile { .. })),
            "the first two attempts must keep the original input_file: {parts:?}"
        );
    }
    let degraded_user_message = recorded[2]
        .messages
        .iter()
        .rev()
        .find(|message| matches!(message.role, crate::core::llm::ChatMessageRole::User))
        .expect("degraded user message");
    let Some(ChatMessageContent::Parts(parts)) = &degraded_user_message.content else {
        panic!(
            "expected degraded user parts, got {:?}",
            degraded_user_message.content
        );
    };
    assert!(
        parts
            .iter()
            .all(|part| !matches!(part, ChatMessageContentPart::InputFile { .. })),
        "degraded retry must strip input_file parts: {parts:?}"
    );
    assert!(
        parts.iter().any(|part| {
            matches!(
                part,
                ChatMessageContentPart::InputText { text }
                    if text.contains(UNSUPPORTED_FILE_INPUT_PLACEHOLDER)
            )
        }),
        "degraded retry must include the placeholder text: {parts:?}"
    );
}

#[tokio::test]
async fn run_unsupported_file_exhausts_full_retry_budget_before_failing() {
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![
        unsupported_file_stream(),
        unsupported_file_stream(),
        unsupported_file_stream(),
        unsupported_file_stream(),
    ]);
    let llm = Arc::new(provider);
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let retry_events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let retry_events = Arc::clone(&retry_events);
        event_bus.on(
            wire::WIRE_AUTO_RETRY_START,
            Box::new(move |_ctx: EventContext| {
                retry_events.lock().unwrap().push("retry".to_string());
                Ok(())
            }),
        );
    }
    let config = AgentLoopConfig {
        max_attempts: 4,
        retry_base_delay_ms: 0,
        session_id: "s-unsupported-file-fatal".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let outcome = loop_.run(vec![pdf_user_message()]).await;

    assert!(
        matches!(outcome, AgentRunOutcome::Failed(_)),
        "four refusals should still end as one final failure",
    );
    assert_eq!(retry_events.lock().unwrap().len(), 3);
    assert_eq!(
        requests.0.lock().unwrap().len(),
        4,
        "unsupported multimodal fallback must still honor the configured retry budget",
    );
}

#[tokio::test(start_paused = true)]
async fn run_retry_sleep_is_interruptible() {
    let llm = Arc::new(MockLlmProvider::new(vec![vec![Err(
        llm_http_status_error("mock", 503, "service unavailable"),
    )]]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        max_attempts: 3,
        retry_base_delay_ms: 5_000,
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let cancel = abort.clone();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let task = tokio::spawn(async move { loop_.run(vec![ChatMessage::user("hi")]).await });
    tokio::task::yield_now().await;
    cancel.cancel();
    let outcome = task.await.expect("join ok");
    assert!(
        matches!(outcome, AgentRunOutcome::Interrupted(_)),
        "退避 sleep 期间 cancel 应立即打断"
    );
}

#[tokio::test]
async fn run_persists_auto_retry_events_to_transcript_sink() {
    let llm = Arc::new(MockLlmProvider::new(vec![
        vec![Err(llm_http_status_error(
            "mock",
            503,
            "service unavailable",
        ))],
        ok_text_stream("RECOVERED"),
    ]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let sink = Arc::new(RecordingAppendSink::default());
    let config = AgentLoopConfig {
        max_attempts: 2,
        retry_base_delay_ms: 0,
        session_id: "s-retry-transcript".to_string(),
        message_append_sink: Some(sink.clone()),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );

    let outcome = loop_.run(vec![ChatMessage::user("hi")]).await;
    assert!(matches!(outcome, AgentRunOutcome::Completed(_)));

    let entries = sink.custom_entries.lock().unwrap().clone();
    assert_eq!(entries.len(), 2, "expected retry start + end entries");
    assert_eq!(
        entries[0]["event"].as_str(),
        Some(wire::WIRE_AUTO_RETRY_START)
    );
    assert_eq!(entries[0]["attempt"].as_u64(), Some(2));
    assert_eq!(
        entries[1]["event"].as_str(),
        Some(wire::WIRE_AUTO_RETRY_END)
    );
    assert_eq!(entries[1]["success"].as_bool(), Some(true));
}

/// 空正文、无 thinking 的纯工具轮是合法中间态，不能被空回合守卫误判。
#[tokio::test]
async fn run_pure_tool_turn_without_thinking_completes_in_two_requests() {
    let stream_tool: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("read".to_string()),
            arguments_delta: Some(r#"{"path":"/tmp/x"}"#.to_string()),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "tool_calls".to_string(),
        }),
    ];
    let stream_text: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "done".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![stream_tool, stream_text]);
    let llm = Arc::new(provider);
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let messages = vec![ChatMessage::user("read /tmp/x")];
    let result = loop_.run(messages).await.unwrap();
    assert!(result.final_text.contains("done"));
    assert_eq!(
        requests.0.lock().unwrap().len(),
        2,
        "pure tool turn must execute then ask the model for its follow-up exactly once"
    );
}

/// 某些兼容端点会在合法的结构化空收尾使用 `end_turn`，但没有正文和 thinking。
/// 它与“只思考、不回答”不同，不能被终止守卫当成失败。
#[tokio::test]
async fn run_structured_end_turn_without_content_is_not_empty_turn_failure() {
    let (provider, requests) =
        RecordingStreamLlmProvider::new(vec![vec![Ok(StreamEvent::FinishReason {
            reason: "end_turn".to_string(),
        })]]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            session_id: "structured-empty-end-turn".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_
        .run(vec![ChatMessage::user("return structured empty")])
        .await;

    assert!(
        matches!(outcome, AgentRunOutcome::Completed(_)),
        "end_turn without content or thinking is a provider-valid structured completion: {outcome:?}"
    );
    assert_eq!(requests.0.lock().unwrap().len(), 1);
}

/// 工具已完整执行时，下一次收尾请求即使是空 `end_turn` 也不能把整个工具回合判失败。
#[tokio::test]
async fn run_empty_end_turn_after_tool_result_is_not_empty_turn_failure() {
    let stream_tool = vec![
        Ok(StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("call_empty_tail".to_string()),
            name: Some("read".to_string()),
            arguments_delta: Some(r#"{"path":"/tmp/x"}"#.to_string()),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "tool_calls".to_string(),
        }),
    ];
    let stream_empty_tail = vec![Ok(StreamEvent::FinishReason {
        reason: "end_turn".to_string(),
    })];
    let (provider, requests) =
        RecordingStreamLlmProvider::new(vec![stream_tool, stream_empty_tail]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            session_id: "tool-result-empty-tail".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_.run(vec![ChatMessage::user("read then end")]).await;

    let AgentRunOutcome::Completed(result) = outcome else {
        panic!("a completed tool round followed by end_turn must remain successful");
    };
    assert!(result.final_text.is_empty());
    assert!(
        result
            .new_messages
            .iter()
            .any(|message| message.role == crate::core::llm::ChatMessageRole::Tool),
        "the successfully produced tool result must survive the empty tail"
    );
    assert_eq!(
        requests.0.lock().unwrap().len(),
        2,
        "one request for the tool turn and one for its empty structured tail"
    );
}

#[tokio::test]
async fn run_tool_loop_emits_display_on_tool_execution_end() {
    let stream_tool: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("write".to_string()),
            arguments_delta: Some(
                r#"{"path":"~/workspace/demo.txt","content":"","overwrite":false}"#.to_string(),
            ),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "tool_calls".to_string(),
        }),
    ];
    let stream_text: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "done".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let llm = Arc::new(MockLlmProvider::new(vec![stream_tool, stream_text]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
    let captured_cb = Arc::clone(&captured);
    event_bus.on(
        wire::WIRE_TOOL_EXECUTION_END,
        Box::new(move |ctx: EventContext| {
            *captured_cb.lock().unwrap() = Some(ctx.payload.clone());
            Ok(())
        }),
    );
    let config = AgentLoopConfig {
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let messages = vec![ChatMessage::user("write demo file")];
    let _ = loop_.run(messages).await.unwrap();

    let payload = captured
        .lock()
        .unwrap()
        .clone()
        .expect("应捕获到 tool_execution_end payload");
    assert_eq!(payload["toolName"].as_str(), Some("write"));
    assert_eq!(payload["display"]["kind"].as_str(), Some("file"));
    assert_eq!(
        payload["display"]["file"].as_str(),
        Some("~/workspace/demo.txt")
    );
}

/// 边界：空消息列表必须被出站不变量明确拒绝，而不是发送畸形请求。
#[tokio::test]
async fn run_empty_messages_fails_before_calling_the_llm() {
    let stream1: Vec<Result<StreamEvent, AppError>> = vec![Ok(StreamEvent::FinishReason {
        reason: "stop".to_string(),
    })];
    let llm = Arc::new(MockLlmProvider::new(vec![stream1]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let messages: Vec<ChatMessage> = vec![];
    let result = loop_.run(messages).await;
    assert!(matches!(
        result,
        AgentRunOutcome::Failed(error)
            if error.to_string().contains("tail is not a user input or completed tool result")
    ));
}

#[tokio::test]
async fn reasoning_only_empty_turn_is_fatal_and_never_auto_retries() {
    let stream = vec![
        Ok(StreamEvent::ReasoningSnapshot {
            thinking_text: Some("first reason through the task".to_string()),
            reasoning_continuation: None,
            continuity: None,
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![stream]);
    let llm = Arc::new(provider);
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            max_attempts: 4,
            retry_base_delay_ms: 0,
            session_id: "s-reasoning-only".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_.run(vec![ChatMessage::user("hi")]).await;
    assert!(
        matches!(outcome, AgentRunOutcome::Failed(_)),
        "thinking-only response must surface as a failed turn"
    );
    assert_eq!(
        requests.0.lock().unwrap().len(),
        1,
        "the empty-turn guard must not retry an unchanged request"
    );
}

#[tokio::test]
async fn truncated_thinking_prefix_is_fatal_and_never_auto_retries() {
    let stream = vec![
        Ok(StreamEvent::ReasoningSnapshot {
            thinking_text: Some(
                "I will reason through every implementation detail first.".to_string(),
            ),
            reasoning_continuation: None,
            continuity: None,
        }),
        Ok(StreamEvent::ContentDelta {
            delta: "I will reason".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![stream]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            max_attempts: 4,
            retry_base_delay_ms: 0,
            session_id: "s-truncated-thinking".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_.run(vec![ChatMessage::user("hi")]).await;
    assert!(matches!(outcome, AgentRunOutcome::Failed(_)));
    assert_eq!(
        requests.0.lock().unwrap().len(),
        1,
        "the truncated-thinking guard must not retry an unchanged request"
    );
}

#[tokio::test]
async fn duplicated_thinking_as_body_is_fatal_and_never_auto_retries() {
    let stream = vec![
        Ok(StreamEvent::ReasoningSnapshot {
            thinking_text: Some("I will inspect the implementation before answering.".to_string()),
            reasoning_continuation: None,
            continuity: None,
        }),
        Ok(StreamEvent::ContentDelta {
            delta: "I will inspect the implementation before answering.".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![stream]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            max_attempts: 4,
            retry_base_delay_ms: 0,
            session_id: "s-duplicated-thinking".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    assert!(matches!(
        loop_.run(vec![ChatMessage::user("hi")]).await,
        AgentRunOutcome::Failed(_)
    ));
    assert_eq!(requests.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn short_non_prefix_body_after_thinking_remains_a_valid_reply() {
    let stream = vec![
        Ok(StreamEvent::ReasoningSnapshot {
            thinking_text: Some("I will inspect the implementation before answering.".to_string()),
            reasoning_continuation: None,
            continuity: None,
        }),
        Ok(StreamEvent::ContentDelta {
            delta: "Done.".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![stream]);
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            max_attempts: 4,
            retry_base_delay_ms: 0,
            session_id: "s-short-final-body".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    assert!(matches!(
        loop_.run(vec![ChatMessage::user("hi")]).await,
        AgentRunOutcome::Completed(_)
    ));
    assert_eq!(requests.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn final_assistant_message_persists_provider_usage() {
    let stream = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "done".to_string(),
        }),
        Ok(StreamEvent::Usage {
            prompt_tokens: 12,
            completion_tokens: 34,
            cache_read_tokens: Some(9),
            cache_write_tokens: Some(3),
            total_tokens: Some(46),
            reasoning_tokens: Some(20),
            text_tokens: Some(14),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let mut loop_ = AgentLoop::new(
        test_binding(Arc::new(MockLlmProvider::new(vec![stream])), "gpt-4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            session_id: "s-usage-persist".to_string(),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = loop_.run(vec![ChatMessage::user("hi")]).await;
    let AgentRunOutcome::Completed(result) = outcome else {
        panic!("text reply should complete");
    };
    let usage = result
        .new_messages
        .iter()
        .find(|message| matches!(message.role, crate::core::llm::ChatMessageRole::Assistant))
        .expect("result must include an assistant message")
        .usage
        .as_ref()
        .expect("assistant transcript message must keep provider usage");
    assert_eq!(usage.prompt_tokens, 12);
    assert_eq!(usage.completion_tokens, 34);
    assert_eq!(usage.cache_read_tokens, Some(9));
    assert_eq!(usage.cache_write_tokens, Some(3));
    assert_eq!(usage.total_tokens, Some(46));
    assert_eq!(usage.reasoning_tokens, Some(20));
    assert_eq!(usage.text_tokens, Some(14));
}

#[tokio::test]
async fn run_emits_tool_call_streaming_before_tool_execution_start_for_write() {
    let stream_tool: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("call_1".to_string()),
            name: Some("write".to_string()),
            arguments_delta: Some(
                r#"{"path":"~/workspace/demo.txt","content":"hello","overwrite":false}"#
                    .to_string(),
            ),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "tool_calls".to_string(),
        }),
    ];
    let stream_text: Vec<Result<StreamEvent, AppError>> = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "done".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let llm = Arc::new(MockLlmProvider::new(vec![stream_tool, stream_text]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let observed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    for wire_name in [
        wire::WIRE_TOOL_CALL_STREAMING,
        wire::WIRE_TOOL_EXECUTION_START,
        wire::WIRE_TOOL_EXECUTION_END,
    ] {
        let sink = Arc::clone(&observed);
        let name = wire_name.to_string();
        event_bus.on(
            wire_name,
            Box::new(move |_ctx: EventContext| {
                sink.lock().unwrap().push(name.clone());
                Ok(())
            }),
        );
    }
    let config = AgentLoopConfig {
        session_id: "s-streaming-order".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(
        test_binding(llm, "gpt-4"),
        primitive,
        event_bus,
        config,
        abort,
    );
    let messages = vec![ChatMessage::user("write demo file")];
    let _ = loop_.run(messages).await.unwrap();

    assert_eq!(
        observed.lock().unwrap().clone(),
        vec![
            wire::WIRE_TOOL_CALL_STREAMING.to_string(),
            wire::WIRE_TOOL_EXECUTION_START.to_string(),
            wire::WIRE_TOOL_EXECUTION_END.to_string(),
        ]
    );
}
