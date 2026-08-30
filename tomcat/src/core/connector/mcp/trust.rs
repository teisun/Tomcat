use std::collections::BTreeMap;
use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};

use crate::core::connector::mcp::config::{ConfiguredMcpServer, McpConfigSource};
use crate::infra::config::get_work_dir;
use crate::infra::error::AppError;
use crate::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustDecision {
    Allowed,
    NeedsConfirmation,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustRecord {
    command_fingerprint: String,
    #[serde(default)]
    denied: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrustFile {
    #[serde(default)]
    servers: BTreeMap<String, TrustRecord>,
}

pub struct TrustStore {
    path: PathBuf,
    state: Mutex<TrustFile>,
}

impl TrustStore {
    pub fn open(cfg: &AppConfig) -> Result<Self, AppError> {
        let path = get_work_dir(cfg)?.join("connector-trust.json");
        let state = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content).map_err(|error| {
                AppError::Config(format!(
                    "parse connector trust store '{}': {error}",
                    path.display()
                ))
            })?
        } else {
            TrustFile::default()
        };
        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    pub fn decide(&self, server: &ConfiguredMcpServer) -> Result<TrustDecision, AppError> {
        if !optional_integrity_matches(server)? {
            return Ok(TrustDecision::Blocked);
        }
        let fingerprint = command_fingerprint(server);
        let mut state = self.state.lock();
        match state.servers.get(&server.name) {
            Some(record) if record.command_fingerprint == fingerprint && record.denied => {
                Ok(TrustDecision::Blocked)
            }
            Some(record) if record.command_fingerprint == fingerprint => Ok(TrustDecision::Allowed),
            Some(_) => Ok(TrustDecision::NeedsConfirmation),
            None if server.source == McpConfigSource::Global || server.config.trusted => {
                state.servers.insert(
                    server.name.clone(),
                    TrustRecord {
                        command_fingerprint: fingerprint,
                        denied: false,
                    },
                );
                persist(&self.path, &state)?;
                Ok(TrustDecision::Allowed)
            }
            None => Ok(TrustDecision::NeedsConfirmation),
        }
    }

    pub fn approve(&self, server: &ConfiguredMcpServer) -> Result<(), AppError> {
        let mut state = self.state.lock();
        state.servers.insert(
            server.name.clone(),
            TrustRecord {
                command_fingerprint: command_fingerprint(server),
                denied: false,
            },
        );
        persist(&self.path, &state)
    }

    pub fn deny(&self, server: &ConfiguredMcpServer) -> Result<(), AppError> {
        let mut state = self.state.lock();
        state.servers.insert(
            server.name.clone(),
            TrustRecord {
                command_fingerprint: command_fingerprint(server),
                denied: true,
            },
        );
        persist(&self.path, &state)
    }
}

pub fn command_fingerprint(server: &ConfiguredMcpServer) -> String {
    let payload = serde_json::json!({
        "command": server.config.command,
        "args": server.config.args,
        "env": server.config.env,
        "cwd": server.config.cwd,
    });
    let encoded = serde_json::to_vec(&payload).expect("command fingerprint JSON serialization");
    let digest = Sha256::digest(encoded);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn optional_integrity_matches(server: &ConfiguredMcpServer) -> Result<bool, AppError> {
    let Some(expected) = server.config.integrity.as_deref() else {
        return Ok(true);
    };
    let artifact = integrity_artifact(server).ok_or_else(|| {
        AppError::Config(format!(
            "MCP server '{}' configured integrity, but command/args do not reference a local preinstalled executable",
            server.name
        ))
    })?;
    let bytes = std::fs::read(&artifact).map_err(|error| {
        AppError::Config(format!(
            "read MCP integrity artifact '{}': {error}",
            artifact.display()
        ))
    })?;
    if let Some(expected_hex) = expected.strip_prefix("sha256:") {
        return Ok(hex_sha256(&bytes) == expected_hex.to_ascii_lowercase());
    }
    if let Some(expected_base64) = expected.strip_prefix("sha256-") {
        return Ok(base64_digest::<Sha256>(&bytes) == expected_base64);
    }
    if let Some(expected_base64) = expected.strip_prefix("sha512-") {
        return Ok(base64_digest::<Sha512>(&bytes) == expected_base64);
    }
    Err(AppError::Config(format!(
        "MCP server '{}' integrity must use sha256:<hex>, sha256-<base64>, or sha512-<base64>",
        server.name
    )))
}

fn integrity_artifact(server: &ConfiguredMcpServer) -> Option<std::path::PathBuf> {
    let cwd = server
        .config
        .cwd
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::iter::once(server.config.command.as_str())
        .chain(server.config.args.iter().map(String::as_str))
        .map(std::path::PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .find(|path| path.is_file())
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn base64_digest<D: Digest>(bytes: &[u8]) -> String {
    use base64::Engine;

    base64::engine::general_purpose::STANDARD.encode(D::digest(bytes))
}

fn persist(path: &std::path::Path, state: &TrustFile) -> Result<(), AppError> {
    let contents = serde_json::to_vec_pretty(state)
        .map_err(|error| AppError::Config(format!("serialize connector trust store: {error}")))?;
    crate::infra::platform::write_file_atomic(path, &contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TrustDecision, TrustStore};
    use crate::core::connector::mcp::config::{
        ConfiguredMcpServer, McpConfigSource, McpServerConfig, ToolFilter,
    };
    use crate::AppConfig;

    fn server(source: McpConfigSource, command: &str) -> ConfiguredMcpServer {
        ConfiguredMcpServer {
            name: "browser".to_string(),
            config: McpServerConfig {
                command: command.to_string(),
                args: vec!["-y".to_string(), "browser-mcp".to_string()],
                env: Default::default(),
                cwd: None,
                trusted: false,
                integrity: None,
                startup_timeout_ms: 30_000,
                call_timeout_ms: 120_000,
                tool_filter: ToolFilter::default(),
            },
            source,
        }
    }

    #[test]
    fn global_config_is_auto_trusted_but_command_change_requires_confirmation() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(temp.path().join("work").to_string_lossy().into_owned());
        let store = TrustStore::open(&cfg).expect("open trust store");
        let configured = server(McpConfigSource::Global, "npx");

        assert_eq!(
            store.decide(&configured).expect("auto trust"),
            TrustDecision::Allowed
        );
        assert_eq!(
            store.decide(&configured).expect("remembered trust"),
            TrustDecision::Allowed
        );
        assert_eq!(
            store
                .decide(&server(McpConfigSource::Global, "node"))
                .expect("changed command"),
            TrustDecision::NeedsConfirmation
        );
    }

    #[test]
    fn project_config_requires_one_explicit_approval() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(temp.path().join("work").to_string_lossy().into_owned());
        let store = TrustStore::open(&cfg).expect("open trust store");
        let configured = server(McpConfigSource::Project, "npx");

        assert_eq!(
            store.decide(&configured).expect("evaluate project config"),
            TrustDecision::NeedsConfirmation
        );
        store.approve(&configured).expect("approve project config");
        assert_eq!(
            store
                .decide(&configured)
                .expect("evaluate approved project config"),
            TrustDecision::Allowed
        );
    }

    #[test]
    fn configured_integrity_blocks_a_tampered_local_launcher() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let launcher = temp.path().join("launcher");
        std::fs::write(&launcher, "expected launcher bytes").expect("write launcher");
        let mut cfg = AppConfig::default();
        cfg.storage.work_dir = Some(temp.path().join("work").to_string_lossy().into_owned());
        let store = TrustStore::open(&cfg).expect("open trust store");
        let mut configured = server(McpConfigSource::Global, launcher.to_str().expect("UTF-8"));
        configured.config.integrity = Some("sha256:deadbeef".to_string());

        assert_eq!(
            store
                .decide(&configured)
                .expect("evaluate tampered launcher"),
            TrustDecision::Blocked
        );
    }
}
