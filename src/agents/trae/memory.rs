use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, MemoryData};
use crate::utils::collect_memory_paths;

#[derive(Debug, Clone)]
pub(super) struct MemoryAsset {
    core: AssetCore,
}

impl MemoryAsset {
    pub(super) fn new(agent_name: impl Into<String>, agent_home: impl Into<std::path::PathBuf>) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MemoryAsset, AssetType::Memory, Vec<MemoryData>);

impl Asset<Vec<MemoryData>> for MemoryAsset {
    fn get_data(&self) -> SentraResult<Vec<MemoryData>> {
        let mut paths = vec![self.core.agent_home().join("memory")];
        if let Some(path) = crate::agents::trae::workspace_path(".trae/rules") {
            paths.push(path);
        }
        if let Some(path) = crate::agents::trae::workspace_path("AGENTS.md") {
            paths.push(path);
        }
        Ok(collect_memory_paths(
            &paths,
            &["trae".to_string(), "memory".to_string()],
        ))
    }
}
