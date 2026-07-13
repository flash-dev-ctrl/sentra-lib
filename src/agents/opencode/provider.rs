use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{
    Asset, AssetMutationErrorCode, AssetMutationResult, AssetType, ProviderData, ProviderModel,
    ProviderProbeRequest, ProviderType,
};
use crate::providers::{
    ProviderActivationStatus, ProviderCandidate, ProviderFieldSource, ProviderRegistry,
    protocol_for_api,
};
use crate::utils::protocol::WireProtocol;
use crate::utils::{backup_file, mask_secret, read_json_file, write_json_file};

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

    pub fn get_request(&self, model: &str) -> Vec<ProviderProbeRequest> {
        vec![ProviderProbeRequest {
            protocol: WireProtocol::ChatCompletions,
            body: Some(opencode_probe_body(model).to_string()),
            prompt: None,
        }]
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

    fn set_data(&self, value: ProviderData) -> SentraResult<AssetMutationResult> {
        set_provider_data(self.core.agent_home(), value)
    }

    fn del_data(&self, _item: &ProviderData) -> SentraResult<AssetMutationResult> {
        Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenCode provider mutation is not supported",
        ))
    }
}

fn opencode_probe_body(model: &str) -> Value {
    json!({
        "max_tokens": 32000,
        "messages": [
            {
                "content": OPENCODE_TITLE_SYSTEM_PROMPT,
                "role": "system"
            },
            {
                "content": "Generate a title for this conversation:\n",
                "role": "user"
            },
            {
                "content": "hello",
                "role": "user"
            }
        ],
        "model": model,
        "stream": true,
        "stream_options": {
            "include_usage": true
        }
    })
}

const OPENCODE_TITLE_SYSTEM_PROMPT: &str = r#"You are a title generator. You output ONLY a thread title. Nothing else.

<task>
Generate a brief title that would help the user find this conversation later.

Follow all rules in <rules>
Use the <examples> so you know what a good title looks like.
Your output must be:
- A single line
- ≤50 characters
- No explanations
</task>

<rules>
- you MUST use the same language as the user message you are summarizing
- Title must be grammatically correct and read naturally - no word salad
- Never include tool names in the title (e.g. "read tool", "bash tool", "edit tool")
- Focus on the main topic or question the user needs to retrieve
- Vary your phrasing - avoid repetitive patterns like always starting with "Analyzing"
- When a file is mentioned, focus on WHAT the user wants to do WITH the file, not just that they shared it
- Keep exact: technical terms, numbers, filenames, HTTP codes
- Remove: the, this, my, a, an
- Never assume tech stack
- Never use tools
- NEVER respond to questions, just generate a title for the conversation
- The title should NEVER include "summarizing" or "generating" when generating a title
- DO NOT SAY YOU CANNOT GENERATE A TITLE OR COMPLAIN ABOUT THE INPUT
- Always output something meaningful, even if the input is minimal.
- If the user message is short or conversational (e.g. "hello", "lol", "what's up", "hey"):
  → create a title that reflects the user's tone or intent (such as Greeting, Quick check-in, Light chat, Intro message, etc.)
</rules>

<examples>
"debug 500 errors in production" → Debugging production 500 errors
"refactor user service" → Refactoring user service
"why is app.js failing" → app.js failure investigation
"implement rate limiting" → Rate limiting implementation
"how do I connect postgres to my API" → Postgres API connection
"best practices for React hooks" → React hooks best practices
"@src/auth.ts can you add refresh token support" → Auth refresh token support
"@utils/parser.ts this is broken" → Parser bug fix
"look at @config.json" → Config review
"@App.tsx add dark mode toggle" → Dark mode toggle in App
</examples>
"#;

fn set_provider_data(
    agent_home: &std::path::Path,
    provider: ProviderData,
) -> SentraResult<AssetMutationResult> {
    if provider.provider_type != ProviderType::Gateway {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::Unsupported,
            "OpenCode account provider mutation is not supported",
        ));
    }
    let Some(model) = provider
        .models
        .iter()
        .find(|item| item.enabled)
        .or_else(|| provider.models.first())
        .filter(|model| !model.id.trim().is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider model is required",
        ));
    };
    let Some(base_url) = provider
        .base_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider baseUrl is required",
        ));
    };
    let Some(api_key) = provider
        .api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(AssetMutationResult::unchanged(
            AssetMutationErrorCode::MissingRequiredField,
            "provider apiKey is required",
        ));
    };

    let provider_id = provider_config_id(&provider);
    let path = agent_home.join("opencode.json");
    let mut config = read_json_file(&path)?.unwrap_or_else(|| json!({}));
    if !config.is_object() {
        config = json!({});
    }
    if config.get("$schema").is_none() {
        config["$schema"] = json!("https://opencode.ai/config.json");
    }
    config["model"] = json!(model_ref(&provider_id, &model.id));

    let providers = object_field(&mut config, "provider");
    let entry = providers
        .entry(provider_id.clone())
        .or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    let entry = entry.as_object_mut().expect("provider entry is object");
    entry.insert(
        "name".to_string(),
        json!(provider_display_name(&provider, &provider_id)),
    );
    entry.insert(
        "npm".to_string(),
        json!(npm_package_for_protocol(provider.protocol)),
    );
    entry.insert(
        "api".to_string(),
        json!(api_for_protocol(provider.protocol)),
    );

    let options = object_entry(entry, "options");
    options.insert("baseURL".to_string(), json!(base_url));
    options.insert("apiKey".to_string(), json!(api_key));

    let models = object_entry(entry, "models");
    let model_config = models.entry(model.id.clone()).or_insert_with(|| json!({}));
    if !model_config.is_object() {
        *model_config = json!({});
    }
    model_config["name"] = json!(model.name.as_deref().unwrap_or(&model.id));

    backup_file(&path)?;
    write_json_file(path, &config)?;
    Ok(AssetMutationResult::changed())
}

fn provider_data(agent_home: &std::path::Path) -> SentraResult<Vec<ProviderData>> {
    let auth_keys = auth_api_keys(agent_home)?;
    let mut results = Vec::new();
    let mut provider_ids = Vec::new();

    for path in crate::agents::opencode::config_files(agent_home) {
        let Some(config) = read_json_file(path)? else {
            continue;
        };
        let active_model = config.get("model").and_then(Value::as_str);
        let active_model_ref = active_model.and_then(split_model_ref);
        let Some(providers) = config
            .get("provider")
            .or_else(|| config.get("providers"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (provider_id, raw) in providers {
            if provider_ids.iter().any(|id| id == provider_id) {
                continue;
            }
            let raw = raw.as_object();
            let options = raw
                .and_then(|raw| raw.get("options"))
                .and_then(Value::as_object);
            let mut models = provider_models(raw.and_then(|raw| raw.get("models")));
            if let Some((active_provider, active_model)) = active_model_ref
                && active_provider == provider_id
            {
                ensure_model(&mut models, active_model, true);
            }

            let explicit_enabled = raw
                .and_then(|raw| raw.get("enabled"))
                .and_then(Value::as_bool);
            let configured_protocol = raw
                .and_then(|raw| {
                    raw.get("api")
                        .or_else(|| raw.get("protocol"))
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    options.and_then(|options| {
                        options
                            .get("api")
                            .or_else(|| options.get("protocol"))
                            .and_then(Value::as_str)
                    })
                })
                .and_then(parse_protocol);
            let inferred_protocol =
                configured_protocol.or_else(|| npm_protocol(raw.and_then(|raw| raw.get("npm"))));

            let mut candidate = ProviderCandidate::new("opencode");
            candidate.agent_provider_id = Some(provider_id.clone());
            candidate.display_name = Some(
                raw.and_then(|raw| {
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .and_then(Value::as_str)
                })
                .unwrap_or(provider_id)
                .to_string(),
            );
            candidate.configured_base_url = base_url(raw, options);
            candidate.protocol_hint = inferred_protocol;
            candidate.protocol_source = inferred_protocol.map(|_| {
                if configured_protocol.is_some() {
                    ProviderFieldSource::Configured
                } else {
                    ProviderFieldSource::Inferred
                }
            });
            candidate.api_key = configured_api_key(raw, options)
                .or_else(|| auth_keys.get(provider_id).cloned())
                .or_else(|| environment_api_key(provider_id));
            candidate.activation = provider_activation(
                active_model_ref.map(|(provider, _)| provider),
                provider_id,
                explicit_enabled,
            );
            candidate.models = models;
            results.push(ProviderRegistry::builtin().resolve(candidate));
            provider_ids.push(provider_id.clone());
        }
    }

    for (provider_id, api_key) in auth_keys {
        if provider_ids.iter().any(|id| id == &provider_id) {
            continue;
        }
        let mut candidate = ProviderCandidate::new("opencode");
        candidate.agent_provider_id = Some(provider_id.clone());
        candidate.display_name = Some(provider_id.clone());
        candidate.api_key = Some(api_key);
        candidate.activation = ProviderActivationStatus::Unknown;
        results.push(ProviderRegistry::builtin().resolve(candidate));
    }

    Ok(results)
}

fn object_field<'a>(value: &'a mut Value, key: &str) -> &'a mut Map<String, Value> {
    if !value.get(key).is_some_and(Value::is_object) {
        value[key] = json!({});
    }
    value[key].as_object_mut().expect("field is object")
}

fn object_entry<'a>(value: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let entry = value.entry(key.to_string()).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    entry.as_object_mut().expect("entry is object")
}

fn provider_config_id(provider: &ProviderData) -> String {
    if let Some(value) = provider
        .raw_provider_id
        .as_deref()
        .or(provider.provider_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return value.to_string();
    }
    if let Some(value) = (!provider.name.trim().is_empty())
        .then_some(provider.name.as_str())
        .map(slug)
        .filter(|value| !value.is_empty())
    {
        return value;
    }
    provider
        .base_url
        .as_deref()
        .and_then(host_from_url)
        .map(|value| slug(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "custom".to_string())
}

fn provider_display_name(provider: &ProviderData, provider_id: &str) -> String {
    provider
        .provider_display_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| (!provider.name.trim().is_empty()).then(|| provider.name.clone()))
        .unwrap_or_else(|| provider_id.to_string())
}

fn model_ref(provider_id: &str, model_id: &str) -> String {
    let prefix = format!("{provider_id}/");
    if model_id.starts_with(&prefix) {
        model_id.to_string()
    } else {
        format!("{provider_id}/{model_id}")
    }
}

fn npm_package_for_protocol(protocol: Option<WireProtocol>) -> &'static str {
    match protocol {
        Some(WireProtocol::AnthropicMessages) => "@ai-sdk/anthropic",
        _ => "@ai-sdk/openai",
    }
}

fn api_for_protocol(protocol: Option<WireProtocol>) -> &'static str {
    match protocol {
        Some(WireProtocol::AnthropicMessages) => "anthropic",
        _ => "openai-chat-completions",
    }
}

fn host_from_url(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())?;
    if let Some(ipv6) = authority.strip_prefix('[') {
        return ipv6.split_once(']').map(|(host, _)| host.to_string());
    }
    authority
        .split(':')
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string)
}

fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
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
            .map(|(id, raw)| {
                model(
                    id,
                    raw.get("name")
                        .or_else(|| raw.get("displayName"))
                        .or_else(|| raw.get("label"))
                        .and_then(Value::as_str)
                        .or(Some(id)),
                    raw.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                )
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

fn split_model_ref(value: &str) -> Option<(&str, &str)> {
    let (provider, model) = value.split_once('/')?;
    (!provider.is_empty() && !model.is_empty()).then_some((provider, model))
}

fn base_url(
    raw: Option<&serde_json::Map<String, Value>>,
    options: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    string_field(options, &["baseURL", "baseUrl", "base_url", "url"])
        .or_else(|| string_field(raw, &["baseURL", "baseUrl", "base_url", "url"]))
}

fn configured_api_key(
    raw: Option<&serde_json::Map<String, Value>>,
    options: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    string_field(options, &["apiKey", "api_key", "key", "token"])
        .or_else(|| string_field(raw, &["apiKey", "api_key", "key", "token"]))
        .and_then(|value| resolve_secret(&value))
        .and_then(|value| mask_secret(Some(&value)))
}

fn string_field(raw: Option<&serde_json::Map<String, Value>>, keys: &[&str]) -> Option<String> {
    let raw = raw?;
    keys.iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn parse_protocol(value: &str) -> Option<WireProtocol> {
    protocol_for_api(value).or_else(|| value.parse().ok())
}

fn npm_protocol(raw: Option<&Value>) -> Option<WireProtocol> {
    let value = raw.and_then(Value::as_str)?.to_ascii_lowercase();
    if value.contains("anthropic") {
        Some(WireProtocol::AnthropicMessages)
    } else if value.contains("openai-compatible") || value.contains("openai") {
        Some(WireProtocol::ChatCompletions)
    } else {
        None
    }
}

fn provider_activation(
    active_provider: Option<&str>,
    provider_id: &str,
    explicit_enabled: Option<bool>,
) -> ProviderActivationStatus {
    match active_provider {
        Some(active) if active == provider_id => ProviderActivationStatus::Active,
        Some(_) => ProviderActivationStatus::Inactive,
        None => match explicit_enabled {
            Some(true) => ProviderActivationStatus::Active,
            Some(false) => ProviderActivationStatus::Inactive,
            None => ProviderActivationStatus::Unknown,
        },
    }
}

fn auth_api_keys(agent_home: &std::path::Path) -> SentraResult<BTreeMap<String, String>> {
    let Some(auth) =
        read_json_file(crate::agents::opencode::data_home(agent_home).join("auth.json"))?
    else {
        return Ok(BTreeMap::new());
    };
    let mut keys = BTreeMap::new();
    for container in ["provider", "providers", "credential", "credentials"] {
        if let Some(map) = auth.get(container).and_then(Value::as_object) {
            collect_provider_secret_map(map, &mut keys);
        }
    }
    if let Some(map) = auth.as_object() {
        for (provider_id, value) in map {
            if value.is_object()
                && let Some(secret) = secret_from_value(value)
            {
                keys.insert(provider_id.clone(), secret);
            }
        }
    }
    Ok(keys)
}

fn collect_provider_secret_map(
    map: &serde_json::Map<String, Value>,
    keys: &mut BTreeMap<String, String>,
) {
    for (provider_id, value) in map {
        if let Some(secret) = secret_from_value(value) {
            keys.insert(provider_id.clone(), secret);
        }
    }
}

fn secret_from_value(value: &Value) -> Option<String> {
    let raw = if let Some(value) = value.as_str() {
        Some(value)
    } else {
        let value = value.as_object()?;
        [
            "apiKey",
            "api_key",
            "key",
            "token",
            "access",
            "accessToken",
            "access_token",
            "value",
        ]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
    }?;
    resolve_secret(raw).and_then(|value| mask_secret(Some(&value)))
}

fn resolve_secret(value: &str) -> Option<String> {
    if value.trim().is_empty() || value.starts_with('!') {
        return None;
    }
    if let Some(name) = value
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
    {
        return std::env::var(name).ok().filter(|value| !value.is_empty());
    }
    if let Some(name) = value.strip_prefix('$')
        && !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return std::env::var(name).ok().filter(|value| !value.is_empty());
    }
    Some(value.to_string())
}

fn environment_api_key(provider_id: &str) -> Option<String> {
    ProviderRegistry::builtin()
        .environment_keys("opencode", provider_id)
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
        .and_then(|value| mask_secret(Some(&value)))
}
