use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::surface;
use crate::interfaces::{Asset, AssetType, MemoryData};
use crate::utils::collect_memory_paths;

#[derive(Debug, Clone)]
pub(super) struct MemoryAsset {
    core: AssetCore,
}

impl MemoryAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(MemoryAsset, AssetType::Memory, Vec<MemoryData>);

impl Asset<Vec<MemoryData>> for MemoryAsset {
    fn get_data(&self) -> SentraResult<Vec<MemoryData>> {
        let state_home = surface::state_home(self.core.agent_name(), self.core.agent_home());
        Ok(collect_memory_paths(
            &[
                self.core.agent_home().join("memory"),
                state_home.join("work").join("memory"),
            ],
            &[self.core.agent_name().to_string(), "memory".to_string()],
        ))
    }
}
