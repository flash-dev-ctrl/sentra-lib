use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::agents::qoder::surface;
use crate::interfaces::{Asset, AssetType, McpData, McpType};
use crate::utils::{SqliteDatabase, parse_mcp_servers, read_json_file, sanitize_mcp_data};

#[derive(Debug, Clone)]
pub(super) struct McpAsset {
    core: AssetCore,
}

impl McpAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(McpAsset, AssetType::Mcp, Vec<McpData>);

impl Asset<Vec<McpData>> for McpAsset {
    fn get_data(&self) -> SentraResult<Vec<McpData>> {
        mcp_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn mcp_data(agent_name: &str, agent_home: &Path) -> SentraResult<Vec<McpData>> {
    let mut out = Vec::new();
    let app_roots = surface::work_data_roots(agent_name, agent_home);
    let mut roots = vec![agent_home.to_path_buf()];
    roots.extend(app_roots.iter().cloned());

    for root in roots {
        for file in [
            "settings.json",
            "settings.local.json",
            "mcp.json",
            "connector.json",
            "connector.custom.json",
            "com.qoder.work.connector.json",
            ".builtin-defaults-state-v3.json",
        ] {
            let path = root.join(file);
            let Some(config) = read_json_file(path)? else {
                continue;
            };
            extend_standard_mcp(&mut out, &config);
            extend_connector_mcp(&mut out, &config);
            extend_builtin_mcp_state(&mut out, &config);
        }
        if let Some(config) = read_json_file(root.join("mcp-adaptor.config"))? {
            if let Some(server) = adaptor_server(agent_name, &config) {
                push_unique(&mut out, server);
            }
        }
    }

    for app_root in app_roots {
        extend_database_mcp(&mut out, &app_root)?;
        extend_log_mcp(&mut out, &app_root);
    }

    Ok(out)
}

fn extend_standard_mcp(out: &mut Vec<McpData>, config: &Value) {
    for raw in [
        config.get("mcpServers"),
        config.get("mcp"),
        config.get("servers"),
    ]
    .into_iter()
    .flatten()
    {
        for server in parse_mcp_servers(raw, None) {
            push_unique(out, server);
        }
    }
}

fn extend_connector_mcp(out: &mut Vec<McpData>, config: &Value) {
    let data = config.get("data").unwrap_or(config);
    if let Some(server) = connector_server(data) {
        push_unique(out, server);
    }
    for key in ["items", "servers"] {
        if let Some(items) = data.get(key).and_then(Value::as_array) {
            for item in items {
                if let Some(server) = connector_server(item) {
                    push_unique(out, server);
                }
            }
        }
    }
    if let Some(custom) = data.get("custom").and_then(Value::as_object) {
        for server in parse_mcp_servers(&Value::Object(custom.clone()), None) {
            push_unique(out, server);
        }
    }
}

fn extend_builtin_mcp_state(out: &mut Vec<McpData>, config: &Value) {
    let Some(enabled) = config
        .get("enabledBuiltinMcpServers")
        .and_then(Value::as_array)
    else {
        return;
    };
    for name in enabled.iter().filter_map(Value::as_str) {
        push_unique(out, connector_marker(name, true));
    }
}

fn extend_database_mcp(out: &mut Vec<McpData>, app_root: &Path) -> SentraResult<()> {
    let Some(database) = SqliteDatabase::open_read_only(app_root.join("data").join("agents.db"))?
    else {
        return Ok(());
    };

    if database.table_exists("app_settings")? {
        let settings = database.query_map(
            "SELECT key, value FROM app_settings \
             WHERE key = 'dws:enabled' \
                OR key = 'msConnectorEnabled' \
                OR key LIKE 'channel:%' \
                OR key LIKE 'qoderwork.settings.connector.%'",
            rusqlite::params![],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;
        for (key, value) in settings {
            extend_app_setting_mcp(out, &key, &value);
        }
    }

    extend_oauth_token_table(out, &database, "mcp_oauth_tokens")?;
    extend_oauth_token_table(out, &database, "mcp_oauth_tokens_by_user")?;
    Ok(())
}

fn extend_oauth_token_table(
    out: &mut Vec<McpData>,
    database: &SqliteDatabase,
    table: &str,
) -> SentraResult<()> {
    if !database.table_exists(table)? {
        return Ok(());
    }
    let names = database.query_map(
        &format!("SELECT server_name FROM {table}"),
        rusqlite::params![],
        |row| row.get::<_, String>(0),
    )?;
    for name in names {
        push_unique(out, connector_marker(&name, true));
    }
    Ok(())
}

fn extend_app_setting_mcp(out: &mut Vec<McpData>, key: &str, value: &str) {
    if key == "qoderwork.settings.connector.market" {
        if let Ok(config) = serde_json::from_str::<Value>(value) {
            extend_enabled_connector_items(out, &config);
        }
        return;
    }
    if let Some(server) = app_setting_server(key, value) {
        push_unique(out, server);
    }
    if let Ok(config) = serde_json::from_str::<Value>(value) {
        extend_enabled_connector_items(out, &config);
    }
}

fn app_setting_server(key: &str, value: &str) -> Option<McpData> {
    if !enabled_setting(value).unwrap_or(false) {
        return None;
    }
    let name = match key {
        "dws:enabled" => "dingtalk",
        "msConnectorEnabled" => "microsoft-365",
        _ => key
            .strip_prefix("channel:")
            .or_else(|| connector_key_suffix(key))?,
    };
    Some(connector_marker(name, true))
}

fn connector_key_suffix(key: &str) -> Option<&str> {
    for prefix in [
        "qoderwork.settings.connector.builtin.",
        "qoderwork.settings.connector.custom.",
        "qoderwork.settings.connector.market.",
    ] {
        if let Some(suffix) = key.strip_prefix(prefix) {
            let suffix = suffix.trim();
            if !suffix.is_empty() && suffix != "global" {
                return Some(suffix);
            }
        }
    }
    None
}

fn enabled_setting(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => serde_json::from_str::<Value>(value)
            .ok()
            .and_then(|value| active_state(&value)),
    }
}

fn extend_enabled_connector_items(out: &mut Vec<McpData>, value: &Value) {
    let data = value.get("data").unwrap_or(value);
    if let Some(items) = data.as_array() {
        for item in items {
            extend_enabled_connector_item(out, item);
        }
        return;
    }
    let Some(map) = data.as_object() else {
        return;
    };
    if active_state(data).unwrap_or(false) {
        if let Some(server) = connector_server(data)
            .or_else(|| connector_name(data).map(|name| connector_marker(&name, true)))
        {
            push_unique(out, server);
        }
    }
    for key in [
        "items",
        "servers",
        "connectors",
        "builtin",
        "market",
        "custom",
    ] {
        if let Some(child) = map.get(key) {
            extend_enabled_connector_items(out, child);
        }
    }
    for (name, child) in map {
        if matches!(
            name.as_str(),
            "items" | "servers" | "connectors" | "builtin" | "market" | "custom"
        ) {
            continue;
        }
        if active_state(child).unwrap_or(false) {
            if let Some(server) = connector_server_with_default_name(name, child) {
                push_unique(out, server);
            }
        }
    }
}

fn extend_enabled_connector_item(out: &mut Vec<McpData>, item: &Value) {
    if !active_state(item).unwrap_or(false) {
        return;
    }
    if let Some(server) = connector_server(item)
        .or_else(|| connector_name(item).map(|name| connector_marker(&name, true)))
    {
        push_unique(out, server);
    }
}

fn active_state(value: &Value) -> Option<bool> {
    if let Some(enabled) = value.as_bool() {
        return Some(enabled);
    }
    if let Some(items) = value.as_array() {
        return items
            .iter()
            .filter_map(active_state)
            .find(|enabled| *enabled)
            .or(Some(false));
    }
    let object = value.as_object()?;
    if let Some(enabled) = object.get("enabled").and_then(Value::as_bool) {
        return Some(enabled);
    }
    if let Some(disabled) = object.get("disabled").and_then(Value::as_bool) {
        return Some(!disabled);
    }
    for key in [
        "authorized",
        "authenticated",
        "connected",
        "token_valid",
        "refresh_token_valid",
    ] {
        if object.get(key).and_then(Value::as_bool) == Some(true) {
            return Some(true);
        }
    }
    match object.get("status").and_then(Value::as_str) {
        Some("enabled" | "connected" | "authorized" | "authenticated" | "active" | "success") => {
            Some(true)
        }
        Some("disabled" | "disconnected" | "unauthorized" | "failed" | "error") => Some(false),
        _ => None,
    }
}

fn extend_log_mcp(out: &mut Vec<McpData>, app_root: &Path) {
    for path in recent_log_files(app_root.join("logs")) {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        let lower = content.to_ascii_lowercase();
        if lower.contains("[dwsauth]")
            && (lower.contains("login completed successfully")
                || lower.contains("\"token_valid\": true")
                || lower.contains("login to dingtalk"))
        {
            push_unique(out, connector_marker("dingtalk", true));
            return;
        }
    }
}

fn recent_log_files(root: PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_log_files(&root, &mut files, 0);
    files.sort();
    files.reverse();
    files.truncate(16);
    files
}

fn collect_log_files(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 {
        return;
    }
    for entry in fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_dir() {
            collect_log_files(&path, files, depth + 1);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("log") {
            files.push(path);
        }
    }
}

fn adaptor_server(agent_name: &str, config: &Value) -> Option<McpData> {
    let url = string_field(config, &["url"])?;
    let mut data = McpData {
        name: if surface::is_cn(agent_name) {
            "qoderwork-cn-connector".to_string()
        } else {
            "qoderwork-connector".to_string()
        },
        mcp_type: Some(McpType::Http),
        url: Some(url),
        enabled: Some(true),
        ..McpData::default()
    };
    sanitize_mcp_data(&mut data);
    Some(data)
}

fn connector_server(value: &Value) -> Option<McpData> {
    connector_server_with_default_name(&connector_name(value)?, value)
}

fn connector_server_with_default_name(default_name: &str, value: &Value) -> Option<McpData> {
    let name = connector_name(value).unwrap_or_else(|| default_name.to_string());
    let config = value
        .get("config")
        .or_else(|| value.get("mcpConfig").and_then(|mcp| mcp.get("config")))?;
    let mut server = config.as_object().cloned().unwrap_or_else(Map::new);
    if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
        server.insert("enabled".to_string(), Value::Bool(enabled));
    }
    if matches!(
        value.get("status").and_then(Value::as_str),
        Some("disabled" | "disconnected")
    ) {
        server.insert("enabled".to_string(), Value::Bool(false));
    }
    let mut servers = Map::new();
    servers.insert(name, Value::Object(server));
    parse_mcp_servers(&Value::Object(servers), None)
        .into_iter()
        .next()
}

fn connector_name(value: &Value) -> Option<String> {
    string_field(
        value,
        &[
            "name",
            "id",
            "serverName",
            "displayName",
            "_builtinId",
            "_marketItemId",
            "key",
        ],
    )
}

fn connector_marker(name: &str, enabled: bool) -> McpData {
    McpData {
        name: name.to_string(),
        enabled: Some(enabled),
        ..McpData::default()
    }
}

fn push_unique(out: &mut Vec<McpData>, server: McpData) {
    if out.iter().any(|item| item.name == server.name) {
        return;
    }
    out.push(server);
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_adaptor_config_without_exposing_token() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("mcp-adaptor.config"),
            r#"{"url":"https://user:pass@example.test/mcp?token=secret","token":"secret"}"#,
        )
        .unwrap();

        let servers = mcp_data("qoder-work", dir.path()).unwrap();

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "qoderwork-connector");
        assert_eq!(servers[0].mcp_type, Some(McpType::Http));
        assert_eq!(
            servers[0].url.as_deref(),
            Some("https://****@example.test/mcp?token=****")
        );
        assert_eq!(servers[0].env, None);
    }

    #[test]
    fn reads_connector_custom_items() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("connector.custom.json"),
            serde_json::to_vec(&serde_json::json!({
                "success": true,
                "data": {
                    "items": [
                        {
                            "name": "filesystem",
                            "enabled": true,
                            "config": {
                                "command": "npx",
                                "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                            }
                        },
                        {
                            "name": "remote",
                            "enabled": false,
                            "config": {"url": "https://example.test/sse"}
                        }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let servers = mcp_data("qoder-work", dir.path()).unwrap();

        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "filesystem");
        assert_eq!(servers[0].command.as_deref(), Some("npx"));
        assert_eq!(servers[0].enabled, Some(true));
        assert_eq!(servers[1].name, "remote");
        assert_eq!(servers[1].mcp_type, Some(McpType::Sse));
        assert_eq!(servers[1].enabled, Some(false));
    }

    #[test]
    fn reads_dingtalk_from_qoderwork_database_state() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".qoderwork");
        let app_root = dir.path().join("AppData").join("Roaming").join("QoderWork");
        std::fs::create_dir_all(&agent_home).unwrap();
        std::fs::create_dir_all(app_root.join("data")).unwrap();
        let database = rusqlite::Connection::open(app_root.join("data").join("agents.db")).unwrap();
        database
            .execute(
                "CREATE TABLE app_settings (key text PRIMARY KEY NOT NULL, value text NOT NULL)",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO app_settings (key, value) VALUES ('dws:enabled', 'true')",
                [],
            )
            .unwrap();

        let servers = mcp_data("qoder-work", &agent_home).unwrap();

        let dingtalk = servers
            .iter()
            .find(|server| server.name == "dingtalk")
            .expect("missing dingtalk connector");
        assert_eq!(dingtalk.enabled, Some(true));
        assert_eq!(dingtalk.url, None);
        assert_eq!(dingtalk.env, None);
    }

    #[test]
    fn skips_market_candidates_without_active_state() {
        let mut servers = Vec::new();
        extend_app_setting_mcp(
            &mut servers,
            "qoderwork.settings.connector.market",
            r#"{"items":[{"name":"candidate","config":{"url":"https://example.test/mcp"}},{"name":"active","enabled":true,"config":{"url":"https://active.example.test/mcp"}}]}"#,
        );

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "active");
    }
}
