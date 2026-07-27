use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::surface;
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
        mcp_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn mcp_data(agent_name: &str, agent_home: &std::path::Path) -> SentraResult<Vec<McpData>> {
    let mut results = Vec::new();
    for path in config_paths(agent_name, agent_home) {
        let Some(config) = read_json_file(path)? else {
            continue;
        };
        for raw in [
            config.get("mcpServers"),
            config.get("servers"),
            config.get("mcp"),
        ]
        .into_iter()
        .flatten()
        {
            results.extend(parse_mcp_servers(raw, None));
        }
    }
    Ok(dedup_mcp(results))
}

fn config_paths(agent_name: &str, agent_home: &std::path::Path) -> Vec<std::path::PathBuf> {
    let state_home = surface::state_home(agent_name, agent_home);
    let mut paths = vec![
        agent_home.join("User").join("mcp.json"),
        agent_home.join("mcp.json"),
        state_home.join("work").join("mcp.json"),
    ];
    for app_root in surface::work_data_roots(agent_name, agent_home) {
        paths.push(app_root.join("User").join("mcp.json"));
    }
    paths
}

fn dedup_mcp(items: Vec<McpData>) -> Vec<McpData> {
    let mut out = Vec::new();
    for item in items {
        if out
            .iter()
            .any(|seen: &McpData| seen.name == item.name && seen.project == item.project)
        {
            continue;
        }
        out.push(item);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::interfaces::{Asset, McpData};

    use super::*;

    #[test]
    fn reads_work_mcp_without_ide_user_mcp() {
        let dir = tempfile::tempdir().unwrap();
        let state_home = dir.path().join(".trae");
        let work_home = state_home.join("work");
        let ide_app = dir.path().join("AppData").join("Roaming").join("Trae");
        std::fs::create_dir_all(&work_home).unwrap();
        std::fs::create_dir_all(ide_app.join("User")).unwrap();
        std::fs::write(
            work_home.join("mcp.json"),
            r#"{"mcpServers":{"work-server":{"command":"work-mcp"}}}"#,
        )
        .unwrap();
        std::fs::write(
            ide_app.join("User").join("mcp.json"),
            r#"{"mcpServers":{"ide-server":{"command":"ide-mcp"}}}"#,
        )
        .unwrap();

        let servers =
            <McpAsset as Asset<Vec<McpData>>>::get_data(&McpAsset::new("trae-work", &work_home))
                .unwrap();

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "work-server");
    }
}
