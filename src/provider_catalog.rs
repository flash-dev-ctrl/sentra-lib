use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::interfaces::{ProviderData, ProviderModel, ProviderType};
use crate::utils::protocol::WireProtocol;

const PROVIDER_CATALOG_JSON: &str = r#"[
  {"id":"abacus","name":"Abacus","baseUrl":"https://routellm.abacus.ai/v1"},
  {"id":"anthropic","name":"Anthropic","baseUrl":"https://api.anthropic.com"},
  {"id":"openai","name":"OpenAI","baseUrl":"https://api.openai.com/v1"},
  {"id":"azure-openai","name":"Azure OpenAI","baseUrl":""},
  {"id":"openrouter","name":"OpenRouter","baseUrl":"https://openrouter.ai/api/v1"},
  {"id":"kimi","name":"Kimi","baseUrl":"https://api.moonshot.cn/v1"},
  {"id":"kimi-for-coding","name":"Kimi For Coding","baseUrl":"https://api.kimi.com/coding/v1"},
  {"id":"kimi_coding","name":"Kimi For Coding","baseUrl":"https://api.kimi.com/coding/v1"},
  {"id":"moonshotai","name":"Moonshot AI","baseUrl":"https://api.moonshot.ai/v1"},
  {"id":"moonshotai-cn","name":"Moonshot AI (China)","baseUrl":"https://api.moonshot.cn/v1"},
  {"id":"deepseek","name":"DeepSeek","baseUrl":"https://api.deepseek.com"},
  {"id":"groq","name":"Groq","baseUrl":"https://api.groq.com/openai/v1"},
  {"id":"minimax","name":"MiniMax","baseUrl":"https://api.minimax.io/anthropic/v1"},
  {"id":"minimax-cn","name":"MiniMax","baseUrl":"https://api.minimaxi.com/anthropic/v1"},
  {"id":"mistral","name":"Mistral AI","baseUrl":"https://api.mistral.ai/v1"},
  {"id":"xai","name":"xAI","baseUrl":"https://api.x.ai/v1"},
  {"id":"opencode","name":"OpenCode Zen","baseUrl":"https://opencode.ai/zen/v1"},
  {"id":"opencode-go","name":"OpenCode Go","baseUrl":"https://opencode.ai/zen/go/v1"},
  {"id":"opencode_go","name":"OpenCode Go","baseUrl":"https://opencode.ai/zen/go/v1"},
  {"id":"google","name":"Google AI","baseUrl":""},
  {"id":"cerebras","name":"Cerebras","baseUrl":""},
  {"id":"nvidia","name":"NVIDIA","baseUrl":"https://integrate.api.nvidia.com/v1"},
  {"id":"cloudflare","name":"Cloudflare AI","baseUrl":""},
  {"id":"vercel-ai-gateway","name":"Vercel AI Gateway","baseUrl":""},
  {"id":"zai","name":"Z.AI","baseUrl":""},
  {"id":"ant-ling","name":"Ant Ling","baseUrl":""}
]"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderActivationStatus {
    Active,
    Inactive,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResolutionStatus {
    Known,
    Custom,
    Ambiguous,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFieldSource {
    Configured,
    Catalog,
    Inferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCandidate {
    pub agent_id: String,
    pub agent_provider_id: Option<String>,
    pub display_name: Option<String>,
    pub configured_base_url: Option<String>,
    pub protocol_hint: Option<WireProtocol>,
    pub protocol_source: Option<ProviderFieldSource>,
    pub api_key: Option<String>,
    pub activation: ProviderActivationStatus,
    pub models: Vec<ProviderModel>,
}

impl ProviderCandidate {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_provider_id: None,
            display_name: None,
            configured_base_url: None,
            protocol_hint: None,
            protocol_source: None,
            api_key: None,
            activation: ProviderActivationStatus::Unknown,
            models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDefinition {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub environment_keys: Vec<String>,
    pub default_endpoint: Option<String>,
    #[serde(default)]
    pub endpoints: Vec<ProviderEndpoint>,
    #[serde(default)]
    pub agent_aliases: Vec<ProviderAgentAlias>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderEndpoint {
    pub id: String,
    pub base_url: String,
    pub protocol: WireProtocol,
    #[serde(default)]
    pub environment_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAgentAlias {
    pub agent: String,
    pub alias: String,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub environment_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderCatalogFile {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    revision: String,
    providers: Vec<ProviderDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EmbeddedProviderCatalog {
    Structured(ProviderCatalogFile),
    Simple(Vec<ProviderCatalogEntry>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderCatalogEntry {
    id: String,
    name: String,
    base_url: String,
}

#[derive(Debug)]
pub struct ProviderRegistry {
    schema_version: u32,
    revision: String,
    providers: Vec<ProviderDefinition>,
}

#[derive(Debug, Clone, Copy)]
struct ProviderMatch<'a> {
    provider: &'a ProviderDefinition,
    endpoint: Option<&'a ProviderEndpoint>,
    inferred: bool,
}

impl ProviderRegistry {
    pub fn builtin() -> &'static Self {
        static REGISTRY: OnceLock<ProviderRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            let catalog: EmbeddedProviderCatalog = serde_json::from_str(PROVIDER_CATALOG_JSON)
                .expect("embedded provider catalog must be valid");
            let catalog = catalog.into_catalog_file();
            let registry = ProviderRegistry {
                schema_version: catalog.schema_version,
                revision: catalog.revision,
                providers: catalog.providers,
            };
            registry
                .validate()
                .expect("embedded provider catalog references must be valid");
            registry
        })
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn providers(&self) -> &[ProviderDefinition] {
        &self.providers
    }

    fn validate(&self) -> Result<(), String> {
        let mut provider_ids = std::collections::HashSet::new();
        let mut global_aliases = std::collections::HashMap::<String, String>::new();
        let mut agent_aliases = std::collections::HashMap::<(String, String), String>::new();

        for provider in &self.providers {
            let provider_id = normalized_id(&provider.id);
            if !provider_ids.insert(provider_id.clone()) {
                return Err(format!("duplicate provider id: {}", provider.id));
            }

            let mut endpoint_ids = std::collections::HashSet::new();
            for endpoint in &provider.endpoints {
                if !endpoint_ids.insert(endpoint.id.as_str()) {
                    return Err(format!(
                        "duplicate endpoint {} for provider {}",
                        endpoint.id, provider.id
                    ));
                }
            }
            if let Some(default_endpoint) = provider.default_endpoint.as_deref()
                && !endpoint_ids.contains(default_endpoint)
            {
                return Err(format!(
                    "unknown default endpoint {default_endpoint} for provider {}",
                    provider.id
                ));
            }

            for alias in &provider.aliases {
                let alias = normalized_id(alias);
                if let Some(existing) = global_aliases.insert(alias.clone(), provider_id.clone())
                    && existing != provider_id
                {
                    return Err(format!("ambiguous provider alias: {alias}"));
                }
            }

            for alias in &provider.agent_aliases {
                if let Some(endpoint) = alias.endpoint.as_deref()
                    && !endpoint_ids.contains(endpoint)
                {
                    return Err(format!(
                        "unknown endpoint {endpoint} for alias {} / {}",
                        alias.agent, alias.alias
                    ));
                }
                let key = (normalized_id(&alias.agent), normalized_id(&alias.alias));
                if let Some(existing) = agent_aliases.insert(key.clone(), provider_id.clone())
                    && existing != provider_id
                {
                    return Err(format!(
                        "ambiguous agent provider alias: {} / {}",
                        key.0, key.1
                    ));
                }
            }
        }

        for provider_id in provider_ids {
            if let Some(existing) = global_aliases.get(&provider_id)
                && existing != &provider_id
            {
                return Err(format!("provider id collides with alias: {provider_id}"));
            }
        }
        Ok(())
    }

    pub fn lookup(&self, provider_id: &str) -> Option<&ProviderDefinition> {
        let provider_id = normalized_id(provider_id);
        self.providers.iter().find(|provider| {
            normalized_id(&provider.id) == provider_id
                || provider
                    .aliases
                    .iter()
                    .any(|alias| normalized_id(alias) == provider_id)
        })
    }

    pub fn identify_by_endpoint(
        &self,
        base_url: &str,
    ) -> Vec<(&ProviderDefinition, &ProviderEndpoint)> {
        let base_url = normalized_url(base_url);
        self.providers
            .iter()
            .flat_map(|provider| {
                provider
                    .endpoints
                    .iter()
                    .filter(|endpoint| normalized_url(&endpoint.base_url) == base_url)
                    .map(move |endpoint| (provider, endpoint))
            })
            .collect()
    }

    pub fn environment_keys(&self, agent_id: &str, provider_id: &str) -> Vec<&str> {
        let Some(provider_match) = self.match_by_id(agent_id, provider_id) else {
            return Vec::new();
        };
        let mut keys = Vec::new();
        if let Some(alias) = provider_match.provider.agent_aliases.iter().find(|alias| {
            normalized_id(&alias.agent) == normalized_id(agent_id)
                && normalized_id(&alias.alias) == normalized_id(provider_id)
        }) {
            keys.extend(alias.environment_keys.iter().map(String::as_str));
        }
        if let Some(endpoint) = provider_match.endpoint {
            keys.extend(endpoint.environment_keys.iter().map(String::as_str));
        }
        keys.extend(
            provider_match
                .provider
                .environment_keys
                .iter()
                .map(String::as_str),
        );
        let mut deduplicated = Vec::new();
        for key in keys {
            if !deduplicated.contains(&key) {
                deduplicated.push(key);
            }
        }
        deduplicated
    }

    pub fn resolve(&self, candidate: ProviderCandidate) -> ProviderData {
        let raw_provider_id = candidate.agent_provider_id.clone();
        let configured_base_url = candidate
            .configured_base_url
            .as_deref()
            .filter(|value| !value.trim().is_empty());

        let id_match = candidate
            .agent_provider_id
            .as_deref()
            .and_then(|id| self.match_by_id(&candidate.agent_id, id));
        let endpoint_matches = configured_base_url
            .map(|base_url| self.identify_by_endpoint(base_url))
            .unwrap_or_default();

        let provider_match = match (id_match, endpoint_matches.as_slice()) {
            (Some(mut id_match), matches) => {
                let matching_endpoints = matches
                    .iter()
                    .filter(|(provider, endpoint)| {
                        provider.id == id_match.provider.id
                            && candidate
                                .protocol_hint
                                .is_none_or(|protocol| endpoint.protocol == protocol)
                    })
                    .map(|(_, endpoint)| *endpoint)
                    .collect::<Vec<_>>();
                if matching_endpoints.len() == 1 {
                    id_match.endpoint = matching_endpoints.first().copied();
                }
                if configured_base_url.is_none()
                    && let Some(protocol) = candidate.protocol_hint
                    && id_match
                        .endpoint
                        .is_none_or(|endpoint| endpoint.protocol != protocol)
                {
                    let protocol_endpoints = id_match
                        .provider
                        .endpoints
                        .iter()
                        .filter(|endpoint| endpoint.protocol == protocol)
                        .collect::<Vec<_>>();
                    if protocol_endpoints.len() == 1 {
                        id_match.endpoint = protocol_endpoints.first().copied();
                    }
                }
                Some(id_match)
            }
            (None, matches) if !matches.is_empty() => {
                let mut providers = matches
                    .iter()
                    .map(|(provider, _)| *provider)
                    .collect::<Vec<_>>();
                providers.sort_unstable_by(|left, right| left.id.cmp(&right.id));
                providers.dedup_by(|left, right| left.id == right.id);
                if providers.len() == 1 {
                    let provider = providers[0];
                    let endpoints = matches
                        .iter()
                        .filter(|(matched_provider, endpoint)| {
                            matched_provider.id == provider.id
                                && candidate
                                    .protocol_hint
                                    .is_none_or(|protocol| endpoint.protocol == protocol)
                        })
                        .map(|(_, endpoint)| *endpoint)
                        .collect::<Vec<_>>();
                    Some(ProviderMatch {
                        provider,
                        endpoint: (endpoints.len() == 1).then(|| endpoints[0]),
                        inferred: true,
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        let ambiguous =
            id_match.is_none() && provider_match.is_none() && endpoint_matches.len() > 1;
        let resolution_status = if provider_match.is_some() {
            ProviderResolutionStatus::Known
        } else if ambiguous {
            ProviderResolutionStatus::Ambiguous
        } else if configured_base_url.is_some() {
            ProviderResolutionStatus::Custom
        } else {
            ProviderResolutionStatus::Unknown
        };

        let endpoint = provider_match.and_then(|matched| matched.endpoint);
        let base_url = configured_base_url
            .map(str::to_string)
            .or_else(|| endpoint.map(|endpoint| endpoint.base_url.clone()));
        let protocol = candidate
            .protocol_hint
            .or_else(|| endpoint.map(|endpoint| endpoint.protocol));
        let base_url_source = configured_base_url
            .map(|_| ProviderFieldSource::Configured)
            .or_else(|| endpoint.map(|_| ProviderFieldSource::Catalog));
        let protocol_source = candidate
            .protocol_hint
            .map(|_| {
                candidate
                    .protocol_source
                    .unwrap_or(ProviderFieldSource::Inferred)
            })
            .or_else(|| endpoint.map(|_| ProviderFieldSource::Catalog));

        let resolution_reason = match resolution_status {
            ProviderResolutionStatus::Known => provider_match.and_then(|matched| {
                matched
                    .inferred
                    .then(|| "provider identified from configured endpoint".to_string())
            }),
            ProviderResolutionStatus::Custom => {
                Some("configured endpoint is not present in the provider catalog".to_string())
            }
            ProviderResolutionStatus::Unknown => {
                Some("provider id and endpoint could not be matched to the catalog".to_string())
            }
            ProviderResolutionStatus::Ambiguous => {
                Some("configured endpoint matches multiple catalog entries".to_string())
            }
        };

        ProviderData {
            name: candidate
                .display_name
                .or_else(|| provider_match.map(|matched| matched.provider.display_name.clone()))
                .or_else(|| raw_provider_id.clone())
                .unwrap_or_else(|| "Unknown Provider".to_string()),
            provider_type: ProviderType::Gateway,
            provider_id: provider_match.map(|matched| matched.provider.id.clone()),
            provider_display_name: provider_match
                .map(|matched| matched.provider.display_name.clone()),
            raw_provider_id,
            base_url,
            base_url_source,
            api_key: candidate.api_key,
            enabled: candidate.activation == ProviderActivationStatus::Active,
            activation_status: candidate.activation,
            models: candidate.models,
            protocol,
            protocol_source,
            endpoint_variant: endpoint.map(|endpoint| endpoint.id.clone()),
            resolution_status,
            resolution_reason,
            account: None,
        }
    }

    fn match_by_id(&self, agent_id: &str, provider_id: &str) -> Option<ProviderMatch<'_>> {
        let agent_id = normalized_id(agent_id);
        let provider_id = normalized_id(provider_id);
        for provider in &self.providers {
            if let Some(alias) = provider.agent_aliases.iter().find(|alias| {
                normalized_id(&alias.agent) == agent_id
                    && normalized_id(&alias.alias) == provider_id
            }) {
                return Some(ProviderMatch {
                    provider,
                    endpoint: alias.endpoint.as_deref().and_then(|id| {
                        provider.endpoints.iter().find(|endpoint| endpoint.id == id)
                    }),
                    inferred: false,
                });
            }
            if normalized_id(&provider.id) == provider_id
                || provider
                    .aliases
                    .iter()
                    .any(|alias| normalized_id(alias) == provider_id)
            {
                return Some(ProviderMatch {
                    provider,
                    endpoint: provider.default_endpoint.as_deref().and_then(|id| {
                        provider.endpoints.iter().find(|endpoint| endpoint.id == id)
                    }),
                    inferred: false,
                });
            }
        }
        None
    }
}

impl EmbeddedProviderCatalog {
    fn into_catalog_file(self) -> ProviderCatalogFile {
        match self {
            Self::Structured(catalog) => catalog,
            Self::Simple(entries) => {
                let mut providers = entries
                    .into_iter()
                    .filter(simple_catalog_entry_is_registry_provider)
                    .filter(|entry| !entry.id.trim().is_empty())
                    .map(ProviderDefinition::from)
                    .collect::<Vec<_>>();
                patch_builtin_aliases(&mut providers);
                ProviderCatalogFile {
                    schema_version: 1,
                    revision: "providers.json".to_string(),
                    providers,
                }
            }
        }
    }
}

fn simple_catalog_entry_is_registry_provider(entry: &ProviderCatalogEntry) -> bool {
    !matches!(
        normalized_id(&entry.id).as_str(),
        "kimi-for-coding"
            | "kimi-coding"
            | "moonshotai"
            | "moonshotai-cn"
            | "opencode-go"
            | "minimax-cn"
            | "minimax-cn-coding-plan"
            | "minimax-coding-plan"
            | "minimax-en"
    )
}

impl From<ProviderCatalogEntry> for ProviderDefinition {
    fn from(entry: ProviderCatalogEntry) -> Self {
        let base_url = entry.base_url.trim().to_string();
        let endpoints = if base_url.is_empty() {
            Vec::new()
        } else {
            vec![ProviderEndpoint {
                id: "default".to_string(),
                base_url,
                protocol: default_protocol_for_provider(&entry.id),
                environment_keys: Vec::new(),
            }]
        };
        Self {
            id: entry.id,
            display_name: entry.name,
            aliases: Vec::new(),
            environment_keys: Vec::new(),
            default_endpoint: (!endpoints.is_empty()).then(|| "default".to_string()),
            endpoints,
            agent_aliases: Vec::new(),
        }
    }
}

fn patch_builtin_aliases(providers: &mut [ProviderDefinition]) {
    if let Some(provider) = find_provider_mut(providers, "kimi") {
        add_alias(provider, "moonshot");
        provider.default_endpoint = Some("moonshot".to_string());
        upsert_endpoint(
            provider,
            "moonshot",
            "https://api.moonshot.ai/v1",
            WireProtocol::ChatCompletions,
        );
        upsert_endpoint(
            provider,
            "kimi-code",
            "https://api.kimi.com/coding/v1",
            WireProtocol::ChatCompletions,
        );
        upsert_agent_alias(provider, "kimi-code", "kimi", "moonshot");
        upsert_agent_alias(provider, "kimi-code", "kimi-code", "kimi-code");
        upsert_agent_alias(provider, "kimi-code", "managed:kimi-code", "kimi-code");
    }

    if let Some(provider) = find_provider_mut(providers, "minimax") {
        add_alias(provider, "minimax-ai");
        provider.default_endpoint = Some("global-anthropic".to_string());
        upsert_endpoint(
            provider,
            "global-anthropic",
            "https://api.minimax.io/anthropic",
            WireProtocol::AnthropicMessages,
        );
        upsert_endpoint(
            provider,
            "cn-anthropic",
            "https://api.minimaxi.com/anthropic",
            WireProtocol::AnthropicMessages,
        );
        upsert_agent_alias(provider, "pi", "minimax-cn", "cn-anthropic");
        upsert_agent_alias(provider, "openclaw", "minimax-cn", "cn-anthropic");
        upsert_agent_alias(provider, "hermes", "minimax-cn", "cn-anthropic");
    }

    if let Some(provider) = find_provider_mut(providers, "opencode") {
        provider.display_name = "OpenCode".to_string();
        provider.default_endpoint = Some("zen".to_string());
        upsert_endpoint(
            provider,
            "zen",
            "https://opencode.ai/zen/v1",
            WireProtocol::ChatCompletions,
        );
        upsert_endpoint(
            provider,
            "go",
            "https://opencode.ai/zen/go/v1",
            WireProtocol::ChatCompletions,
        );
        upsert_endpoint(
            provider,
            "go-anthropic",
            "https://opencode.ai/zen/go",
            WireProtocol::AnthropicMessages,
        );
        upsert_agent_alias(provider, "pi", "opencode-go", "go");
        upsert_agent_alias(provider, "opencode", "opencode-go", "go");
        upsert_agent_alias(provider, "openclaw", "opencode", "zen");
        upsert_agent_alias(provider, "openclaw", "opencode-zen", "zen");
        upsert_agent_alias(provider, "openclaw", "opencode-go", "go");
        upsert_agent_alias(provider, "hermes", "opencode", "zen");
        upsert_agent_alias(provider, "hermes", "opencode-zen", "zen");
        upsert_agent_alias(provider, "hermes", "opencode-go", "go");
    }
}

fn find_provider_mut<'a>(
    providers: &'a mut [ProviderDefinition],
    id: &str,
) -> Option<&'a mut ProviderDefinition> {
    providers
        .iter_mut()
        .find(|provider| normalized_id(&provider.id) == normalized_id(id))
}

fn add_alias(provider: &mut ProviderDefinition, alias: &str) {
    if !provider
        .aliases
        .iter()
        .any(|value| normalized_id(value) == normalized_id(alias))
    {
        provider.aliases.push(alias.to_string());
    }
}

fn upsert_endpoint(
    provider: &mut ProviderDefinition,
    id: &str,
    base_url: &str,
    protocol: WireProtocol,
) {
    if let Some(endpoint) = provider
        .endpoints
        .iter_mut()
        .find(|endpoint| endpoint.id == id)
    {
        endpoint.base_url = base_url.to_string();
        endpoint.protocol = protocol;
    } else {
        provider.endpoints.push(ProviderEndpoint {
            id: id.to_string(),
            base_url: base_url.to_string(),
            protocol,
            environment_keys: Vec::new(),
        });
    }
}

fn upsert_agent_alias(provider: &mut ProviderDefinition, agent: &str, alias: &str, endpoint: &str) {
    if provider.agent_aliases.iter().any(|item| {
        normalized_id(&item.agent) == normalized_id(agent)
            && normalized_id(&item.alias) == normalized_id(alias)
    }) {
        return;
    }
    provider.agent_aliases.push(ProviderAgentAlias {
        agent: agent.to_string(),
        alias: alias.to_string(),
        endpoint: Some(endpoint.to_string()),
        environment_keys: Vec::new(),
    });
}

fn default_protocol_for_provider(id: &str) -> WireProtocol {
    match normalized_id(id).as_str() {
        "anthropic" => WireProtocol::AnthropicMessages,
        _ => WireProtocol::ChatCompletions,
    }
}

pub fn protocol_for_api(api: &str) -> Option<WireProtocol> {
    match api {
        "openai-responses" | "openai-codex-responses" | "azure-openai-responses" => {
            Some(WireProtocol::Responses)
        }
        "openai-completions" | "openai-chat-completions" | "openai-chat" | "openai_chat" => {
            Some(WireProtocol::ChatCompletions)
        }
        "anthropic" | "anthropic-messages" | "anthropic_messages" => {
            Some(WireProtocol::AnthropicMessages)
        }
        "codex-responses" | "codex_responses" => Some(WireProtocol::Responses),
        _ => None,
    }
}

fn normalized_id(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn normalized_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_agent_alias_to_endpoint_variant() {
        let mut candidate = ProviderCandidate::new("pi");
        candidate.agent_provider_id = Some("minimax-cn".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.provider_id.as_deref(), Some("minimax"));
        assert_eq!(provider.endpoint_variant.as_deref(), Some("cn-anthropic"));
        assert_eq!(
            provider.base_url.as_deref(),
            Some("https://api.minimaxi.com/anthropic")
        );
        assert_eq!(provider.resolution_status, ProviderResolutionStatus::Known);
    }

    #[test]
    fn protocol_hint_selects_matching_endpoint_for_agent_alias() {
        let mut candidate = ProviderCandidate::new("openclaw");
        candidate.agent_provider_id = Some("opencode-go".to_string());
        candidate.protocol_hint = Some(WireProtocol::AnthropicMessages);
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.provider_id.as_deref(), Some("opencode"));
        assert_eq!(provider.provider_display_name.as_deref(), Some("OpenCode"));
        assert_eq!(provider.endpoint_variant.as_deref(), Some("go-anthropic"));
        assert_eq!(
            provider.base_url.as_deref(),
            Some("https://opencode.ai/zen/go")
        );
    }

    #[test]
    fn embedded_catalog_has_valid_metadata_and_references() {
        let registry = ProviderRegistry::builtin();
        assert_eq!(registry.schema_version(), 1);
        assert!(!registry.revision().is_empty());
        registry.validate().unwrap();
    }

    #[test]
    fn identifies_known_provider_from_endpoint() {
        let mut candidate = ProviderCandidate::new("sentra");
        candidate.configured_base_url = Some("https://api.deepseek.com/".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.provider_id.as_deref(), Some("deepseek"));
        assert_eq!(provider.resolution_status, ProviderResolutionStatus::Known);
        assert_eq!(
            provider.base_url_source,
            Some(ProviderFieldSource::Configured)
        );
    }

    #[test]
    fn keeps_unknown_and_custom_providers_visible() {
        let mut unknown = ProviderCandidate::new("openclaw");
        unknown.agent_provider_id = Some("future-provider".to_string());
        let unknown = ProviderRegistry::builtin().resolve(unknown);
        assert_eq!(unknown.resolution_status, ProviderResolutionStatus::Unknown);
        assert_eq!(unknown.raw_provider_id.as_deref(), Some("future-provider"));

        let mut custom = ProviderCandidate::new("openclaw");
        custom.agent_provider_id = Some("corp-gateway".to_string());
        custom.configured_base_url = Some("https://llm.example.test/v1".to_string());
        let custom = ProviderRegistry::builtin().resolve(custom);
        assert_eq!(custom.resolution_status, ProviderResolutionStatus::Custom);
        assert_eq!(
            custom.base_url.as_deref(),
            Some("https://llm.example.test/v1")
        );
    }
}
