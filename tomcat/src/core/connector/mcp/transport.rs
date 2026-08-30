use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;

use crate::core::connector::mcp::config::ConfiguredMcpServer;
use crate::infra::error::AppError;

pub type McpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn connect(&self, server: &ConfiguredMcpServer) -> Result<McpClient, AppError>;
}

/// Client-side stdio transport. Only PATH and HOME survive env_clear; configured
/// environment variables are added explicitly, so an MCP configuration cannot
/// silently inherit unrelated process secrets.
pub struct StdioTransport {
    workspace_root: PathBuf,
}

impl StdioTransport {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    fn command(&self, server: &ConfiguredMcpServer) -> tokio::process::Command {
        tokio::process::Command::new(&server.config.command).configure(|command| {
            command.args(&server.config.args);
            command.current_dir(server.config.normalized_cwd(&self.workspace_root));
            command.env_clear();
            for key in ["PATH", "HOME"] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
            command.envs(&server.config.env);
            if let Some(executable_path) = managed_playwright_executable(server) {
                command.env("PLAYWRIGHT_MCP_EXECUTABLE_PATH", executable_path);
            }
        })
    }
}

/// Phase 1's bootstrap records the system Chrome fallback here when Playwright
/// cannot download a bundled Chromium (notably macOS 13). `@playwright/mcp` is
/// a separate process and cannot discover that marker itself, so the curated
/// Playwright connector bridges it through the MCP server's documented env var.
fn managed_playwright_executable(server: &ConfiguredMcpServer) -> Option<PathBuf> {
    if server.name != "playwright"
        || server
            .config
            .env
            .contains_key("PLAYWRIGHT_MCP_EXECUTABLE_PATH")
    {
        return None;
    }
    let browser_root = server.config.env.get("PLAYWRIGHT_BROWSERS_PATH")?;
    let marker_path = Path::new(browser_root).join("system-browser.json");
    let marker = std::fs::read_to_string(marker_path).ok()?;
    let marker = serde_json::from_str::<serde_json::Value>(&marker).ok()?;
    let executable_path = marker.get("executablePath")?.as_str()?;
    let executable_path = PathBuf::from(executable_path);
    executable_path.is_file().then_some(executable_path)
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&self, server: &ConfiguredMcpServer) -> Result<McpClient, AppError> {
        let transport = TokioChildProcess::new(self.command(server)).map_err(|error| {
            AppError::Tool(format!("spawn MCP server '{}': {error}", server.name))
        })?;
        ().serve(transport).await.map_err(|error| {
            AppError::Tool(format!("initialize MCP server '{}': {error}", server.name))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::StdioTransport;
    use crate::core::connector::mcp::config::{
        ConfiguredMcpServer, McpConfigSource, McpServerConfig, ToolFilter,
    };

    #[test]
    fn configured_env_is_added_after_environment_is_cleared() {
        let transport = StdioTransport::new("/workspace");
        let server = ConfiguredMcpServer {
            name: "test".to_string(),
            source: McpConfigSource::Global,
            config: McpServerConfig {
                command: "echo".to_string(),
                args: Vec::new(),
                env: [("EXPLICIT".to_string(), "value".to_string())].into(),
                cwd: None,
                trusted: false,
                integrity: None,
                startup_timeout_ms: 30_000,
                call_timeout_ms: 120_000,
                tool_filter: ToolFilter::default(),
            },
        };
        let command = transport.command(&server);
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(Path::new("/workspace"))
        );
    }

    #[test]
    fn curated_playwright_passes_bootstrap_system_browser_fallback_to_mcp() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let browser_root = temp.path().join("browser-cache");
        std::fs::create_dir_all(&browser_root).expect("browser cache");
        let executable = temp.path().join("chrome");
        std::fs::write(&executable, "").expect("system browser fixture");
        std::fs::write(
            browser_root.join("system-browser.json"),
            serde_json::json!({ "executablePath": executable }).to_string(),
        )
        .expect("fallback marker");
        let transport = StdioTransport::new("/workspace");
        let server = ConfiguredMcpServer {
            name: "playwright".to_string(),
            source: McpConfigSource::Global,
            config: McpServerConfig {
                command: "npx".to_string(),
                args: Vec::new(),
                env: [(
                    "PLAYWRIGHT_BROWSERS_PATH".to_string(),
                    browser_root.to_string_lossy().into_owned(),
                )]
                .into(),
                cwd: None,
                trusted: false,
                integrity: None,
                startup_timeout_ms: 30_000,
                call_timeout_ms: 120_000,
                tool_filter: ToolFilter::default(),
            },
        };

        let command = transport.command(&server);
        assert!(
            command.as_std().get_envs().any(|(key, value)| {
                key == "PLAYWRIGHT_MCP_EXECUTABLE_PATH" && value == Some(executable.as_os_str())
            }),
            "curated MCP must receive Phase 1's fallback browser path"
        );
    }
}
