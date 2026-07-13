use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use url::Url;

use crate::interfaces::{ProviderData, ProviderModel, ProviderType};
use crate::utils::protocol::WireProtocol;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRouteStatus {
    Official,
    Unverified,
    RelayCandidate,
    ProviderMismatch,
    Custom,
    Ambiguous,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderEndpointTrust {
    VendorVerified,
    #[default]
    ModelsDev,
    Unverified,
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
    #[serde(default)]
    pub configuration_keys: Vec<String>,
    #[serde(default)]
    pub allows_custom_endpoints: bool,
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
    pub base_url: Option<String>,
    #[serde(default)]
    pub base_url_aliases: Vec<String>,
    #[serde(default)]
    pub match_rules: Vec<ProviderEndpointMatchRule>,
    #[serde(default)]
    pub trust: ProviderEndpointTrust,
    #[serde(default)]
    pub protocol: Option<WireProtocol>,
    #[serde(default)]
    pub environment_keys: Vec<String>,
    #[serde(default)]
    pub configuration_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderEndpointMatchRule {
    #[serde(default = "default_https_scheme")]
    pub scheme: String,
    pub host: Option<String>,
    pub host_suffix: Option<String>,
    pub path_prefix: Option<String>,
    pub path_suffix: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAgentAlias {
    pub agent: String,
    pub alias: String,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub environment_keys: Vec<String>,
    #[serde(default)]
    pub configuration_keys: Vec<String>,
}

fn default_https_scheme() -> String {
    "https".to_string()
}

#[derive(Debug, Deserialize)]
struct ProviderCatalogFile {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    revision: String,
    providers: Vec<ProviderDefinition>,
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
            let catalog: ProviderCatalogFile = serde_json::from_str(include_str!("catalog.json"))
                .expect("embedded provider catalog must be valid");
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
                if endpoint.base_url.is_none() && endpoint.match_rules.is_empty() {
                    return Err(format!(
                        "endpoint {} for provider {} has no base URL or match rule",
                        endpoint.id, provider.id
                    ));
                }
                for value in endpoint
                    .base_url
                    .iter()
                    .chain(endpoint.base_url_aliases.iter())
                {
                    validate_catalog_url(value).map_err(|reason| {
                        format!(
                            "invalid URL for endpoint {} / {}: {reason}",
                            provider.id, endpoint.id
                        )
                    })?;
                }
                for rule in &endpoint.match_rules {
                    validate_match_rule(rule).map_err(|reason| {
                        format!(
                            "invalid match rule for endpoint {} / {}: {reason}",
                            provider.id, endpoint.id
                        )
                    })?;
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
        let parsed_url = Url::parse(base_url.trim()).ok();
        let normalized_base_url = parsed_url.as_ref().and_then(normalized_url);
        self.providers
            .iter()
            .flat_map(|provider| {
                provider
                    .endpoints
                    .iter()
                    .filter(|endpoint| {
                        normalized_base_url.is_some()
                            && endpoint
                                .base_url
                                .as_deref()
                                .and_then(|value| Url::parse(value).ok())
                                .as_ref()
                                .and_then(normalized_url)
                                == normalized_base_url
                            || endpoint
                                .base_url_aliases
                                .iter()
                                .filter_map(|alias| Url::parse(alias).ok())
                                .any(|alias| normalized_url(&alias) == normalized_base_url)
                            || parsed_url.as_ref().is_some_and(|url| {
                                endpoint
                                    .match_rules
                                    .iter()
                                    .any(|rule| endpoint_rule_matches(rule, url))
                            })
                    })
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

    pub fn configuration_keys(&self, agent_id: &str, provider_id: &str) -> Vec<&str> {
        let Some(provider_match) = self.match_by_id(agent_id, provider_id) else {
            return Vec::new();
        };
        let mut keys = Vec::new();
        if let Some(alias) = provider_match.provider.agent_aliases.iter().find(|alias| {
            normalized_id(&alias.agent) == normalized_id(agent_id)
                && normalized_id(&alias.alias) == normalized_id(provider_id)
        }) {
            keys.extend(alias.configuration_keys.iter().map(String::as_str));
        }
        if let Some(endpoint) = provider_match.endpoint {
            keys.extend(endpoint.configuration_keys.iter().map(String::as_str));
        }
        keys.extend(
            provider_match
                .provider
                .configuration_keys
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
                if configured_base_url.is_some() {
                    // An explicit URL must prove its own endpoint identity. Keeping the
                    // provider's default endpoint here would incorrectly label a relay URL
                    // as the official default endpoint.
                    id_match.endpoint = None;
                }
                let matching_endpoints = matches
                    .iter()
                    .filter(|(provider, endpoint)| {
                        provider.id == id_match.provider.id
                            && candidate
                                .protocol_hint
                                .is_none_or(|protocol| endpoint.protocol == Some(protocol))
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
                        .is_none_or(|endpoint| endpoint.protocol != Some(protocol))
                {
                    let protocol_endpoints = id_match
                        .provider
                        .endpoints
                        .iter()
                        .filter(|endpoint| endpoint.protocol == Some(protocol))
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
                                    .is_none_or(|protocol| endpoint.protocol == Some(protocol))
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
        let endpoint_provider_ids = endpoint_matches
            .iter()
            .map(|(provider, _)| provider.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let observed_endpoint_trust = configured_base_url.and_then(|_| {
            let mut trusts = endpoint_matches
                .iter()
                .filter(|(provider, endpoint)| {
                    id_match.is_none_or(|matched| matched.provider.id == provider.id)
                        && candidate
                            .protocol_hint
                            .is_none_or(|protocol| endpoint.protocol == Some(protocol))
                })
                .map(|(_, endpoint)| endpoint.trust)
                .collect::<Vec<_>>();
            let first = trusts.pop()?;
            trusts.iter().all(|trust| *trust == first).then_some(first)
        });
        let route_status = match configured_base_url {
            Some(_) if endpoint_provider_ids.len() > 1 => ProviderRouteStatus::Ambiguous,
            Some(_)
                if let Some(id_match) = id_match
                    && !endpoint_provider_ids.is_empty()
                    && !endpoint_provider_ids.contains(id_match.provider.id.as_str()) =>
            {
                ProviderRouteStatus::ProviderMismatch
            }
            Some(_)
                if observed_endpoint_trust.is_some()
                    && endpoint_provider_ids.len() == 1
                    && id_match.is_none_or(|id_match| {
                        endpoint_provider_ids.contains(id_match.provider.id.as_str())
                    }) =>
            {
                match observed_endpoint_trust {
                    Some(ProviderEndpointTrust::VendorVerified) => ProviderRouteStatus::Official,
                    Some(ProviderEndpointTrust::ModelsDev | ProviderEndpointTrust::Unverified)
                    | None => ProviderRouteStatus::Unverified,
                }
            }
            Some(_) if id_match.is_some_and(|matched| matched.provider.allows_custom_endpoints) => {
                ProviderRouteStatus::Unverified
            }
            Some(_) if id_match.is_some() => ProviderRouteStatus::RelayCandidate,
            Some(_) => ProviderRouteStatus::Custom,
            None => ProviderRouteStatus::Unknown,
        };
        let base_url = configured_base_url
            .map(str::to_string)
            .or_else(|| endpoint.and_then(|endpoint| endpoint.base_url.clone()));
        let protocol = candidate
            .protocol_hint
            .or_else(|| endpoint.and_then(|endpoint| endpoint.protocol));
        let base_url_source = configured_base_url
            .map(|_| ProviderFieldSource::Configured)
            .or_else(|| {
                endpoint.and_then(|endpoint| {
                    endpoint
                        .base_url
                        .as_ref()
                        .map(|_| ProviderFieldSource::Catalog)
                })
            });
        let protocol_source = candidate
            .protocol_hint
            .map(|_| {
                candidate
                    .protocol_source
                    .unwrap_or(ProviderFieldSource::Inferred)
            })
            .or_else(|| {
                endpoint
                    .and_then(|endpoint| endpoint.protocol.map(|_| ProviderFieldSource::Catalog))
            });

        let endpoint_trust = match route_status {
            ProviderRouteStatus::Official => observed_endpoint_trust,
            ProviderRouteStatus::Unverified => {
                observed_endpoint_trust.or(Some(ProviderEndpointTrust::Unverified))
            }
            _ => None,
        };
        let route_reason = match route_status {
            ProviderRouteStatus::Official => None,
            ProviderRouteStatus::Unverified => Some(match observed_endpoint_trust {
                Some(ProviderEndpointTrust::ModelsDev) => {
                    "configured endpoint is cataloged from Models.dev but has not been vendor-verified"
                        .to_string()
                }
                Some(ProviderEndpointTrust::Unverified) => {
                    "configured endpoint is explicitly cataloged as unverified".to_string()
                }
                _ => "provider permits custom endpoints; the configured endpoint is not vendor-verified"
                    .to_string(),
            }),
            ProviderRouteStatus::RelayCandidate => Some(
                "provider is known but the configured endpoint is not an official catalog endpoint"
                    .to_string(),
            ),
            ProviderRouteStatus::ProviderMismatch => Some(
                "configured endpoint belongs to a different catalog provider than the configured provider id"
                    .to_string(),
            ),
            ProviderRouteStatus::Custom => {
                Some("configured endpoint is not present in the provider catalog".to_string())
            }
            ProviderRouteStatus::Ambiguous => {
                Some("configured endpoint matches multiple catalog providers".to_string())
            }
            ProviderRouteStatus::Unknown => {
                Some("no configured endpoint was observed; a catalog default is not route evidence".to_string())
            }
        };

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
            route_status,
            route_reason,
            endpoint_trust,
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

fn validate_catalog_url(value: &str) -> Result<(), String> {
    let parsed = Url::parse(value.trim()).map_err(|error| error.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("only HTTP(S) URLs are supported".to_string());
    }
    if parsed.scheme() == "http" && !is_loopback_host(&parsed) {
        return Err("plain HTTP is only allowed for loopback endpoints".to_string());
    }
    normalized_url(&parsed).map(|_| ()).ok_or_else(|| {
        "URL must not contain credentials, query parameters, or fragments".to_string()
    })
}

fn validate_match_rule(rule: &ProviderEndpointMatchRule) -> Result<(), String> {
    if rule.scheme != "https" {
        return Err("dynamic endpoint rules must use HTTPS".to_string());
    }
    if rule.host.is_some() == rule.host_suffix.is_some() {
        return Err("exactly one of host or hostSuffix is required".to_string());
    }
    if let Some(host) = rule.host.as_deref()
        && (host.is_empty() || host != host.to_ascii_lowercase())
    {
        return Err("host must be a non-empty lowercase hostname".to_string());
    }
    if let Some(host) = rule.host.as_deref()
        && Url::parse(&format!("https://{host}/"))
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .as_deref()
            != Some(host)
    {
        return Err("host must be a valid hostname without a port".to_string());
    }
    if let Some(suffix) = rule.host_suffix.as_deref()
        && (!suffix.starts_with('.')
            || !suffix[1..].contains('.')
            || suffix != suffix.to_ascii_lowercase())
    {
        return Err("hostSuffix must be a lowercase DNS suffix beginning with '.'".to_string());
    }
    for path in [rule.path_prefix.as_deref(), rule.path_suffix.as_deref()]
        .into_iter()
        .flatten()
    {
        if !path.starts_with('/')
            || path.trim_end_matches('/').is_empty()
            || path.contains('?')
            || path.contains('#')
        {
            return Err(
                "path constraints must be absolute paths without query or fragment".to_string(),
            );
        }
    }
    Ok(())
}

fn endpoint_rule_matches(rule: &ProviderEndpointMatchRule, url: &Url) -> bool {
    if normalized_url(url).is_none()
        || url.scheme() != rule.scheme
        || url.port_or_known_default() != Some(443)
    {
        return false;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    let host_matches = rule
        .host
        .as_deref()
        .is_some_and(|expected| host == expected)
        || rule
            .host_suffix
            .as_deref()
            .is_some_and(|suffix| host.ends_with(suffix));
    if !host_matches {
        return false;
    }
    let path = url.path().trim_end_matches('/');
    let prefix_matches = rule.path_prefix.as_deref().is_none_or(|prefix| {
        let prefix = prefix.trim_end_matches('/');
        path == prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|remainder| remainder.starts_with('/'))
    });
    let suffix_matches = rule
        .path_suffix
        .as_deref()
        .is_none_or(|suffix| path.ends_with(suffix.trim_end_matches('/')));
    prefix_matches && suffix_matches
}

fn normalized_url(url: &Url) -> Option<String> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.scheme(), "http" | "https")
    {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let port = match (url.scheme(), url.port()) {
        ("https", Some(443)) | ("http", Some(80)) | (_, None) => String::new(),
        (_, Some(port)) => format!(":{port}"),
    };
    let path = url.path().trim_end_matches('/');
    Some(format!("{}://{host}{port}{path}", url.scheme()))
}

fn is_loopback_host(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    })
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
        assert_eq!(provider.route_status, ProviderRouteStatus::Official);
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
        assert_eq!(custom.route_status, ProviderRouteStatus::Custom);
    }

    #[test]
    fn marks_known_provider_with_non_catalog_endpoint_as_relay_candidate() {
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some("openai".to_string());
        candidate.configured_base_url = Some("https://relay.example.test/v1".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.provider_id.as_deref(), Some("openai"));
        assert_eq!(provider.resolution_status, ProviderResolutionStatus::Known);
        assert_eq!(provider.route_status, ProviderRouteStatus::RelayCandidate);
        assert_eq!(provider.endpoint_variant, None);
    }

    #[test]
    fn recognizes_official_shared_base_url_without_protocol_hint() {
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some("openai".to_string());
        candidate.configured_base_url = Some("https://api.openai.com/v1".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.provider_id.as_deref(), Some("openai"));
        assert_eq!(provider.route_status, ProviderRouteStatus::Official);
        assert_eq!(provider.endpoint_variant, None);
    }

    #[test]
    fn marks_provider_id_and_official_endpoint_conflict() {
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some("openai".to_string());
        candidate.configured_base_url = Some("https://api.deepseek.com".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.provider_id.as_deref(), Some("openai"));
        assert_eq!(provider.route_status, ProviderRouteStatus::ProviderMismatch);
        assert_eq!(provider.endpoint_variant, None);
    }

    #[test]
    fn recognizes_official_endpoint_aliases() {
        let matches =
            ProviderRegistry::builtin().identify_by_endpoint("https://api.minimax.io/anthropic/v1");

        assert!(matches.iter().any(|(provider, endpoint)| {
            provider.id == "minimax" && endpoint.id == "global-anthropic"
        }));
    }

    #[test]
    fn catalog_default_without_an_observed_url_is_not_marked_official() {
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some("openai".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.route_status, ProviderRouteStatus::Unknown);
        assert_eq!(provider.endpoint_trust, None);
        assert_eq!(provider.base_url_source, Some(ProviderFieldSource::Catalog));
    }

    #[test]
    fn recognizes_constrained_dynamic_vendor_endpoints() {
        for (provider_id, base_url) in [
            (
                "databricks",
                "https://acme.cloud.databricks.com/ai-gateway/mlflow/v1",
            ),
            (
                "snowflake",
                "https://acme-org.snowflakecomputing.com/api/v2/cortex/v1",
            ),
            (
                "cloudflare-workers-ai",
                "https://api.cloudflare.com/client/v4/accounts/account-id/ai/v1",
            ),
        ] {
            let mut candidate = ProviderCandidate::new("opencode");
            candidate.agent_provider_id = Some(provider_id.to_string());
            candidate.configured_base_url = Some(base_url.to_string());
            candidate.protocol_hint = Some(WireProtocol::ChatCompletions);
            let provider = ProviderRegistry::builtin().resolve(candidate);

            assert_eq!(
                provider.route_status,
                ProviderRouteStatus::Official,
                "{provider_id} should recognize {base_url}"
            );
            assert_eq!(
                provider.endpoint_trust,
                Some(ProviderEndpointTrust::VendorVerified)
            );
        }
    }

    #[test]
    fn dynamic_endpoint_rules_reject_deceptive_hosts_and_paths() {
        for (provider_id, base_url) in [
            (
                "snowflake",
                "https://acme.snowflakecomputing.com.evil.test/api/v2/cortex/v1",
            ),
            (
                "snowflake",
                "https://acme.snowflakecomputing.com/api/v2/cortex/v10",
            ),
            (
                "snowflake",
                "https://user@acme.snowflakecomputing.com/api/v2/cortex/v1",
            ),
            (
                "databricks",
                "https://acme.cloud.databricks.com:444/ai-gateway/mlflow/v1",
            ),
            (
                "cloudflare-workers-ai",
                "https://api.cloudflare.com.evil.test/client/v4/accounts/acct/ai/v1",
            ),
            (
                "cloudflare-workers-ai",
                "https://api.cloudflare.com/client/v4/accounts/acct/ai/v1?route=relay",
            ),
        ] {
            let mut candidate = ProviderCandidate::new("opencode");
            candidate.agent_provider_id = Some(provider_id.to_string());
            candidate.configured_base_url = Some(base_url.to_string());
            let provider = ProviderRegistry::builtin().resolve(candidate);

            assert_eq!(provider.route_status, ProviderRouteStatus::RelayCandidate);
            assert_eq!(provider.endpoint_trust, None);
        }
    }

    #[test]
    fn provider_declared_custom_endpoints_are_unverified_not_official() {
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some("neon".to_string());
        candidate.configured_base_url = Some("https://gateway.example.test/v1".to_string());
        let provider = ProviderRegistry::builtin().resolve(candidate);

        assert_eq!(provider.route_status, ProviderRouteStatus::Unverified);
        assert_eq!(
            provider.endpoint_trust,
            Some(ProviderEndpointTrust::Unverified)
        );
    }

    #[test]
    fn credential_keys_exclude_non_secret_provider_configuration() {
        let registry = ProviderRegistry::builtin();

        assert_eq!(
            registry.environment_keys("pi", "databricks"),
            vec!["DATABRICKS_TOKEN"]
        );
        assert_eq!(
            registry.configuration_keys("pi", "databricks"),
            vec!["DATABRICKS_HOST"]
        );
        assert_eq!(
            registry.environment_keys("pi", "neon"),
            vec!["NEON_AI_GATEWAY_TOKEN"]
        );
        assert_eq!(
            registry.configuration_keys("pi", "neon"),
            vec!["NEON_AI_GATEWAY_BASE_URL"]
        );
    }

    #[test]
    fn catalog_covers_requested_mainstream_providers() {
        let registry = ProviderRegistry::builtin();
        for id in [
            "deepseek",
            "moonshotai",
            "zai",
            "minimax",
            "volcengine",
            "alibaba",
            "tencent",
            "baidu-qianfan",
            "openrouter",
            "opencode",
            "xiaomi",
            "siliconflow",
            "modelscope",
            "stepfun",
            "github-models",
            "cohere",
            "perplexity",
            "deepinfra",
            "ollama",
        ] {
            assert!(registry.lookup(id).is_some(), "missing provider {id}");
        }
    }
}
