use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

use serde_json::Value;

use crate::interfaces::{McpData, McpToolDef, McpType};
use crate::utils::{sanitize_command_args, sanitize_env_value, sanitize_url_credentials};
use crate::{SentraError, SentraResult};

const MCP_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const MCP_RESPONSE_LIMIT_BYTES: usize = 64 * 1024;

pub fn parse_mcp_servers(raw: &serde_json::Value, project: Option<String>) -> Vec<McpData> {
    let Some(map) = raw.as_object() else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, server)| {
            let value = server.as_object();
            let explicit_type = value
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str())
                .and_then(|value| match value {
                    "stdio" => Some(McpType::Stdio),
                    "sse" => Some(McpType::Sse),
                    "http" => Some(McpType::Http),
                    _ => None,
                });
            let command = value
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let url = value
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let mcp_type = explicit_type.or_else(|| {
                if url.is_some() {
                    Some(McpType::Sse)
                } else {
                    Some(McpType::Stdio)
                }
            });
            let args = value
                .and_then(|v| v.get("args"))
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let env = value
                .and_then(|v| v.get("env"))
                .and_then(|v| v.as_object())
                .map(|env| {
                    env.iter()
                        .filter_map(|(key, value)| {
                            value.as_str().map(|value| (key.clone(), value.to_string()))
                        })
                        .collect::<HashMap<_, _>>()
                });
            let disabled = value
                .and_then(|v| v.get("disabled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let enabled_false = value
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool())
                .map(|enabled| !enabled)
                .unwrap_or(false);
            let mut data = McpData {
                name: name.clone(),
                mcp_type,
                command,
                args,
                url,
                env,
                enabled: Some(!disabled && !enabled_false),
                project: project.clone(),
                tools: Vec::new(),
            };
            sanitize_mcp_data(&mut data);
            data
        })
        .collect()
}

pub fn parse_mcp_servers_json(raw: &str, project: Option<String>) -> SentraResult<Vec<McpData>> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|err| SentraError::Message(err.to_string()))?;
    Ok(parse_mcp_servers(&value, project))
}

pub fn hydrate_mcp_tools(mut asset: McpData) -> McpData {
    if asset.tools.is_empty()
        && let Some(url) = asset.url.as_deref()
        && should_fetch_tools(&asset)
        && let Some(tools) = fetch_tools(&asset, url)
    {
        asset.tools = tools;
    }
    asset
}

pub(crate) fn sanitize_mcp_data(data: &mut McpData) {
    data.args = sanitize_command_args(&data.args);
    if let Some(url) = &mut data.url {
        *url = sanitize_url_credentials(url);
    }
    if let Some(env) = &mut data.env {
        for (key, value) in env {
            *value = sanitize_env_value(key, value);
        }
    }
}

fn should_fetch_tools(asset: &McpData) -> bool {
    if asset.enabled == Some(false) || asset.url.is_none() {
        return false;
    }
    !matches!(asset.mcp_type, Some(McpType::Stdio))
}

fn fetch_tools(asset: &McpData, url: &str) -> Option<Vec<McpToolDef>> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(MCP_REQUEST_TIMEOUT)
        .timeout_read(MCP_REQUEST_TIMEOUT)
        .timeout_write(MCP_REQUEST_TIMEOUT)
        .timeout(MCP_REQUEST_TIMEOUT)
        .build();
    if matches!(asset.mcp_type, Some(McpType::Sse)) {
        return fetch_tools_sse(&agent, url)
            .or_else(|| fetch_tools_endpoint(&agent, url))
            .or_else(|| fetch_tools_json_rpc(&agent, url));
    }
    fetch_tools_json_rpc(&agent, url).or_else(|| fetch_tools_endpoint(&agent, url))
}

fn fetch_tools_json_rpc(agent: &ureq::Agent, url: &str) -> Option<Vec<McpToolDef>> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });
    let response = agent
        .post(url)
        .timeout(MCP_REQUEST_TIMEOUT)
        .set("accept", "application/json, text/event-stream")
        .set("content-type", "application/json")
        .send_string(&request.to_string())
        .ok()?;
    if is_unbounded_sse_response(&response) {
        return None;
    }
    read_tools_response(response.into_reader())
}

fn fetch_tools_sse(agent: &ureq::Agent, url: &str) -> Option<Vec<McpToolDef>> {
    let sse_url = sse_url(url)?;
    let response = agent
        .get(&sse_url)
        .timeout(MCP_REQUEST_TIMEOUT)
        .set("accept", "text/event-stream")
        .call()
        .ok()?;
    let mut reader = response.into_reader();
    let endpoint = read_sse_endpoint(&mut reader)?;
    let message_url = endpoint_url(&sse_url, &endpoint)?;
    post_json_rpc(
        agent,
        &message_url,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "sentra-mcp-scan", "version": "0.1.0" }
            }
        }),
    )?;
    let _ = post_json_rpc(
        agent,
        &message_url,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    );
    post_json_rpc(
        agent,
        &message_url,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )?;
    read_tools_response(reader)
}

fn fetch_tools_endpoint(agent: &ureq::Agent, url: &str) -> Option<Vec<McpToolDef>> {
    let tools_url = tools_url(url)?;
    let response = agent
        .get(&tools_url)
        .timeout(MCP_REQUEST_TIMEOUT)
        .set("accept", "application/json, text/event-stream")
        .call()
        .ok()?;
    read_tools_response(response.into_reader())
}

fn is_unbounded_sse_response(response: &ureq::Response) -> bool {
    response
        .header("content-type")
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        && response.header("content-length").is_none()
}

fn post_json_rpc(agent: &ureq::Agent, url: &str, body: &Value) -> Option<()> {
    let response = agent
        .post(url)
        .timeout(MCP_REQUEST_TIMEOUT)
        .set("accept", "application/json, text/event-stream")
        .set("content-type", "application/json")
        .send_string(&body.to_string())
        .ok()?;
    let _ = response
        .into_reader()
        .take(1024)
        .read_to_string(&mut String::new());
    Some(())
}

fn read_sse_endpoint(reader: &mut impl Read) -> Option<String> {
    let started = std::time::Instant::now();
    let mut body = String::new();
    let mut buffer = [0_u8; 1024];
    while body.len() < MCP_RESPONSE_LIMIT_BYTES && started.elapsed() <= MCP_REQUEST_TIMEOUT {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                body.push_str(&String::from_utf8_lossy(&buffer[..count]));
                for event in parse_sse_events(&body) {
                    if event.event.as_deref() == Some("endpoint")
                        || event.data.starts_with("/message")
                    {
                        return Some(event.data);
                    }
                }
            }
            Err(_) => break,
        }
    }
    None
}

fn read_tools_response(mut reader: impl Read) -> Option<Vec<McpToolDef>> {
    let started = std::time::Instant::now();
    let mut body = String::new();
    let mut buffer = [0_u8; 4096];
    while body.len() < MCP_RESPONSE_LIMIT_BYTES && started.elapsed() <= MCP_REQUEST_TIMEOUT {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                body.push_str(&String::from_utf8_lossy(&buffer[..count]));
                if let Some(tools) = parse_tools_response(&body) {
                    return Some(tools);
                }
            }
            Err(_) => break,
        }
    }
    parse_tools_response(&body)
}

fn parse_tools_response(body: &str) -> Option<Vec<McpToolDef>> {
    for value in json_candidates(body) {
        if let Some(tools) = extract_tools(&value)
            && !tools.is_empty()
        {
            return Some(tools);
        }
    }
    None
}

fn json_candidates(body: &str) -> Vec<Value> {
    let mut values = Vec::new();
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        values.push(value);
    }
    values.extend(body.lines().filter_map(|line| {
        let data = line.trim().strip_prefix("data:")?.trim();
        if data.is_empty() || data == "[DONE]" {
            return None;
        }
        serde_json::from_str::<Value>(data).ok()
    }));
    values
}

struct SseEvent {
    event: Option<String>,
    data: String,
}

fn parse_sse_events(body: &str) -> Vec<SseEvent> {
    body.replace("\r\n", "\n")
        .split("\n\n")
        .filter(|chunk| !chunk.trim().is_empty())
        .filter_map(|chunk| {
            let mut event = None;
            let mut data = Vec::new();
            for line in chunk.lines() {
                let line = line.trim();
                if let Some(value) = line.strip_prefix("event:") {
                    event = Some(value.trim().to_string());
                } else if let Some(value) = line.strip_prefix("data:") {
                    data.push(value.trim());
                }
            }
            if data.is_empty() {
                None
            } else {
                Some(SseEvent {
                    event,
                    data: data.join("\n"),
                })
            }
        })
        .collect()
}

fn extract_tools(value: &Value) -> Option<Vec<McpToolDef>> {
    let items = value
        .pointer("/result/tools")
        .or_else(|| value.pointer("/data/tools"))
        .or_else(|| value.get("tools"))
        .and_then(Value::as_array)?;
    Some(items.iter().filter_map(parse_tool).collect())
}

fn parse_tool(value: &Value) -> Option<McpToolDef> {
    let raw = value.as_object()?;
    Some(McpToolDef {
        name: raw.get("name")?.as_str()?.to_string(),
        description: raw
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        parameters: raw
            .get("parameters")
            .or_else(|| raw.get("inputSchema"))
            .or_else(|| raw.get("input_schema"))
            .cloned(),
    })
}

fn sse_url(url: &str) -> Option<String> {
    let (base, suffix) = split_query_fragment(url);
    if base.ends_with("/sse") {
        return Some(url.to_string());
    }
    if base.rsplit('/').next().is_some_and(|part| !part.is_empty()) {
        return Some(url.to_string());
    }
    Some(format!("{}/sse{suffix}", base.trim_end_matches('/')))
}

fn tools_url(url: &str) -> Option<String> {
    let (base, _) = split_query_fragment(url);
    if base.ends_with("/tools") {
        return Some(url.to_string());
    }
    if base.ends_with("/sse") {
        return Some(format!("{}/tools", base.trim_end_matches("/sse")));
    }
    Some(format!("{}/tools", base.trim_end_matches('/')))
}

fn endpoint_url(sse_url: &str, endpoint: &str) -> Option<String> {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return Some(endpoint.to_string());
    }
    let origin = url_origin(sse_url)?;
    if endpoint.starts_with('/') {
        Some(format!("{origin}{endpoint}"))
    } else {
        Some(format!("{}/{}", sse_url.trim_end_matches('/'), endpoint))
    }
}

fn url_origin(url: &str) -> Option<String> {
    let scheme_end = url.find("://")? + 3;
    let authority_end = url[scheme_end..]
        .find(['/', '?', '#'])
        .map(|index| scheme_end + index)
        .unwrap_or(url.len());
    Some(url[..authority_end].to_string())
}

fn split_query_fragment(url: &str) -> (&str, &str) {
    let index = url.find(['?', '#']).unwrap_or(url.len());
    (&url[..index], &url[index..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_environment_and_arguments() {
        let servers = parse_mcp_servers(
            &serde_json::json!({
                "example": {
                    "command": "server",
                    "args": ["--token", "token-value", "--api-key=key-value", "--verbose"],
                    "url": "https://user:password@mcp.example/mcp?token=url-token&format=json&monkey=banana&author=alice&keyboard=us&X-Amz-Signature=signed#api%5Fkey=fragment-secret",
                    "env": {
                        "API_KEY": "env-key-value",
                        "REGION": "us-east-1"
                    }
                }
            }),
            None,
        );
        let server = &servers[0];

        assert_eq!(
            server.args,
            ["--token", "****", "--api-key=****", "--verbose"]
        );
        assert_eq!(
            server.url.as_deref(),
            Some(
                "https://****@mcp.example/mcp?token=****&format=json&monkey=banana&author=alice&keyboard=us&X-Amz-Signature=****#api%5Fkey=****"
            )
        );
        let env = server.env.as_ref().unwrap();
        assert_eq!(env.get("API_KEY").map(String::as_str), Some("****"));
        assert_eq!(env.get("REGION").map(String::as_str), Some("us-east-1"));
    }

    #[test]
    fn parses_sse_tools_response() {
        let body = r#"event: message
data: {"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"lookup","description":"Find docs"}]}}

"#;

        let tools = parse_tools_response(body).unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "lookup");
        assert_eq!(tools[0].description.as_deref(), Some("Find docs"));
    }
}
