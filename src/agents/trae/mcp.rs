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
        extend_standard_mcp(&mut results, &config, None);
    }
    extend_metadata_mcp(&mut results, &surface::state_home(agent_name, agent_home))?;
    Ok(dedup_mcp(results))
}

fn config_paths(agent_name: &str, agent_home: &std::path::Path) -> Vec<std::path::PathBuf> {
    let state_home = surface::state_home(agent_name, agent_home);
    let mut paths = vec![
        agent_home.join("User").join("mcp.json"),
        agent_home.join("mcp.json"),
        state_home.join("mcp.json"),
    ];
    if let Some(path) = crate::agents::trae::workspace_path(".trae/mcp.json") {
        paths.push(path);
    }
    for app_root in surface::ide_data_roots(agent_name, agent_home) {
        paths.push(app_root.join("User").join("mcp.json"));
    }
    paths
}

fn extend_standard_mcp(
    out: &mut Vec<McpData>,
    config: &serde_json::Value,
    project: Option<String>,
) {
    for raw in [
        config.get("mcpServers"),
        config.get("servers"),
        config.get("mcp"),
    ]
    .into_iter()
    .flatten()
    {
        out.extend(parse_mcp_servers(raw, project.clone()));
    }
}

fn extend_metadata_mcp(out: &mut Vec<McpData>, agent_home: &std::path::Path) -> SentraResult<()> {
    for path in find_server_metadata(&agent_home.join("mcps"), 5, 0) {
        let Some(config) = read_json_file(&path)? else {
            continue;
        };
        let Some(name) = string_field(&config, "server_name")
            .or_else(|| string_field(&config, "name"))
            .or_else(|| {
                path.parent()
                    .and_then(|parent| parent.file_name())
                    .map(|name| name.to_string_lossy().to_string())
            })
        else {
            continue;
        };
        out.push(McpData {
            name,
            enabled: Some(true),
            project: metadata_project(agent_home, &path),
            ..McpData::default()
        });
    }
    Ok(())
}

fn find_server_metadata(
    dir: &std::path::Path,
    max_depth: usize,
    depth: usize,
) -> Vec<std::path::PathBuf> {
    if depth > max_depth || !crate::utils::dir_exists(dir) {
        return Vec::new();
    }
    let candidate = dir.join("SERVER_METADATA.json");
    if candidate.is_file() {
        return vec![candidate];
    }
    let mut results = Vec::new();
    for entry in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| crate::utils::is_directory(path))
    {
        results.extend(find_server_metadata(&entry, max_depth, depth + 1));
    }
    results
}

fn metadata_project(agent_home: &std::path::Path, path: &std::path::Path) -> Option<String> {
    let relative = path
        .parent()?
        .strip_prefix(agent_home.join("mcps"))
        .ok()?
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if relative.len() > 1 {
        Some(relative[..relative.len() - 1].join("/"))
    } else {
        None
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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
    fn reads_ide_user_mcp_config() {
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("AppData").join("Roaming").join("Trae");
        std::fs::create_dir_all(app_root.join("User")).unwrap();
        std::fs::write(
            app_root.join("User").join("mcp.json"),
            r#"{"mcpServers":{"browser":{"command":"npx","args":["chrome-devtools-mcp@latest"],"disabled":true}}}"#,
        )
        .unwrap();

        let servers =
            <McpAsset as Asset<Vec<McpData>>>::get_data(&McpAsset::new("trae-ide", app_root))
                .unwrap();

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "browser");
        assert_eq!(servers[0].enabled, Some(false));
    }

    #[test]
    fn reads_server_metadata_cache() {
        let dir = tempfile::tempdir().unwrap();
        let state_home = dir.path().join(".trae");
        let app_root = dir.path().join("AppData").join("Roaming").join("Trae");
        let server = state_home
            .join("mcps")
            .join("project-a")
            .join("dev_agent")
            .join("mcp_shadcn-ui");
        std::fs::create_dir_all(&server).unwrap();
        std::fs::write(
            server.join("SERVER_METADATA.json"),
            r#"{"server_name":"mcp_shadcn-ui"}"#,
        )
        .unwrap();

        let servers = mcp_data("trae-ide", &app_root).unwrap();

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "mcp_shadcn-ui");
        assert_eq!(servers[0].project.as_deref(), Some("project-a/dev_agent"));
    }
}
