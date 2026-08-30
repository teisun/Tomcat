//! Black-box integration coverage for the stdio MCP connector.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serial_test::serial;
use tokio_util::sync::CancellationToken;
use tomcat::api::chat::ChatContextOverrides;
use tomcat::{
    init_context_state, run_chat_turn, AppConfig, AppError, Capabilities, ChatContext, ChatMessage,
    ChatRequest, ChatResponse, LlmProvider, LlmResolver, LlmScene, ResolvedCall, StreamEvent,
    ToolRegistry,
};

struct EnvGuard {
    key: &'static str,
    old: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn fixture_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mcp/fake_stdio_server.mjs")
        .to_string_lossy()
        .into_owned()
}

fn write_mcp_config(path: &std::path::Path, source: serde_json::Value) {
    std::fs::create_dir_all(path.parent().expect("MCP config parent")).expect("MCP config parent");
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&source).expect("serialize MCP config"),
    )
    .expect("write MCP config");
}

fn fake_mcp_config(name: &str) -> serde_json::Value {
    serde_json::json!({
        "mcpServers": {
            name: {
                "command": "node",
                "args": [fixture_path()],
            }
        }
    })
}

fn connector_context(
    temp: &tempfile::TempDir,
    workspace: std::path::PathBuf,
    api_key_env: &str,
) -> ChatContext {
    let work_dir = temp.path().join("work");
    std::fs::create_dir_all(&work_dir).expect("create test work directory");
    std::fs::write(
        work_dir.join("models.toml"),
        format!(
            r#"[[models]]
id = "connector-test"
model_name = "connector-test"
api = "openai-responses"
provider = "connector-test"
api_key_env = "{api_key_env}"
base_url = "http://127.0.0.1:9"
capabilities = {{ vision = true, files = true, tools = true, reasoning = true, web_search = false }}
"#
        ),
    )
    .expect("write test models.toml");

    let mut cfg = AppConfig::default();
    cfg.connector.enabled = true;
    cfg.skills.enabled = false;
    cfg.llm.default_model = "connector-test".to_string();
    cfg.storage.work_dir = Some(work_dir.to_string_lossy().into_owned());
    ChatContext::from_config_with_overrides(
        cfg,
        ChatContextOverrides::default().with_session_cwd_override(workspace),
    )
    .expect("chat context")
}

async fn start_connectors(ctx: &ChatContext) {
    let connectors = ctx
        .global_services
        .connector_registry
        .as_ref()
        .expect("connector module enabled")
        .clone();
    connectors
        .spawn_connect_all(Arc::downgrade(&ctx.global_services.tool_registry))
        .await;
}

async fn wait_for_tool(registry: &Arc<dyn ToolRegistry>, name: &str, present: bool) {
    for _ in 0..50 {
        if registry.get_tool(name).await.is_ok() == present {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!(
        "MCP tool '{name}' did not become {}",
        if present { "available" } else { "unavailable" }
    );
}

#[tokio::test]
#[serial(env_lock)]
async fn pending_confirm_project_server_is_absent_until_confirmed() {
    const API_KEY_ENV: &str = "TOMCAT_CONNECTOR_PROJECT_TEST_KEY";

    let temp = tempfile::tempdir().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    let _api_key = EnvGuard::set(API_KEY_ENV, "test-key");
    write_mcp_config(
        &workspace.join(".tomcat").join("mcp.json"),
        fake_mcp_config("project-fake"),
    );
    let ctx = connector_context(&temp, workspace, API_KEY_ENV);
    start_connectors(&ctx).await;

    wait_for_tool(
        &ctx.global_services.tool_registry,
        "mcp__project-fake__capture",
        false,
    )
    .await;
    let connectors = ctx
        .global_services
        .connector_registry
        .as_ref()
        .expect("connector module enabled");
    connectors
        .approve_and_connect("project-fake")
        .await
        .expect("approve project MCP server");
    wait_for_tool(
        &ctx.global_services.tool_registry,
        "mcp__project-fake__capture",
        true,
    )
    .await;
}

#[tokio::test]
#[serial(env_lock)]
async fn pasted_cursor_style_mcp_snippet_connects_without_translation() {
    const API_KEY_ENV: &str = "TOMCAT_CONNECTOR_GLOBAL_TEST_KEY";

    let temp = tempfile::tempdir().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    let _api_key = EnvGuard::set(API_KEY_ENV, "test-key");
    write_mcp_config(
        &temp.path().join("work").join("mcp.json"),
        fake_mcp_config("pasted"),
    );
    let ctx = connector_context(&temp, workspace, API_KEY_ENV);
    start_connectors(&ctx).await;

    wait_for_tool(
        &ctx.global_services.tool_registry,
        "mcp__pasted__capture",
        true,
    )
    .await;
}

struct RecordingMockLlm {
    streams: Mutex<VecDeque<Vec<Result<StreamEvent, AppError>>>>,
    requests: Arc<Mutex<Vec<ChatRequest>>>,
}

impl RecordingMockLlm {
    fn new(streams: Vec<Vec<Result<StreamEvent, AppError>>>) -> Self {
        Self {
            streams: Mutex::new(streams.into()),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LlmProvider for RecordingMockLlm {
    fn provider_name(&self) -> &str {
        "connector-mock"
    }

    async fn chat(&self, _req: ChatRequest) -> Result<ChatResponse, AppError> {
        Err(AppError::Llm("mock chat is not used".to_string()))
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> Result<
        Box<dyn tokio_stream::Stream<Item = Result<StreamEvent, AppError>> + Send + Unpin>,
        AppError,
    > {
        self.requests.lock().expect("requests lock").push(req);
        let stream = self
            .streams
            .lock()
            .expect("streams lock")
            .pop_front()
            .ok_or_else(|| AppError::Llm("unexpected mock request".to_string()))?;
        Ok(Box::new(tokio_stream::iter(stream)))
    }

    fn count_tokens(&self, _messages: &[ChatMessage]) -> Result<u32, AppError> {
        Ok(0)
    }
}

struct FixedResolver {
    provider: Arc<dyn LlmProvider>,
}

impl LlmResolver for FixedResolver {
    fn resolve(
        &self,
        _scene: LlmScene,
        _session_override: Option<&str>,
    ) -> Result<ResolvedCall, AppError> {
        let mut call =
            ResolvedCall::from_parts_unchecked(self.provider.clone(), "gpt-5.4", "gpt-5.4");
        call.api = "openai-responses".to_string();
        call.provider = "openai".to_string();
        call.capabilities = Capabilities {
            vision: true,
            files: true,
            tools: true,
            reasoning: true,
            web_search: false,
        };
        Ok(call)
    }
}

#[tokio::test]
#[serial(env_lock)]
async fn fake_stdio_server_end_to_end_image_reflow() {
    const API_KEY_ENV: &str = "TOMCAT_CONNECTOR_IMAGE_TEST_KEY";

    let temp = tempfile::tempdir().expect("temporary directory");
    let workspace = temp.path().join("workspace");
    let _api_key = EnvGuard::set(API_KEY_ENV, "test-key");
    write_mcp_config(
        &temp.path().join("work").join("mcp.json"),
        fake_mcp_config("fake"),
    );
    let mut ctx = connector_context(&temp, workspace, API_KEY_ENV);
    start_connectors(&ctx).await;
    wait_for_tool(
        &ctx.global_services.tool_registry,
        "mcp__fake__capture",
        true,
    )
    .await;

    let provider = Arc::new(RecordingMockLlm::new(vec![
        vec![
            Ok(StreamEvent::ToolCallDelta {
                index: 0,
                id: Some("capture-1".to_string()),
                name: Some("mcp__fake__capture".to_string()),
                arguments_delta: Some("{}".to_string()),
            }),
            Ok(StreamEvent::FinishReason {
                reason: "tool_calls".to_string(),
            }),
        ],
        vec![
            Ok(StreamEvent::ContentDelta {
                delta: "capture accepted".to_string(),
            }),
            Ok(StreamEvent::FinishReason {
                reason: "stop".to_string(),
            }),
        ],
    ]));
    let requests = provider.requests.clone();
    ctx.global_services.llm_resolver = Arc::new(FixedResolver {
        provider: provider.clone(),
    });

    let system_prompt = "test system prompt";
    let mut context_state = init_context_state(
        &ctx.session_runtime.session,
        &ctx.config.context,
        system_prompt,
    )
    .expect("context state");
    let outcome = run_chat_turn(
        &ctx,
        "capture an image",
        system_prompt,
        &mut context_state,
        CancellationToken::new(),
    )
    .await
    .expect("tool turn");
    assert!(!outcome.is_interrupted());

    let reflowed_image = requests
        .lock()
        .expect("requests lock")
        .iter()
        .flat_map(|request| request.messages.iter())
        .any(|message| {
            serde_json::to_string(message)
                .expect("serialize message")
                .contains(r#""type":"input_image""#)
        });
    assert!(
        reflowed_image,
        "MCP image result must become an InputImage in the next model request"
    );
}
