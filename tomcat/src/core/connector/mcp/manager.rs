use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::broadcast;
use tracing::warn;

use crate::core::connector::mcp::config::{load_servers, ConfiguredMcpServer, McpConfigSource};
use crate::core::connector::mcp::naming::to_model_name;
use crate::core::connector::mcp::transport::{McpClient, McpTransport, StdioTransport};
use crate::core::connector::mcp::trust::{TrustDecision, TrustStatus, TrustStore};
use crate::infra::error::AppError;
use crate::AppConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerState {
    Pending,
    Connecting,
    Ready,
    Disconnected,
    NeedsConfirmation,
    Blocked,
    Failed(String),
}

impl ServerState {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Connecting => "connecting",
            Self::Ready => "connected",
            Self::Disconnected => "disconnected",
            Self::NeedsConfirmation => "needs_confirmation",
            Self::Blocked => "blocked",
            Self::Failed(_) => "failed",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Pending => "等待连接",
            Self::Connecting => "连接中",
            Self::Ready => "已连接",
            Self::Disconnected => "已断开",
            Self::NeedsConfirmation => "待确认",
            Self::Blocked => "已阻止",
            Self::Failed(_) => "失败",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub name: String,
    pub source: McpConfigSource,
    pub state: ServerState,
    pub trust: TrustStatus,
    pub tool_count: usize,
    pub resource_count: usize,
}

#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub server: String,
    pub raw_name: String,
    pub model_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum ServerLifecycleEvent {
    Ready {
        server: String,
        tools: Vec<McpToolDef>,
    },
    NotReady {
        server: String,
    },
}

pub struct McpManager {
    servers: RwLock<BTreeMap<String, ConfiguredMcpServer>>,
    states: RwLock<BTreeMap<String, ServerStatus>>,
    connections: RwLock<BTreeMap<String, Arc<ConnectedServer>>>,
    trust: TrustStore,
    lifecycle_events: broadcast::Sender<ServerLifecycleEvent>,
    workspace_root: std::path::PathBuf,
}

struct ConnectedServer {
    client: McpClient,
    tools: BTreeMap<String, McpToolDef>,
    call_timeout: Duration,
    call_lock: tokio::sync::Mutex<()>,
    resource_count: usize,
}

impl McpManager {
    pub fn new(cfg: &AppConfig, workspace_root: &std::path::Path) -> Result<Arc<Self>, AppError> {
        let servers = match load_servers(cfg, workspace_root) {
            Ok(servers) => servers,
            Err(error) => {
                warn!(
                    error = %error,
                    "MCP configuration is invalid; continuing without configured MCP servers"
                );
                Vec::new()
            }
        };
        let trust = TrustStore::open(cfg)?;
        let states = servers
            .iter()
            .map(|server| (server.name.clone(), initial_server_status(server, &trust)))
            .collect();
        let (lifecycle_events, _) = broadcast::channel(32);
        Ok(Arc::new(Self {
            servers: RwLock::new(
                servers
                    .into_iter()
                    .map(|server| (server.name.clone(), server))
                    .collect(),
            ),
            states: RwLock::new(states),
            connections: RwLock::new(BTreeMap::new()),
            trust,
            lifecycle_events,
            workspace_root: workspace_root.to_path_buf(),
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerLifecycleEvent> {
        self.lifecycle_events.subscribe()
    }

    pub fn statuses(&self) -> Vec<ServerStatus> {
        self.states.read().values().cloned().collect()
    }

    pub fn configured_server(&self, server_name: &str) -> Option<ConfiguredMcpServer> {
        self.servers.read().get(server_name).cloned()
    }

    pub fn tool_defs(&self, server_name: &str) -> Vec<McpToolDef> {
        self.connections
            .read()
            .get(server_name)
            .map(|connection| connection.tools.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn approve(&self, server_name: &str) -> Result<(), AppError> {
        let server = self
            .configured_server(server_name)
            .ok_or_else(|| AppError::Tool(format!("unknown MCP server: {server_name}")))?;
        self.trust.approve(&server)?;
        self.refresh_trust_status(server_name, &server);
        Ok(())
    }

    pub fn deny(&self, server_name: &str) -> Result<(), AppError> {
        let server = self
            .configured_server(server_name)
            .ok_or_else(|| AppError::Tool(format!("unknown MCP server: {server_name}")))?;
        self.trust.deny(&server)?;
        self.refresh_trust_status(server_name, &server);
        self.connections.write().remove(server_name);
        self.update_state(server_name, ServerState::Blocked, 0);
        let _ = self.lifecycle_events.send(ServerLifecycleEvent::NotReady {
            server: server_name.to_string(),
        });
        Ok(())
    }

    pub fn reload_configuration(&self, cfg: &AppConfig) -> Result<Vec<String>, AppError> {
        let servers = load_servers(cfg, &self.workspace_root)?;
        let previous_names = self.states.read().keys().cloned().collect::<Vec<_>>();
        self.connections.write().clear();
        for server in previous_names {
            let _ = self
                .lifecycle_events
                .send(ServerLifecycleEvent::NotReady { server });
        }
        *self.servers.write() = servers
            .iter()
            .cloned()
            .map(|server| (server.name.clone(), server))
            .collect();
        *self.states.write() = servers
            .iter()
            .map(|server| {
                (
                    server.name.clone(),
                    initial_server_status(server, &self.trust),
                )
            })
            .collect();
        Ok(servers.into_iter().map(|server| server.name).collect())
    }

    pub async fn connect_server(&self, server_name: &str) -> Result<(), AppError> {
        let server = self
            .configured_server(server_name)
            .ok_or_else(|| AppError::Tool(format!("unknown MCP server: {server_name}")))?;
        if self.connections.read().contains_key(server_name) {
            return Ok(());
        }
        match self.trust.decide(&server)? {
            TrustDecision::Allowed => {}
            TrustDecision::NeedsConfirmation => {
                self.update_state(server_name, ServerState::NeedsConfirmation, 0);
                return Ok(());
            }
            TrustDecision::Blocked => {
                self.update_state(server_name, ServerState::Blocked, 0);
                return Ok(());
            }
        }
        self.refresh_trust_status(server_name, &server);

        self.update_state(server_name, ServerState::Connecting, 0);
        let startup_timeout = Duration::from_millis(server.config.startup_timeout_ms);
        let connected = match tokio::time::timeout(
            startup_timeout,
            ConnectedServer::connect(&server, &self.workspace_root),
        )
        .await
        {
            Ok(Ok(connection)) => Arc::new(connection),
            Ok(Err(error)) => {
                self.update_state(server_name, ServerState::Failed(error.to_string()), 0);
                let _ = self.lifecycle_events.send(ServerLifecycleEvent::NotReady {
                    server: server_name.to_string(),
                });
                return Err(error);
            }
            Err(_) => {
                let error = AppError::Tool(format!(
                    "MCP server '{server_name}' startup timed out after {} ms",
                    server.config.startup_timeout_ms
                ));
                self.update_state(server_name, ServerState::Failed(error.to_string()), 0);
                let _ = self.lifecycle_events.send(ServerLifecycleEvent::NotReady {
                    server: server_name.to_string(),
                });
                return Err(error);
            }
        };
        let tools = connected.tools.values().cloned().collect::<Vec<_>>();
        let resource_count = connected.resource_count;
        self.connections
            .write()
            .insert(server_name.to_string(), connected);
        self.update_state(server_name, ServerState::Ready, tools.len());
        if let Some(status) = self.states.write().get_mut(server_name) {
            status.resource_count = resource_count;
        }
        let _ = self.lifecycle_events.send(ServerLifecycleEvent::Ready {
            server: server_name.to_string(),
            tools,
        });
        Ok(())
    }

    pub async fn call_tool(
        &self,
        server_name: &str,
        model_tool_name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let connection = self
            .connections
            .read()
            .get(server_name)
            .cloned()
            .ok_or_else(|| {
                AppError::Tool(format!(
                    "MCP server '{server_name}' is not ready; use /connector list for status"
                ))
            })?;
        let tool = connection
            .tools
            .get(model_tool_name)
            .ok_or_else(|| AppError::Tool(format!("unknown MCP tool: {model_tool_name}")))?;
        let arguments = params.as_object().cloned().ok_or_else(|| {
            AppError::Tool(format!(
                "MCP tool '{model_tool_name}' arguments must be a JSON object"
            ))
        })?;
        let request = rmcp::model::CallToolRequestParams::new(tool.raw_name.clone())
            .with_arguments(arguments);
        let _call_guard = connection.call_lock.lock().await;
        let result = tokio::time::timeout(
            connection.call_timeout,
            connection.client.peer().call_tool(request),
        )
        .await
        .map_err(|_| AppError::Tool(format!("MCP tool '{model_tool_name}' timed out")))
        .and_then(|result| {
            result.map_err(|error| {
                AppError::Tool(format!("MCP tool '{model_tool_name}' failed: {error}"))
            })
        });
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.mark_disconnected(server_name);
                return Err(error);
            }
        };
        serde_json::to_value(result)
            .map_err(|error| AppError::Tool(format!("serialize MCP tool result: {error}")))
    }

    pub async fn reconnect_server(&self, server_name: &str) -> Result<(), AppError> {
        self.connections.write().remove(server_name);
        self.update_state(server_name, ServerState::Pending, 0);
        let _ = self.lifecycle_events.send(ServerLifecycleEvent::NotReady {
            server: server_name.to_string(),
        });
        self.connect_server(server_name).await
    }

    pub async fn connect_all(&self) {
        for status in self.statuses() {
            if let Err(error) = self.connect_server(&status.name).await {
                warn!(server = %status.name, error = %error, "MCP server did not become ready");
            }
        }
    }

    fn update_state(&self, server_name: &str, state: ServerState, tool_count: usize) {
        if let Some(status) = self.states.write().get_mut(server_name) {
            if !matches!(&state, ServerState::Ready) {
                status.resource_count = 0;
            }
            status.state = state;
            status.tool_count = tool_count;
        }
    }

    fn refresh_trust_status(&self, server_name: &str, server: &ConfiguredMcpServer) {
        match self.trust.inspect(server) {
            Ok(trust) => {
                if let Some(status) = self.states.write().get_mut(server_name) {
                    status.trust = trust;
                }
            }
            Err(error) => warn!(
                server = %server_name,
                error = %error,
                "failed to inspect MCP server trust status"
            ),
        }
    }

    fn mark_disconnected(&self, server_name: &str) {
        self.connections.write().remove(server_name);
        self.update_state(server_name, ServerState::Disconnected, 0);
        let _ = self.lifecycle_events.send(ServerLifecycleEvent::NotReady {
            server: server_name.to_string(),
        });
    }
}

fn initial_server_status(server: &ConfiguredMcpServer, trust: &TrustStore) -> ServerStatus {
    let trust = trust.inspect(server).unwrap_or_else(|error| {
        warn!(
            server = %server.name,
            error = %error,
            "failed to inspect MCP server trust status"
        );
        TrustStatus::Blocked
    });
    ServerStatus {
        name: server.name.clone(),
        source: server.source,
        state: ServerState::Pending,
        trust,
        tool_count: 0,
        resource_count: 0,
    }
}

impl ConnectedServer {
    async fn connect(
        server: &ConfiguredMcpServer,
        workspace_root: &std::path::Path,
    ) -> Result<Self, AppError> {
        let transport = StdioTransport::new(workspace_root);
        let client = transport.connect(server).await?;
        let listed_value = list_all_tools(&client, &server.name).await?;
        let tools = parse_tools(&server.name, &server.config.tool_filter, &listed_value)?;
        let resource_count = list_resource_count(&client).await;
        Ok(Self {
            client,
            tools,
            call_timeout: Duration::from_millis(server.config.call_timeout_ms),
            call_lock: tokio::sync::Mutex::new(()),
            resource_count,
        })
    }
}

async fn list_resource_count(client: &McpClient) -> usize {
    let resources_supported = client
        .peer()
        .peer_info()
        .and_then(|info| serde_json::to_value(info.capabilities.clone()).ok())
        .and_then(|capabilities| capabilities.get("resources").cloned())
        .is_some();
    if !resources_supported {
        return 0;
    }
    match client.peer().list_resources(None).await {
        Ok(resources) => serde_json::to_value(resources)
            .ok()
            .and_then(|value| {
                value
                    .get("resources")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
            })
            .map_or(0, |resources| resources.len()),
        Err(error) => {
            warn!(error = %error, "MCP server advertised resources but resources/list failed");
            0
        }
    }
}

async fn list_all_tools(
    client: &McpClient,
    server_name: &str,
) -> Result<serde_json::Value, AppError> {
    let mut cursor = None;
    let mut all_tools = Vec::new();
    for _ in 0..100 {
        let params = cursor
            .take()
            .map(|cursor| rmcp::model::PaginatedRequestParams::default().with_cursor(Some(cursor)));
        let listed =
            client.peer().list_tools(params).await.map_err(|error| {
                AppError::Tool(format!("list MCP tools '{server_name}': {error}"))
            })?;
        let listed_value = serde_json::to_value(listed)
            .map_err(|error| AppError::Tool(format!("serialize MCP tool list: {error}")))?;
        let tools = listed_value
            .get("tools")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                AppError::Tool(format!(
                    "MCP server '{server_name}' returned invalid tools/list"
                ))
            })?;
        all_tools.extend(tools.iter().cloned());
        cursor = listed_value
            .get("nextCursor")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        if cursor.is_none() {
            return Ok(serde_json::json!({ "tools": all_tools }));
        }
    }
    Err(AppError::Tool(format!(
        "MCP server '{server_name}' returned more than 100 pages of tools"
    )))
}

fn parse_tools(
    server: &str,
    filter: &crate::core::connector::mcp::config::ToolFilter,
    listed: &serde_json::Value,
) -> Result<BTreeMap<String, McpToolDef>, AppError> {
    let include = build_glob_set(&filter.include)?;
    let exclude = build_glob_set(&filter.exclude)?;
    let tools = listed
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            AppError::Tool(format!("MCP server '{server}' returned invalid tools/list"))
        })?;
    let mut definitions = BTreeMap::new();
    for tool in tools {
        let raw_name = tool
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                AppError::Tool(format!(
                    "MCP server '{server}' returned a tool without name"
                ))
            })?;
        if !filter.include.is_empty() && !include.is_match(raw_name) || exclude.is_match(raw_name) {
            continue;
        }
        let model_name = to_model_name(server, raw_name);
        definitions.insert(
            model_name.clone(),
            McpToolDef {
                server: server.to_string(),
                raw_name: raw_name.to_string(),
                model_name,
                description: model_description(
                    server,
                    raw_name,
                    tool.get("description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                ),
                input_schema: tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object"})),
            },
        );
    }
    Ok(definitions)
}

fn model_description(server: &str, raw_name: &str, description: &str) -> String {
    if server == "playwright" && raw_name == "browser_take_screenshot" {
        format!(
            "{description}\n\nFor visual inspection by the model: omit filename and omit fullPage=true. The current Playwright MCP server saves named/full-page screenshots to disk and returns only a file link, not an image content block."
        )
    } else {
        description.to_string()
    }
}

fn build_glob_set(patterns: &[String]) -> Result<globset::GlobSet, AppError> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(globset::Glob::new(pattern).map_err(|error| {
            AppError::Config(format!("invalid MCP tool filter '{pattern}': {error}"))
        })?);
    }
    builder
        .build()
        .map_err(|error| AppError::Config(format!("build MCP tool filter: {error}")))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{McpManager, ServerState};
    use crate::infra::config::get_work_dir;
    use crate::AppConfig;

    fn manager_with_fake_server(
        args: Vec<String>,
        call_timeout_ms: u64,
    ) -> (tempfile::TempDir, Arc<McpManager>) {
        let temp = tempfile::tempdir().expect("temporary directory");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(temp.path().join("work").to_string_lossy().into_owned());
        let config_path = get_work_dir(&cfg).expect("work dir").join("mcp.json");
        std::fs::create_dir_all(config_path.parent().expect("config parent"))
            .expect("config directory");
        std::fs::write(
            config_path,
            serde_json::json!({
                "mcpServers": {
                    "fake": {
                        "command": "node",
                        "args": args,
                        "callTimeoutMs": call_timeout_ms,
                    }
                }
            })
            .to_string(),
        )
        .expect("write MCP config");
        let manager = McpManager::new(&cfg, &workspace).expect("construct MCP manager");
        (temp, manager)
    }

    fn fake_server_args(extra: &[String]) -> Vec<String> {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mcp/fake_stdio_server.mjs");
        std::iter::once(fixture.to_string_lossy().into_owned())
            .chain(extra.iter().cloned())
            .collect()
    }

    #[tokio::test]
    async fn fake_stdio_server_lists_and_calls_tools() {
        let (_temp, manager) = manager_with_fake_server(fake_server_args(&[]), 120_000);
        manager
            .connect_server("fake")
            .await
            .expect("connect fake server");
        let status = manager.statuses().pop().expect("server status");
        assert_eq!(status.state, ServerState::Ready);
        assert_eq!(status.tool_count, 2);
        let tool = manager
            .tool_defs("fake")
            .into_iter()
            .find(|tool| tool.raw_name == "capture")
            .expect("capture tool");
        assert_eq!(tool.model_name, "mcp__fake__capture");
        let result = manager
            .call_tool("fake", &tool.model_name, serde_json::json!({}))
            .await
            .expect("call fake tool");
        assert_eq!(result["content"][0]["text"], "fake capture complete");
        assert_eq!(result["content"][1]["type"], "image");
    }

    #[tokio::test]
    async fn call_tool_timeout_returns_error_and_marks_server_disconnected() {
        let (_temp, manager) =
            manager_with_fake_server(fake_server_args(&["--hang".to_string()]), 25);
        manager
            .connect_server("fake")
            .await
            .expect("connect fake server");
        let tool = manager.tool_defs("fake").pop().expect("discovered tool");

        let error = manager
            .call_tool("fake", &tool.model_name, serde_json::json!({}))
            .await
            .expect_err("hanging MCP tool should time out");

        assert!(error.to_string().contains("timed out"));
        assert!(matches!(
            manager.statuses().pop().expect("server status").state,
            ServerState::Disconnected
        ));
    }

    #[tokio::test]
    async fn transport_drop_marks_disconnected_without_replaying_call() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let call_log = temp.path().join("calls.log");
        let (_manager_temp, manager) = manager_with_fake_server(
            fake_server_args(&[
                "--die-midcall".to_string(),
                "--record".to_string(),
                call_log.to_string_lossy().into_owned(),
            ]),
            1_000,
        );
        manager
            .connect_server("fake")
            .await
            .expect("connect fake server");
        let tool = manager.tool_defs("fake").pop().expect("discovered tool");

        manager
            .call_tool("fake", &tool.model_name, serde_json::json!({}))
            .await
            .expect_err("connection drop should fail the in-flight call");

        let methods = std::fs::read_to_string(&call_log).expect("read call log");
        assert_eq!(
            methods
                .lines()
                .filter(|method| *method == "tools/call")
                .count(),
            1,
            "the in-flight call must not be replayed"
        );
        assert!(matches!(
            manager.statuses().pop().expect("server status").state,
            ServerState::Disconnected
        ));
    }

    #[tokio::test]
    async fn reconnect_refetches_tools_without_a_persistent_cache() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let request_log = temp.path().join("requests.log");
        let (_manager_temp, manager) = manager_with_fake_server(
            fake_server_args(&[
                "--record".to_string(),
                request_log.to_string_lossy().into_owned(),
            ]),
            1_000,
        );
        manager
            .connect_server("fake")
            .await
            .expect("initial connection");
        manager
            .reconnect_server("fake")
            .await
            .expect("reconnection");

        let methods = std::fs::read_to_string(&request_log).expect("read request log");
        assert_eq!(
            methods
                .lines()
                .filter(|method| *method == "tools/list")
                .count(),
            2,
            "each connection must refresh its tool catalog"
        );
    }
}
