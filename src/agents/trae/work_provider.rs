use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::trae::{provider, surface};
use crate::interfaces::{Asset, AssetType, ProviderData};
use crate::utils::read_text_file;

#[derive(Debug, Clone)]
pub(super) struct ProviderAsset {
    core: AssetCore,
}

impl ProviderAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(ProviderAsset, AssetType::Provider, Vec<ProviderData>);

impl Asset<Vec<ProviderData>> for ProviderAsset {
    fn get_data(&self) -> SentraResult<Vec<ProviderData>> {
        read_provider_data(self.core.agent_name(), self.core.agent_home(), true)
    }

    fn get_runtime_data(&self) -> SentraResult<Vec<ProviderData>> {
        read_provider_data(self.core.agent_name(), self.core.agent_home(), false)
    }
}

fn read_provider_data(
    agent_name: &str,
    agent_home: &std::path::Path,
    mask_secrets: bool,
) -> SentraResult<Vec<ProviderData>> {
    let state_home = surface::state_home(agent_name, agent_home);
    for path in [
        agent_home.join("trae_config.yaml"),
        state_home.join("work").join("trae_config.yaml"),
    ] {
        let Some(content) = read_text_file(path)? else {
            continue;
        };
        let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
            continue;
        };
        return Ok(provider::provider_data(&config, mask_secrets));
    }
    Ok(Vec::new())
}
