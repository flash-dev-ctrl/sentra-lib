use crate::SentraResult;
use crate::agents::install_status::hidden_home_parent;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{parse_mcp_servers, read_json_file};

#[derive(Debug, Clone)]
pub(super) struct McpAsset {
    core: AssetCore,
}

impl McpAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(McpAsset, AssetType::Mcp, Vec<McpData>);

impl Asset<Vec<McpData>> for McpAsset {
    fn get_data(&self) -> SentraResult<Vec<McpData>> {
        let home = self.core.agent_home();
        let user_home = hidden_home_parent(home);
        let mut results = Vec::new();
        for path in [home.join("mcp.json"), user_home.join(".claude.json")] {
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            results.extend(parse_mcp_servers(
                config.get("mcpServers").unwrap_or(&serde_json::Value::Null),
                None,
            ));
            if let Some(projects) = config.get("projects").and_then(|value| value.as_object()) {
                for (project, value) in projects {
                    results.extend(parse_mcp_servers(
                        value.get("mcpServers").unwrap_or(&serde_json::Value::Null),
                        Some(project.clone()),
                    ));
                }
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_lingcode_and_root_claude_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".lingcode");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("mcp.json"),
            r#"{"mcpServers":{"lingcode":{"command":"lingcode-server"}}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"mcpServers":{"claude":{"command":"claude-server"}}}"#,
        )
        .unwrap();

        let data =
            <McpAsset as Asset<Vec<McpData>>>::get_data(&McpAsset::new("lingcode", home)).unwrap();

        assert_eq!(data.len(), 2);
        assert!(data.iter().any(|server| server.name == "lingcode"));
        assert!(data.iter().any(|server| server.name == "claude"));
    }
}
