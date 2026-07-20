use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
};
use crate::SentraResult;

#[derive(Debug, Clone)]
pub(super) struct ProviderAsset {
    pub(crate) core: AssetCore,
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

    pub fn get_request(&self, _model: &str) -> Vec<crate::interfaces::ProviderProbeRequest> {
        Vec::new()
    }
}

impl_erased_asset!(
    ProviderAsset,
    AssetType::Provider,
    Vec<ProviderData>,
    ProviderData,
    provider
);

impl Asset<Vec<ProviderData>, ProviderData> for ProviderAsset {
    fn get_data(&self) -> SentraResult<Vec<ProviderData>> {
        let mut providers = Vec::new();
        for plugin in
            crate::agents::vscode::plugin::PluginAsset::new("vscode", self.core.agent_home())
                .get_data()?
        {
            if plugin.id.as_deref().is_some_and(|id| {
                id.eq_ignore_ascii_case("GitHub.copilot")
                    || id.eq_ignore_ascii_case("GitHub.copilot-chat")
            }) {
                providers.push(ProviderData {
                    name: "GitHub Copilot".to_string(),
                    enabled: true,
                    models: Vec::<ProviderModel>::new(),
                    protocol: None,
                    ..ProviderData::default()
                });
                break;
            }
        }
        Ok(providers)
    }

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "VS Code provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "VS Code provider mutation is not supported",
        ))
    }
}
