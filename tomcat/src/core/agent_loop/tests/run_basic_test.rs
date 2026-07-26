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
use crate::core::llm::{ChatMessage, ChatMessageContent, ChatMessageContentPart, StreamEvent};
use crate::core::llm::multimodal::UNSUPPORTED_FILE_INPUT_PLACEHOLDER;
use crate::infra::error::{llm_http_status_error, AppError};
use crate::infra::event_bus::EventBus;
use crate::infra::{wire, DefaultEventBus, EventContext};

use super::mocks::{
    MockLlmProvider, MockPrimitiveExecutor, RecordingStreamLlmProvider,
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
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
    let messages = vec![ChatMessage::user("hi")];
    let result = loop_.run(messages).await.unwrap();
    assert_eq!(result.final_text, "Hello world");
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
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
    let messages = vec![ChatMessage::user("hi")];
    let result = loop_.run(messages).await.unwrap();
    assert_eq!(result.final_text, "OK");
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
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
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
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
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
                if let Some(message) = ctx.payload.get("message").and_then(serde_json::Value::as_str)
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
        model: "gpt-4".to_string(),
        session_id: "s-unsupported-file".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
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
    assert_eq!(recorded.len(), 3, "expected original + raw retry + degraded retry");
    for raw_request in &recorded[..2] {
        let user_message = raw_request
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, crate::core::llm::ChatMessageRole::User))
            .expect("user message");
        let Some(ChatMessageContent::Parts(parts)) = &user_message.content else {
            panic!("expected multimodal user parts, got {:?}", user_message.content);
        };
        assert!(
            parts.iter().any(|part| matches!(part, ChatMessageContentPart::InputFile { .. })),
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
        parts.iter().all(|part| !matches!(part, ChatMessageContentPart::InputFile { .. })),
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
        model: "gpt-4".to_string(),
        session_id: "s-unsupported-file-fatal".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
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
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let cancel = abort.clone();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
    let task = tokio::spawn(async move { loop_.run(vec![ChatMessage::user("hi")]).await });
    tokio::task::yield_now().await;
    cancel.cancel();
    let outcome = task.await.expect("join ok");
    assert!(
        matches!(outcome, AgentRunOutcome::Interrupted(_)),
        "退避 sleep 期间 cancel 应立即打断"
    );
}

/// 工具循环：第 1 次 LLM 返回 read tool call，第 2 次返回纯文本；断言 final_text 含第 2 次文本。
#[tokio::test]
async fn run_tool_loop_calls_tool_then_returns_text() {
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
    let llm = Arc::new(MockLlmProvider::new(vec![stream_tool, stream_text]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
    let messages = vec![ChatMessage::user("read /tmp/x")];
    let result = loop_.run(messages).await.unwrap();
    assert!(result.final_text.contains("done"));
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
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
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

/// 边界：空消息列表不崩溃，run 仍可调用（LLM 可能返回错误或空回复）。
#[tokio::test]
async fn run_empty_messages_does_not_crash() {
    let stream1: Vec<Result<StreamEvent, AppError>> = vec![Ok(StreamEvent::FinishReason {
        reason: "stop".to_string(),
    })];
    let llm = Arc::new(MockLlmProvider::new(vec![stream1]));
    let primitive = Arc::new(MockPrimitiveExecutor);
    let event_bus = Arc::new(DefaultEventBus::new());
    let config = AgentLoopConfig {
        model: "gpt-4".to_string(),
        session_id: "s1".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
    let messages: Vec<ChatMessage> = vec![];
    let result = loop_.run(messages).await;
    assert!(result.is_ok());
    assert!(result.unwrap().final_text.is_empty());
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
        model: "gpt-4".to_string(),
        session_id: "s-streaming-order".to_string(),
        ..Default::default()
    };
    let abort = CancellationToken::new();
    let mut loop_ = AgentLoop::new(llm, primitive, event_bus, config, abort);
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
