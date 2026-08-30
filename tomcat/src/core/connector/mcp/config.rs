use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::infra::config::get_work_dir;
use crate::infra::error::AppError;
use crate::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigSource {
    Global,
    Project,
}

impl McpConfigSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "User",
            Self::Project => "Workspace",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolFilter {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub integrity: Option<String>,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_call_timeout_ms")]
    pub call_timeout_ms: u64,
    #[serde(default)]
    pub tool_filter: ToolFilter,
}

impl McpServerConfig {
    pub fn validate(&self, server_name: &str) -> Result<(), AppError> {
        if server_name.trim().is_empty() {
            return Err(AppError::Config(
                "MCP server name cannot be empty".to_string(),
            ));
        }
        if self.command.trim().is_empty() {
            return Err(AppError::Config(format!(
                "MCP server '{server_name}' command cannot be empty"
            )));
        }
        if self.startup_timeout_ms == 0 || self.call_timeout_ms == 0 {
            return Err(AppError::Config(format!(
                "MCP server '{server_name}' timeouts must be positive"
            )));
        }
        Ok(())
    }

    pub fn normalized_cwd(&self, workspace_root: &Path) -> PathBuf {
        match self.cwd.as_deref() {
            Some(path) if path.is_absolute() => path.to_path_buf(),
            Some(path) => workspace_root.join(path),
            None => workspace_root.to_path_buf(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfiguredMcpServer {
    pub name: String,
    pub config: McpServerConfig,
    pub source: McpConfigSource,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpFile {
    #[serde(default)]
    mcp_servers: BTreeMap<String, McpServerConfig>,
}

pub fn global_mcp_path(cfg: &AppConfig) -> Result<PathBuf, AppError> {
    Ok(get_work_dir(cfg)?.join("mcp.json"))
}

pub fn project_mcp_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".tomcat").join("mcp.json")
}

pub fn load_servers(
    cfg: &AppConfig,
    workspace_root: &Path,
) -> Result<Vec<ConfiguredMcpServer>, AppError> {
    let mut merged = read_mcp_file(&global_mcp_path(cfg)?)?
        .mcp_servers
        .into_iter()
        .map(|(name, config)| {
            (
                name,
                ConfiguredMcpServer {
                    name: String::new(),
                    config,
                    source: McpConfigSource::Global,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (name, config) in read_mcp_file(&project_mcp_path(workspace_root))?.mcp_servers {
        merged.insert(
            name,
            ConfiguredMcpServer {
                name: String::new(),
                config,
                source: McpConfigSource::Project,
            },
        );
    }

    merged
        .into_iter()
        .filter(|(name, _)| {
            !cfg.connector
                .disabled
                .iter()
                .any(|disabled| disabled == name)
        })
        .map(|(name, mut server)| {
            server.config.validate(&name)?;
            server.name = name;
            Ok(server)
        })
        .collect()
}

pub fn upsert_global_server(
    cfg: &AppConfig,
    name: String,
    server: McpServerConfig,
) -> Result<(), AppError> {
    server.validate(&name)?;
    let path = global_mcp_path(cfg)?;
    let mut file = read_mcp_file(&path)?;
    file.mcp_servers.insert(name, server);
    write_mcp_file(&path, &file)
}

pub fn remove_global_server(cfg: &AppConfig, name: &str) -> Result<bool, AppError> {
    let path = global_mcp_path(cfg)?;
    let mut file = read_mcp_file(&path)?;
    let removed = file.mcp_servers.remove(name).is_some();
    if removed {
        write_mcp_file(&path, &file)?;
    }
    Ok(removed)
}

pub fn set_global_tool_filter(
    cfg: &AppConfig,
    name: &str,
    tool_filter: ToolFilter,
) -> Result<(), AppError> {
    let path = global_mcp_path(cfg)?;
    let mut file = read_mcp_file(&path)?;
    let server = file
        .mcp_servers
        .get_mut(name)
        .ok_or_else(|| AppError::Tool(format!("unknown global MCP server: {name}")))?;
    server.tool_filter = tool_filter;
    write_mcp_file(&path, &file)
}

fn read_mcp_file(path: &Path) -> Result<McpFile, AppError> {
    if !path.exists() {
        return Ok(McpFile::default());
    }
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|error| {
        AppError::Config(format!(
            "parse MCP configuration '{}': {error}",
            path.display()
        ))
    })
}

fn write_mcp_file(path: &Path, file: &McpFile) -> Result<(), AppError> {
    let contents = serde_json::to_vec_pretty(file)
        .map_err(|error| AppError::Config(format!("serialize MCP configuration: {error}")))?;
    crate::infra::platform::write_file_atomic(path, &contents)
}

const fn default_startup_timeout_ms() -> u64 {
    30_000
}

const fn default_call_timeout_ms() -> u64 {
    120_000
}

#[cfg(test)]
mod tests {
    use super::{load_servers, project_mcp_path, McpConfigSource};
    use crate::infra::config::get_work_dir;
    use crate::AppConfig;

    #[test]
    fn parses_minimal_cursor_style_command_and_args() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(temp.path().join("work").to_string_lossy().into_owned());
        let global = get_work_dir(&cfg).expect("work dir").join("mcp.json");
        std::fs::create_dir_all(global.parent().expect("global parent")).expect("global parent");
        std::fs::write(
            global,
            r#"{"mcpServers":{"browser":{"command":"npx","args":["-y","browser-mcp@1.2.3"]}}}"#,
        )
        .expect("write config");

        let servers = load_servers(&cfg, &workspace).expect("parse minimal MCP config");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "browser");
        assert_eq!(servers[0].config.command, "npx");
        assert_eq!(servers[0].config.args, ["-y", "browser-mcp@1.2.3"]);
        assert_eq!(servers[0].source, McpConfigSource::Global);
    }

    #[test]
    fn project_server_overrides_global_server_with_same_name() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(workspace.join(".tomcat")).expect("project config directory");
        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(temp.path().join("work").to_string_lossy().into_owned());
        let global = get_work_dir(&cfg).expect("work dir").join("mcp.json");
        std::fs::create_dir_all(global.parent().expect("global parent")).expect("global parent");
        std::fs::write(
            global,
            r#"{"mcpServers":{"same":{"command":"global","args":[]}}}"#,
        )
        .expect("write global config");
        std::fs::write(
            project_mcp_path(&workspace),
            r#"{"mcpServers":{"same":{"command":"project","args":[]}}}"#,
        )
        .expect("write project config");

        let servers = load_servers(&cfg, &workspace).expect("load merged config");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].config.command, "project");
        assert_eq!(servers[0].source, McpConfigSource::Project);
    }
}
