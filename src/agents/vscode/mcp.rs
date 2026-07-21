use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, McpData};
use crate::utils::{parse_mcp_servers, read_jsonc_file};

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
        let mut results = Vec::new();
        for path in mcp_paths(self.core.agent_home()) {
            let Some(config) = read_jsonc_file(path)? else {
                continue;
            };
            results.extend(parse_mcp_servers(
                config
                    .get("servers")
                    .or_else(|| config.get("mcpServers"))
                    .unwrap_or(&serde_json::Value::Null),
                None,
            ));
        }
        Ok(results)
    }
}

fn mcp_paths(agent_home: &std::path::Path) -> Vec<std::path::PathBuf> {
    let user_home = super::user_home(agent_home);
    let mut paths = vec![
        std::env::current_dir()
            .unwrap_or_default()
            .join(".vscode")
            .join("mcp.json"),
        agent_home.join("mcp.json"),
    ];
    #[cfg(windows)]
    let config_root = crate::agents::install_status::env_path("APPDATA")
        .unwrap_or_else(|| user_home.join("AppData").join("Roaming"));
    #[cfg(target_os = "macos")]
    let config_root = user_home.join("Library").join("Application Support");
    #[cfg(all(unix, not(target_os = "macos")))]
    let config_root = crate::agents::install_status::env_path("XDG_CONFIG_HOME")
        .unwrap_or_else(|| user_home.join(".config"));
    #[cfg(not(any(windows, unix)))]
    let config_root = user_home.join(".config");
    for product in ["Code", "Code - Insiders"] {
        paths.push(config_root.join(product).join("User").join("mcp.json"));
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_platform_user_configuration() {
        let paths = mcp_paths(std::path::Path::new("home").join(".vscode").as_path());

        assert!(paths.iter().any(|path| {
            path.ends_with(std::path::Path::new("Code").join("User").join("mcp.json"))
        }));
    }
}
