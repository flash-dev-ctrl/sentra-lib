use serde_json::Value;

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest,
};
use crate::utils::protocol::{WireProtocol, default_model_probe_prompt};
use crate::utils::read_json_file;

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

    pub fn get_request(&self, _model: &str) -> Vec<ProviderProbeRequest> {
        [
            WireProtocol::Responses,
            WireProtocol::ChatCompletions,
            WireProtocol::AnthropicMessages,
        ]
        .into_iter()
        .map(|protocol| ProviderProbeRequest {
            protocol,
            body: None,
            prompt: Some(default_model_probe_prompt()),
        })
        .collect()
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
        provider_data(self.core.agent_home())
    }

    fn set_data(&self, _value: ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Pi provider mutation is not supported",
        ))
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "Pi provider mutation is not supported",
        ))
    }
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let settings = read_json_file(agent_home.join("settings.json"))?.unwrap_or(Value::Null);
    let models_config = read_json_file(agent_home.join("models.json"))?.unwrap_or(Value::Null);
    let auth = read_json_file(agent_home.join("auth.json"))?.unwrap_or(Value::Null);

    let default_provider = string_at(&settings, &["defaultProvider"]);
    let default_model = string_at(&settings, &["defaultModel"]);
    let auth_providers = auth_providers(&auth);
    let mut providers = Vec::new();
    let mut provider_ids = Vec::new();

    if let Some(raw_providers) = models_config
        .get("providers")
        .or_else(|| models_config.get("modelProviders"))
        .and_then(Value::as_object)
    {
        for (id, raw) in raw_providers {
            let raw = raw.as_object();
            let provider_id = id.as_str();
            let enabled = provider_enabled(default_provider.as_deref(), provider_id);
            let mut models = provider_models(raw.and_then(|raw| raw.get("models")));
            if enabled && let Some(model) = default_model.as_deref() {
                ensure_model(&mut models, model, true);
            }

            let name = raw
                .and_then(|raw| {
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .and_then(Value::as_str)
                })
                .unwrap_or(provider_id)
                .to_string();
            providers.push(ProviderData {
                name,
                base_url: raw
                    .and_then(base_url)
                    .or_else(|| pi_builtin_base_url(provider_id).map(str::to_string)),
                api_key: raw
                    .and_then(|raw| {
                        raw.get("apiKey")
                            .or_else(|| raw.get("api_key"))
                            .and_then(Value::as_str)
                            .and_then(literal_config_string)
                    })
                    .or_else(|| auth_key(auth_providers, provider_id)),
                enabled,
                models,
                protocol: raw
                    .and_then(|raw| raw.get("protocol").and_then(Value::as_str))
                    .and_then(|value| value.parse().ok()),
                ..ProviderData::default()
            });
            provider_ids.push(provider_id.to_string());
        }

        if let Some(provider_id) = default_provider.as_deref()
            && !contains_provider_id(&provider_ids, provider_id)
        {
            providers.push(fallback_provider(
                provider_id,
                default_model.as_deref(),
                auth_providers,
                true,
            ));
            provider_ids.push(provider_id.to_string());
        }
    }

    if let Some(provider_id) = default_provider.as_deref()
        && !contains_provider_id(&provider_ids, provider_id)
    {
        providers.push(fallback_provider(
            provider_id,
            default_model.as_deref(),
            auth_providers,
            true,
        ));
        provider_ids.push(provider_id.to_string());
    }

    if let Some(auth_providers) = auth_providers {
        for provider_id in auth_providers.keys() {
            if contains_provider_id(&provider_ids, provider_id) {
                continue;
            }
            providers.push(fallback_provider(
                provider_id,
                None,
                Some(auth_providers),
                provider_enabled(default_provider.as_deref(), provider_id),
            ));
            provider_ids.push(provider_id.to_string());
        }
    }

    Ok(providers)
}

fn contains_provider_id(provider_ids: &[String], provider_id: &str) -> bool {
    provider_ids.iter().any(|id| id == provider_id)
}

fn provider_models(raw: Option<&Value>) -> Vec<ProviderModel> {
    match raw {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(id) => Some(model(id, Some(id), true)),
                Value::Object(raw) => {
                    let id = raw
                        .get("id")
                        .or_else(|| raw.get("name"))
                        .and_then(Value::as_str)?;
                    Some(model(
                        id,
                        raw.get("displayName")
                            .or_else(|| raw.get("label"))
                            .or_else(|| raw.get("name"))
                            .and_then(Value::as_str),
                        raw.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                    ))
                }
                _ => None,
            })
            .collect(),
        Some(Value::Object(items)) => items
            .iter()
            .map(|(id, value)| {
                let name = value
                    .get("name")
                    .or_else(|| value.get("displayName"))
                    .or_else(|| value.get("label"))
                    .and_then(Value::as_str)
                    .unwrap_or(id);
                model(id, Some(name), true)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ensure_model(models: &mut Vec<ProviderModel>, id: &str, enabled: bool) {
    if let Some(model) = models.iter_mut().find(|model| model.id == id) {
        model.enabled = enabled;
        return;
    }
    models.push(model(id, Some(id), enabled));
}

fn model(id: &str, name: Option<&str>, enabled: bool) -> ProviderModel {
    ProviderModel {
        id: id.to_string(),
        name: name.map(str::to_string).or_else(|| Some(id.to_string())),
        enabled,
    }
}

fn fallback_provider(
    provider_id: &str,
    default_model: Option<&str>,
    auth_providers: Option<&serde_json::Map<String, Value>>,
    enabled: bool,
) -> ProviderData {
    ProviderData {
        name: provider_id.to_string(),
        base_url: pi_builtin_base_url(provider_id).map(str::to_string),
        api_key: auth_key(auth_providers, provider_id),
        enabled,
        models: default_model
            .map(|id| vec![model(id, Some(id), true)])
            .unwrap_or_default(),
        protocol: None,
        ..ProviderData::default()
    }
}

fn provider_enabled(default_provider: Option<&str>, provider_id: &str) -> bool {
    match default_provider {
        Some(default_provider) => default_provider == provider_id,
        None => true,
    }
}

fn pi_builtin_base_url(provider_id: &str) -> Option<&'static str> {
    match provider_id
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "abacus" => Some("https://routellm.abacus.ai/v1"),
        "ant-ling" => Some("https://api.ant-ling.com/v1"),
        "anthropic" => Some("https://api.anthropic.com"),
        "cerebras" => Some("https://api.cerebras.ai/v1"),
        "deepseek" => Some("https://api.deepseek.com"),
        "fireworks" => Some("https://api.fireworks.ai/inference"),
        "github-copilot" => Some("https://api.individual.githubcopilot.com"),
        "google" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "groq" => Some("https://api.groq.com/openai/v1"),
        "huggingface" => Some("https://router.huggingface.co/v1"),
        "kimi" => Some("https://api.moonshot.cn/v1"),
        "kimi-coding" | "kimi-for-coding" => Some("https://api.kimi.com/coding/v1"),
        "minimax" => Some("https://api.minimax.io/anthropic/v1"),
        "minimax-cn" => Some("https://api.minimaxi.com/anthropic/v1"),
        "mistral" => Some("https://api.mistral.ai/v1"),
        "moonshotai" => Some("https://api.moonshot.ai/v1"),
        "moonshotai-cn" => Some("https://api.moonshot.cn/v1"),
        "nvidia" => Some("https://integrate.api.nvidia.com/v1"),
        "openai" => Some("https://api.openai.com/v1"),
        "openai-codex" => Some("https://chatgpt.com/backend-api"),
        "opencode" => Some("https://opencode.ai/zen/v1"),
        "opencode-go" => Some("https://opencode.ai/zen/go/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "radius" => Some("https://radius.pi.dev"),
        "together" => Some("https://api.together.ai/v1"),
        "vercel-ai-gateway" => Some("https://ai-gateway.vercel.sh"),
        "xai" => Some("https://api.x.ai/v1"),
        "xiaomi" => Some("https://api.xiaomimimo.com/v1"),
        "xiaomi-token-plan-ams" => Some("https://token-plan-ams.xiaomimimo.com/v1"),
        "xiaomi-token-plan-cn" => Some("https://token-plan-cn.xiaomimimo.com/v1"),
        "xiaomi-token-plan-sgp" => Some("https://token-plan-sgp.xiaomimimo.com/v1"),
        "zai" => Some("https://api.z.ai/api/coding/paas/v4"),
        "zai-coding-cn" => Some("https://open.bigmodel.cn/api/coding/paas/v4"),
        _ => None,
    }
}

fn base_url(raw: &serde_json::Map<String, Value>) -> Option<String> {
    raw.get("baseURL")
        .or_else(|| raw.get("baseUrl"))
        .or_else(|| raw.get("base_url"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn string_at(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn auth_providers(auth: &Value) -> Option<&serde_json::Map<String, Value>> {
    auth.get("providers")
        .and_then(Value::as_object)
        .or_else(|| auth.as_object())
}

fn auth_key(
    providers: Option<&serde_json::Map<String, Value>>,
    provider_id: &str,
) -> Option<String> {
    let value = providers?.get(provider_id)?;
    if let Some(key) = value.as_str() {
        return literal_config_string(key);
    }
    let raw = value.as_object()?;
    raw.get("apiKey")
        .or_else(|| raw.get("api_key"))
        .or_else(|| raw.get("key"))
        .or_else(|| raw.get("token"))
        .and_then(Value::as_str)
        .and_then(literal_config_string)
}

fn literal_config_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.starts_with('!')).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::pi_builtin_base_url;

    #[test]
    fn pi_builtin_base_urls_cover_static_catalog() {
        for (provider_id, base_url) in [
            ("abacus", "https://routellm.abacus.ai/v1"),
            ("ant-ling", "https://api.ant-ling.com/v1"),
            ("anthropic", "https://api.anthropic.com"),
            ("cerebras", "https://api.cerebras.ai/v1"),
            ("deepseek", "https://api.deepseek.com"),
            ("fireworks", "https://api.fireworks.ai/inference"),
            ("github-copilot", "https://api.individual.githubcopilot.com"),
            ("google", "https://generativelanguage.googleapis.com/v1beta"),
            ("groq", "https://api.groq.com/openai/v1"),
            ("huggingface", "https://router.huggingface.co/v1"),
            ("kimi", "https://api.moonshot.cn/v1"),
            ("kimi-coding", "https://api.kimi.com/coding/v1"),
            ("kimi-for-coding", "https://api.kimi.com/coding/v1"),
            ("minimax", "https://api.minimax.io/anthropic/v1"),
            ("minimax-cn", "https://api.minimaxi.com/anthropic/v1"),
            ("mistral", "https://api.mistral.ai/v1"),
            ("moonshotai", "https://api.moonshot.ai/v1"),
            ("moonshotai-cn", "https://api.moonshot.cn/v1"),
            ("nvidia", "https://integrate.api.nvidia.com/v1"),
            ("openai", "https://api.openai.com/v1"),
            ("openai-codex", "https://chatgpt.com/backend-api"),
            ("opencode", "https://opencode.ai/zen/v1"),
            ("opencode-go", "https://opencode.ai/zen/go/v1"),
            ("openrouter", "https://openrouter.ai/api/v1"),
            ("radius", "https://radius.pi.dev"),
            ("together", "https://api.together.ai/v1"),
            ("vercel-ai-gateway", "https://ai-gateway.vercel.sh"),
            ("xai", "https://api.x.ai/v1"),
            ("xiaomi", "https://api.xiaomimimo.com/v1"),
            (
                "xiaomi-token-plan-ams",
                "https://token-plan-ams.xiaomimimo.com/v1",
            ),
            (
                "xiaomi-token-plan-cn",
                "https://token-plan-cn.xiaomimimo.com/v1",
            ),
            (
                "xiaomi-token-plan-sgp",
                "https://token-plan-sgp.xiaomimimo.com/v1",
            ),
            ("zai", "https://api.z.ai/api/coding/paas/v4"),
            (
                "zai-coding-cn",
                "https://open.bigmodel.cn/api/coding/paas/v4",
            ),
        ] {
            assert_eq!(pi_builtin_base_url(provider_id), Some(base_url));
        }

        assert_eq!(
            pi_builtin_base_url("opencode_go"),
            pi_builtin_base_url("opencode-go")
        );
        assert_eq!(pi_builtin_base_url("azure-openai-responses"), None);
        assert_eq!(pi_builtin_base_url("cloudflare-ai-gateway"), None);
    }
}
