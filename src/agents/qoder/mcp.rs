use serde_json::Value;

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
        let mut out = Vec::new();
        let cwd = std::env::current_dir().unwrap_or_default();
        let project_home = cwd.join(format!(".{}", self.core.agent_name()));
        for root in [self.core.agent_home().to_path_buf(), project_home] {
            for file in ["settings.json", "settings.local.json", "mcp.json"] {
                let Some(config) = read_json_file(root.join(file))? else {
                    continue;
                };
                out.extend(parse_servers(
                    config
                        .get("mcpServers")
                        .or_else(|| config.get("mcp"))
                        .or_else(|| config.get("servers")),
                ));
            }
        }
        for path in [
            cwd.join(".mcp.json"),
            global_config_file(self.core.agent_home()),
        ] {
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            out.extend(parse_servers(
                config
                    .get("mcpServers")
                    .or_else(|| config.get("mcp"))
                    .or_else(|| config.get("servers")),
            ));
        }
        Ok(out)
    }
}

fn global_config_file(agent_home: &std::path::Path) -> std::path::PathBuf {
    agent_home.parent().unwrap_or(agent_home).join(format!(
        "{}.json",
        agent_home.file_name().unwrap_or_default().to_string_lossy()
    ))
}

fn parse_servers(raw: Option<&Value>) -> Vec<McpData> {
    parse_mcp_servers(raw.unwrap_or(&Value::Null), None)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn global_config_file_keeps_hidden_agent_name() {
        let path = super::global_config_file(Path::new("/home/me/.qoder-cn"));
        assert_eq!(path, Path::new("/home/me/.qoder-cn.json"));
    }
}
