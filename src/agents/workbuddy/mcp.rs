use std::path::PathBuf;

use crate::SentraResult;
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
        let mut results = read_servers(home.join(".mcp.json"), None)?;
        let mut connector_configs = std::fs::read_dir(home.join("connectors"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| {
                (
                    entry.file_name().to_string_lossy().to_string(),
                    entry.path().join("mcp.json"),
                )
            })
            .filter(|(_, path)| path.is_file())
            .collect::<Vec<_>>();
        connector_configs.sort_by(|left, right| left.0.cmp(&right.0));
        for (profile, path) in connector_configs {
            results.extend(read_servers(path, Some(profile))?);
        }
        Ok(results)
    }
}

fn read_servers(path: PathBuf, project: Option<String>) -> SentraResult<Vec<McpData>> {
    let Some(config) = read_json_file(path)? else {
        return Ok(Vec::new());
    };
    Ok(parse_mcp_servers(
        config
            .get("mcpServers")
            .or_else(|| config.get("servers"))
            .unwrap_or(&serde_json::Value::Null),
        project,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_root_and_connector_profile_servers() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("connectors").join("default");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"root":{"url":"https://example.com"}}}"#,
        )
        .unwrap();
        std::fs::write(
            profile.join("mcp.json"),
            r#"{"mcpServers":{"connector":{"command":"connector-server"}}}"#,
        )
        .unwrap();

        let data =
            <McpAsset as Asset<Vec<McpData>>>::get_data(&McpAsset::new("workbuddy", dir.path()))
                .unwrap();

        assert_eq!(data.len(), 2);
        assert!(
            data.iter()
                .any(|server| server.name == "root" && server.project.is_none())
        );
        assert!(data.iter().any(|server| {
            server.name == "connector" && server.project.as_deref() == Some("default")
        }));
    }
}
