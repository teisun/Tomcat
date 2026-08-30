//! Media follow-ups must be appended after the entire provider tool-call batch.
//!
//! OpenAI permits a user message only after every call in the preceding assistant
//! tool batch has a corresponding tool result. This test uses the production
//! read-image path plus a persisted session so it catches both in-memory and
//! transcript-chain regressions.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::core::agent_loop::{AgentLoop, AgentLoopConfig, AgentRunOutcome};
use crate::core::llm::{
    ChatMessage, ChatMessageContent, ChatMessageContentPart, ChatMessageRole, StreamEvent,
};
use crate::core::permission::{DefaultPermissionGate, GateConfig, SessionGrants};
use crate::core::session::{assert_active_tool_result_integrity, SessionManager};
use crate::core::tools::pipeline::read_state::ReadFileState;
use crate::core::tools::primitive::{DefaultPrimitiveExecutor, PrimitiveExecutor};
use crate::infra::{DefaultEventBus, PrimitiveConfig, TracingAuditRecorder};
use crate::AllowAllConfirmation;

use super::mocks::{test_binding, RecordingStreamLlmProvider};

fn file_primitive(root: &std::path::Path) -> Arc<dyn PrimitiveExecutor> {
    let gate = DefaultPermissionGate::new(
        GateConfig {
            agent_definition_dir: root.to_path_buf(),
            workspace_roots: vec![],
            agent_trail_readonly_dirs: vec![],
            user_path_rules: vec![],
            user_bash_forbidden: vec![],
            user_bash_approval: vec![],
            auto_confirm: false,
        },
        SessionGrants::new(),
    )
    .into_arc();
    Arc::new(DefaultPrimitiveExecutor::new(
        PrimitiveConfig::default(),
        Arc::new(AllowAllConfirmation),
        Arc::new(TracingAuditRecorder),
        gate,
    ))
}

#[tokio::test]
async fn image_follow_up_waits_until_every_tool_result_in_the_batch() {
    let root = tempfile::tempdir().expect("temporary workspace");
    let image_path = root.path().join("evidence.png");
    let text_path = root.path().join("evidence.txt");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/llm_multimodal/sample_image.png"),
        &image_path,
    )
    .expect("copy PNG fixture");
    std::fs::write(&text_path, "second tool result\n").expect("write text fixture");

    let path_arguments =
        |path: &std::path::Path| serde_json::json!({ "path": path.to_string_lossy() }).to_string();
    let tool_batch = vec![
        Ok(StreamEvent::ToolCallDelta {
            index: 0,
            id: Some("read-image".to_string()),
            name: Some("read".to_string()),
            arguments_delta: Some(path_arguments(&image_path)),
        }),
        Ok(StreamEvent::ToolCallDelta {
            index: 1,
            id: Some("read-text".to_string()),
            name: Some("read".to_string()),
            arguments_delta: Some(path_arguments(&text_path)),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "tool_calls".to_string(),
        }),
    ];
    let final_reply = vec![
        Ok(StreamEvent::ContentDelta {
            delta: "batch complete".to_string(),
        }),
        Ok(StreamEvent::FinishReason {
            reason: "stop".to_string(),
        }),
    ];
    let (provider, requests) = RecordingStreamLlmProvider::new(vec![tool_batch, final_reply]);

    let session_root = tempfile::tempdir().expect("temporary session root");
    let manager = SessionManager::new(session_root.path().to_path_buf());
    manager
        .create_session(manager.current_session_key(), None)
        .expect("create session");
    let mut agent = AgentLoop::new(
        test_binding(Arc::new(provider), "gpt-4"),
        file_primitive(root.path()),
        Arc::new(DefaultEventBus::new()),
        AgentLoopConfig {
            session_id: "media-batch".to_string(),
            message_append_sink: Some(Arc::new(manager.clone())),
            read_file_state: Arc::new(ReadFileState::new()),
            ..Default::default()
        },
        CancellationToken::new(),
    );

    let outcome = agent.run(vec![ChatMessage::user("read both files")]).await;
    assert!(
        matches!(outcome, AgentRunOutcome::Completed(_)),
        "image follow-up must not break the tool-call chain: {outcome:?}"
    );

    let requests = requests.0.lock().expect("recorded requests");
    assert_eq!(
        requests.len(),
        2,
        "the batch must lead to a second model turn"
    );
    let second_turn = &requests[1].messages;
    let tool_indexes = second_turn
        .iter()
        .enumerate()
        .filter_map(|(index, message)| (message.role == ChatMessageRole::Tool).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(tool_indexes.len(), 2, "both tool results must be present");
    let image_follow_up_index = second_turn
        .iter()
        .enumerate()
        .find_map(|(index, message)| {
            matches!(
                &message.content,
                Some(ChatMessageContent::Parts(parts))
                    if parts.iter().any(|part| matches!(part, ChatMessageContentPart::InputImage { .. }))
            )
            .then_some(index)
        })
        .expect("second turn must include the PNG as an InputImage");
    assert!(
        image_follow_up_index > *tool_indexes.last().expect("tool result indexes"),
        "media user message must follow the complete tool batch; tool_indexes={tool_indexes:?}, image_follow_up_index={image_follow_up_index}"
    );

    assert_active_tool_result_integrity(&manager.get_entries(32).expect("read transcript"))
        .expect("persisted transcript must contain one result for every tool call");
}
