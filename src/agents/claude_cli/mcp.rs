use crate::SentraResult;
use crate::agents::install_status::hidden_home_parent;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{parse_mcp_servers, read_json_file};

#[derive(Debug, Clone)]
pub(super) struct McpAsset {
    pub(crate) core: AssetCore,
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
        mcp_data(self.core.agent_home())
    }
}

fn mcp_data(agent_home: &std::path::Path) -> SentraResult<Vec<McpData>> {
    let Some(config) = read_json_file(hidden_home_parent(agent_home).join(".claude.json"))? else {
        return Ok(Vec::new());
    };
    let mut results = parse_mcp_servers(
        config.get("mcpServers").unwrap_or(&serde_json::Value::Null),
        None,
    );
    if let Some(projects) = config.get("projects").and_then(|value| value.as_object()) {
        for (project_path, project) in projects {
            results.extend(parse_mcp_servers(
                project
                    .get("mcpServers")
                    .unwrap_or(&serde_json::Value::Null),
                Some(project_path.clone()),
            ));
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_root_claude_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".claude");
        std::fs::create_dir_all(&agent_home).unwrap();
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"mcpServers":{"root":{"command":"root-command"}},"projects":{"project-a":{"mcpServers":{"project":{"command":"project-command"}}}}}"#,
        )
        .unwrap();

        let data = mcp_data(&agent_home).unwrap();

        assert_eq!(data.len(), 2);
        assert!(data.iter().any(|server| server.name == "root"));
        assert!(data.iter().any(|server| {
            server.name == "project" && server.project.as_deref() == Some("project-a")
        }));
    }
}
