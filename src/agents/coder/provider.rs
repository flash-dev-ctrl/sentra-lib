use crate::agents::object::{impl_erased_asset, AssetCore};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderAccount, ProviderData,
    ProviderType,
};
use crate::utils::read_json_file;
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
        let Some(config) =
            read_json_file(super::config_home(self.core.agent_home()).join("config.json"))?
        else {
            return Ok(Vec::new());
        };
        let Some(url) = config
            .get("url")
            .or_else(|| config.get("access_url"))
            .and_then(|value| value.as_str())
        else {
            return Ok(Vec::new());
        };
        Ok(vec![ProviderData {
            name: "Coder".to_string(),
            provider_type: ProviderType::Gateway,
            base_url: Some(url.to_string()),
            enabled: true,
            account: Some(ProviderAccount {
                source: Some("config.json".to_string()),
                ..ProviderAccount::default()
            }),
            ..ProviderData::default()
        }])
    }

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Coder provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Coder provider mutation is not supported",
        ))
    }
}
