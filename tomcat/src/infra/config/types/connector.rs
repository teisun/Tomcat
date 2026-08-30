use serde::{Deserialize, Serialize};

/// External connector subsystem. It starts no process until an MCP server is
/// configured; the switch remains available for one-step global disablement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectorConfig {
    pub enabled: bool,
    pub disabled: Vec<String>,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            disabled: Vec::new(),
        }
    }
}
