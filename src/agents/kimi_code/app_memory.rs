use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
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
        let agents_home =
            crate::agents::kimi_code::app_daimon_home(self.core.agent_home()).join("agents");
        let vaults = std::fs::read_dir(agents_home)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("memory").join("vault"))
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        Ok(collect_memory_paths(
            &vaults,
            &[
                "kimi-app".to_string(),
                "daimon".to_string(),
                "memory".to_string(),
            ],
        ))
    }
}
