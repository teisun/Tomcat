use std::sync::Arc;

use async_trait::async_trait;
use serial_test::serial;

use super::super::cmd_compact::run;
use super::super::parse::ChatCommandOutcome;
use crate::api::chat::ChatContext;
use crate::core::llm::{
    ChatMessage, ChatRequest, ChatResponse, ChatResponseChoice, LlmProvider, LlmResolver, LlmScene,
    ResolvedCall,
};
use crate::core::session::TranscriptEntry;
use crate::{init_context_state, AppConfig, AppError};

/// `/compact` should use the non-streaming compaction scene; a streamed main-turn request here
/// would mean the CLI command escaped its intended path.
struct CompactOnlyProvider;

#[async_trait]
impl LlmProvider for CompactOnlyProvider {
    fn provider_name(&self) -> &str {
        "compact-only"
    }

    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, AppError> {
        Ok(ChatResponse {
            id: Some("compact-cli-summary".to_string()),
            choices: vec![ChatResponseChoice {
                index: 0,
                message: ChatMessage::assistant("CLI compacted summary."),
                finish_reason: Some("stop".to_string()),
            }],
            usage: None,
        })
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> Result<
        Box<
            dyn tokio_stream::Stream<Item = Result<crate::core::llm::StreamEvent, AppError>>
                + Send
                + Unpin,
        >,
        AppError,
    > {
        Err(AppError::Llm(
            "the /compact CLI command must not start a streamed main turn".to_string(),
        ))
    }

    fn count_tokens(&self, _messages: &[ChatMessage]) -> Result<u32, AppError> {
        Ok(0)
    }
}

struct FixedResolver {
    catalog: Arc<crate::core::llm::ModelCatalog>,
    provider: Arc<dyn LlmProvider>,
}

impl LlmResolver for FixedResolver {
    fn resolve(
        &self,
        _scene: LlmScene,
        session_override: Option<&str>,
    ) -> Result<ResolvedCall, AppError> {
        let model = self
            .catalog
            .lookup_explicit(session_override.unwrap_or("gpt-5.4"))?;
        Ok(ResolvedCall::from_parts_unchecked(
            Arc::clone(&self.provider),
            model.id.clone(),
            model.request_model_name().to_string(),
        ))
    }
}

#[tokio::test]
#[serial]
async fn compact_command_persists_boundary_and_rehydrates_cli_context() {
    const API_KEY_ENV: &str = "TOMCAT_CMD_COMPACT_TEST_KEY";
    let old_key = std::env::var_os(API_KEY_ENV);
    unsafe { std::env::set_var(API_KEY_ENV, "test-key") };

    let work_dir = tempfile::tempdir().expect("work dir");
    let mut config = AppConfig::default();
    config.storage.work_dir = Some(work_dir.path().to_string_lossy().to_string());
    config.llm.api_key_env = Some(API_KEY_ENV.to_string());
    let mut ctx = ChatContext::from_config(config).expect("chat context");
    let provider: Arc<dyn LlmProvider> = Arc::new(CompactOnlyProvider);
    ctx.global_services.llm_resolver = Arc::new(FixedResolver {
        catalog: ctx.global_services.model_catalog.snapshot(),
        provider,
    });

    ctx.session_runtime
        .session
        .append_message(serde_json::json!({
            "role": "user",
            "content": format!("first CLI message: {}", "historical detail ".repeat(2_000)),
        }))
        .expect("append user");
    ctx.session_runtime
        .session
        .append_message(serde_json::json!({
            "role": "assistant",
            "content": format!("first CLI reply: {}", "historical detail ".repeat(2_000)),
        }))
        .expect("append assistant");
    let mut context_state = init_context_state(
        &ctx.session_runtime.session,
        &ctx.config.context,
        "system prompt",
    )
    .expect("hydrate CLI context");
    let before_ratio = context_state.usage_ratio();

    let outcome = run(&ctx, &mut context_state, "system prompt").await;

    assert!(matches!(outcome, ChatCommandOutcome::Handled));
    assert!(
        ctx.session_runtime
            .session
            .get_entries(16)
            .expect("transcript entries")
            .iter()
            .any(|entry| matches!(entry, TranscriptEntry::BranchSummary(summary) if summary.is_boundary == Some(true))),
        "/compact must persist a durable boundary"
    );
    assert!(
        context_state
            .messages
            .iter()
            .any(|message| message.kind == crate::core::llm::MessageKind::CompactionSummary),
        "/compact must rehydrate the CLI's in-memory context before accepting its next input"
    );
    assert!(
        context_state.usage_ratio() < before_ratio,
        "/compact 的验收不能只看 boundary 落盘，重载后的上下文占用必须严格下降: before={before_ratio}, after={}",
        context_state.usage_ratio()
    );

    match old_key {
        Some(value) => unsafe { std::env::set_var(API_KEY_ENV, value) },
        None => unsafe { std::env::remove_var(API_KEY_ENV) },
    }
}
