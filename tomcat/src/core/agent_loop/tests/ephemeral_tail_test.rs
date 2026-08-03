use std::sync::Arc;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use super::mocks::{test_binding, MockLlmProvider, MockPrimitiveExecutor};
use crate::core::agent_loop::reasoning_loop::{cache_key_for, with_ephemeral_tail};
use crate::core::agent_loop::{AgentLoop, AgentLoopConfig, EphemeralTailProvider, SubagentType};
use crate::core::llm::{ChatMessage, ChatRequest, MessageKind};
use crate::infra::DefaultEventBus;

struct StaticTail(&'static str);

impl EphemeralTailProvider for StaticTail {
    fn render_ephemeral_tail(&self) -> String {
        self.0.to_string()
    }
}

struct MutableTail(Arc<Mutex<String>>);

impl EphemeralTailProvider for MutableTail {
    fn render_ephemeral_tail(&self) -> String {
        self.0.lock().clone()
    }
}

fn agent(config: AgentLoopConfig) -> AgentLoop {
    AgentLoop::new(
        test_binding(Arc::new(MockLlmProvider::new(vec![])), "gpt-5.4"),
        Arc::new(MockPrimitiveExecutor),
        Arc::new(DefaultEventBus::new()),
        config,
        CancellationToken::new(),
    )
}

#[test]
fn ephemeral_tail_is_request_only_and_not_a_persisted_message() {
    let persistent = vec![ChatMessage::system("stable"), ChatMessage::user("question")];
    let loop_ = agent(AgentLoopConfig {
        ephemeral_tail_provider: Some(Arc::new(StaticTail(
            "<system_reminder>runtime state</system_reminder>",
        ))),
        ..Default::default()
    });

    let (outgoing, ephemeral_tail_count) = with_ephemeral_tail(&persistent, &loop_);

    assert_eq!(ephemeral_tail_count, 1);
    assert_eq!(persistent.len(), 2, "source history must stay unchanged");
    assert_eq!(outgoing.len(), 3);
    assert_eq!(
        outgoing.last().and_then(ChatMessage::text_content),
        Some("<system_reminder>runtime state</system_reminder>")
    );
}

#[test]
fn mutable_ephemeral_tail_does_not_rewrite_existing_history() {
    let tail = Arc::new(Mutex::new(
        "<system_reminder>permissions: initial</system_reminder>".to_string(),
    ));
    let loop_ = agent(AgentLoopConfig {
        session_id: "cache-prefix-session".to_string(),
        tool_definitions: vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "read",
                "description": "read a file",
                "parameters": {"type": "object"}
            }
        })],
        ephemeral_tail_provider: Some(Arc::new(MutableTail(Arc::clone(&tail)))),
        ..Default::default()
    });
    let first_history = vec![
        ChatMessage::system("stable system"),
        ChatMessage::user("first"),
    ];
    let (first_messages, first_tail_count) = with_ephemeral_tail(&first_history, &loop_);
    let first_request = ChatRequest {
        messages: first_messages,
        model: "gpt-5.4".to_string(),
        cache_key: cache_key_for(&loop_),
        ephemeral_tail_count: first_tail_count,
        tools: Some(loop_.config.tool_definitions.clone()),
        ..Default::default()
    };

    let mut second_history = first_history.clone();
    second_history.push(ChatMessage::assistant("first response"));
    second_history.push(ChatMessage::user("second"));
    *tail.lock() = "<system_reminder>permissions: granted</system_reminder>".to_string();
    let (second_messages, second_tail_count) = with_ephemeral_tail(&second_history, &loop_);
    let second_request = ChatRequest {
        messages: second_messages,
        model: "gpt-5.4".to_string(),
        cache_key: cache_key_for(&loop_),
        ephemeral_tail_count: second_tail_count,
        tools: Some(loop_.config.tool_definitions.clone()),
        ..Default::default()
    };

    let first_persisted_len = first_request.messages.len() - first_tail_count;
    assert_eq!(first_tail_count, 1);
    assert_eq!(second_tail_count, 1);
    assert_eq!(first_request.tools, second_request.tools);
    assert_eq!(first_request.cache_key, second_request.cache_key);
    assert_eq!(
        serde_json::to_string(&second_request.messages[..first_persisted_len])
            .expect("messages serialize for byte-level comparison"),
        serde_json::to_string(&first_request.messages[..first_persisted_len])
            .expect("messages serialize for byte-level comparison"),
        "a runtime-tail change may append history but must not rewrite its cached prefix"
    );
    assert_eq!(
        first_request.messages[0].text_content(),
        second_request.messages[0].text_content(),
        "the system message belongs to the stable prefix"
    );
    assert_ne!(
        second_request
            .messages
            .last()
            .and_then(ChatMessage::text_content),
        first_request
            .messages
            .last()
            .and_then(ChatMessage::text_content),
        "only the synthetic tail may change with runtime permissions"
    );
}

#[test]
fn cache_key_is_scoped_by_session_and_request_family() {
    let main = agent(AgentLoopConfig {
        session_id: "session-1".to_string(),
        subagent_type: SubagentType::User,
        ..Default::default()
    });
    let verifier = agent(AgentLoopConfig {
        session_id: "session-1".to_string(),
        subagent_type: SubagentType::Verifier,
        ..Default::default()
    });
    let missing_session = agent(AgentLoopConfig::default());

    assert_eq!(cache_key_for(&main).as_deref(), Some("session-1:main"));
    assert_eq!(
        cache_key_for(&verifier).as_deref(),
        Some("session-1:subagent:verifier")
    );
    assert_ne!(cache_key_for(&main), cache_key_for(&verifier));
    assert_eq!(cache_key_for(&missing_session), None);
}

#[test]
fn subagent_request_uses_its_own_cache_key_family_and_own_tail() {
    let parent = agent(AgentLoopConfig {
        session_id: "session-1".to_string(),
        subagent_type: SubagentType::User,
        ephemeral_tail_provider: Some(Arc::new(StaticTail("parent runtime facts"))),
        ..Default::default()
    });
    let subagent = agent(AgentLoopConfig {
        session_id: "session-1".to_string(),
        subagent_type: SubagentType::Verifier,
        ephemeral_tail_provider: Some(Arc::new(StaticTail("verifier runtime facts"))),
        ..Default::default()
    });
    let history = vec![ChatMessage::system("stable"), ChatMessage::user("question")];

    let (parent_request, parent_tail_count) = with_ephemeral_tail(&history, &parent);
    let (subagent_request, subagent_tail_count) = with_ephemeral_tail(&history, &subagent);

    assert_eq!(parent_tail_count, 1);
    assert_eq!(subagent_tail_count, 1);
    assert_eq!(
        parent_request.last().and_then(ChatMessage::text_content),
        Some("parent runtime facts")
    );
    assert_eq!(
        subagent_request.last().and_then(ChatMessage::text_content),
        Some("verifier runtime facts")
    );
    assert_ne!(cache_key_for(&parent), cache_key_for(&subagent));
}

#[test]
fn ephemeral_tail_survives_collapse() {
    let loop_ = agent(AgentLoopConfig {
        ephemeral_tail_provider: Some(Arc::new(StaticTail(
            "<system_reminder>current permissions</system_reminder>",
        ))),
        ..Default::default()
    });
    let mut summary = ChatMessage::user("compacted history");
    summary.kind = MessageKind::CompactionSummary;
    let collapsed_history = vec![
        ChatMessage::system("stable system"),
        summary,
        ChatMessage::user("new question"),
    ];

    let (request, tail_count) = with_ephemeral_tail(&collapsed_history, &loop_);

    assert_eq!(tail_count, 1);
    assert_eq!(request.len(), collapsed_history.len() + 1);
    assert_eq!(request[1].kind, MessageKind::CompactionSummary);
    assert_eq!(
        request.last().and_then(ChatMessage::text_content),
        Some("<system_reminder>current permissions</system_reminder>")
    );
}
