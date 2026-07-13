use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt;
use std::future::Future;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{SentraError, SentraResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireProtocol {
    Responses,
    ChatCompletions,
    AnthropicMessages,
}

impl WireProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            WireProtocol::Responses => "responses",
            WireProtocol::ChatCompletions => "chat_completions",
            WireProtocol::AnthropicMessages => "anthropic_messages",
        }
    }

    pub fn default_stream(self) -> bool {
        !matches!(self, WireProtocol::AnthropicMessages)
    }
}

impl fmt::Display for WireProtocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str((*self).as_str())
    }
}

impl FromStr for WireProtocol {
    type Err = SentraError;

    fn from_str(value: &str) -> SentraResult<Self> {
        match value {
            "responses" => Ok(WireProtocol::Responses),
            "chat_completions" => Ok(WireProtocol::ChatCompletions),
            "anthropic_messages" => Ok(WireProtocol::AnthropicMessages),
            other => Err(SentraError::UnsupportedProtocol(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRequestParams {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub protocol: WireProtocol,
    pub max_tokens: usize,
    pub stream: bool,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPrompt {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProbeRequest {
    pub protocol: WireProtocol,
    pub body: Option<String>,
}

pub fn parse_wire_protocol(value: &str) -> SentraResult<WireProtocol> {
    value.parse()
}

const PROBE_SYSTEM_PROMPT: &str =
    "You are Sentra llm-checker availability probe. Reply only with valid JSON.";
const PROBE_USER_PROMPT: &str =
    r#"Demo source: demo://sentra-probe. Content: safe demo text. Return exactly {"results":[]}."#;
const PROBE_MAX_TOKENS: usize = 1024;

pub fn default_model_probe_prompt() -> ModelPrompt {
    ModelPrompt {
        system: PROBE_SYSTEM_PROMPT.to_string(),
        user: PROBE_USER_PROMPT.to_string(),
    }
}

pub fn build_model_probe_request(protocol: WireProtocol, model: &str) -> ModelProbeRequest {
    let body = match protocol {
        WireProtocol::Responses => Some(json!({
            "model": model,
            "stream": true,
            "max_output_tokens": PROBE_MAX_TOKENS,
            "input": [
                {
                    "role": "system",
                    "content": [
                        {
                            "type": "input_text",
                            "text": PROBE_SYSTEM_PROMPT
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": PROBE_USER_PROMPT
                        }
                    ]
                }
            ]
        })),
        WireProtocol::ChatCompletions => Some(json!({
            "model": model,
            "stream": true,
            "max_tokens": PROBE_MAX_TOKENS,
            "messages": [
                {"role": "system", "content": PROBE_SYSTEM_PROMPT},
                {"role": "user", "content": PROBE_USER_PROMPT}
            ]
        })),
        WireProtocol::AnthropicMessages => Some(json!({
            "model": model,
            "max_tokens": PROBE_MAX_TOKENS,
            "system": [
                {
                    "type": "text",
                    "text": PROBE_SYSTEM_PROMPT
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": PROBE_USER_PROMPT
                        }
                    ]
                }
            ]
        })),
    };
    ModelProbeRequest {
        protocol,
        body: body.map(|body| body.to_string()),
    }
}

pub fn validate_model_probe_response(protocol: WireProtocol, raw: &str) -> Result<String, String> {
    let text = validate_model_text_response(protocol, raw)?;
    validate_probe_json_text(&text)?;
    Ok(text)
}

pub fn validate_model_text_response(protocol: WireProtocol, raw: &str) -> Result<String, String> {
    if raw.contains("event: error") || raw.contains("\"type\":\"error\"") {
        return Err(format!(
            "model probe returned provider error: {}",
            compact_excerpt(raw, 1000)
        ));
    }

    extract_probe_text(protocol, raw)
}

fn validate_probe_json_text(text: &str) -> Result<(), String> {
    let json = extract_json_object(text).ok_or_else(|| {
        format!(
            "model probe response did not contain JSON; text excerpt: {}",
            compact_excerpt(text, 1000)
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|err| {
        format!(
            "model probe response JSON parse failed: {err}; text excerpt: {}",
            compact_excerpt(text, 1000)
        )
    })?;
    if value
        .get("results")
        .and_then(|value| value.as_array())
        .is_none()
    {
        return Err(format!(
            "model probe response JSON missing results array; text excerpt: {}",
            compact_excerpt(text, 1000)
        ));
    }
    Ok(())
}

fn extract_probe_text(protocol: WireProtocol, raw: &str) -> Result<String, String> {
    match protocol {
        WireProtocol::Responses => extract_responses_probe_text(raw),
        WireProtocol::ChatCompletions => extract_chat_probe_text(raw),
        WireProtocol::AnthropicMessages => extract_anthropic_probe_text(raw),
    }
    .and_then(|text| {
        if text.trim().is_empty() {
            Err(format!(
                "model probe produced empty text; raw excerpt: {}",
                compact_excerpt(raw, 1000)
            ))
        } else {
            Ok(text)
        }
    })
}

fn extract_responses_probe_text(raw: &str) -> Result<String, String> {
    if raw.trim_start().starts_with('{') {
        let value: Value = serde_json::from_str(raw)
            .map_err(|err| format!("failed to parse responses probe JSON: {err}"))?;
        return Ok(collect_nested_text(
            value.get("output").and_then(Value::as_array),
            |item| item.get("content").and_then(Value::as_array),
            "text",
        ));
    }

    Ok(collect_sse_text(raw, |value| {
        (value.get("type").and_then(Value::as_str) == Some("response.output_text.delta"))
            .then(|| value.get("delta").and_then(Value::as_str))
            .flatten()
    }))
}

fn extract_chat_probe_text(raw: &str) -> Result<String, String> {
    if raw.trim_start().starts_with('{') {
        let value: Value = serde_json::from_str(raw)
            .map_err(|err| format!("failed to parse chat probe JSON: {err}"))?;
        return Ok(value
            .get("choices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|choice| {
                choice
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join(""));
    }

    Ok(collect_sse_text(raw, |value| {
        value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
    }))
}

fn extract_anthropic_probe_text(raw: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|err| format!("failed to parse anthropic probe JSON: {err}"))?;
    Ok(collect_text_array(
        value.get("content").and_then(Value::as_array),
        "text",
    ))
}

fn collect_nested_text<'a>(
    items: Option<&'a Vec<Value>>,
    content: impl Fn(&'a Value) -> Option<&'a Vec<Value>>,
    text_key: &str,
) -> String {
    items
        .into_iter()
        .flatten()
        .flat_map(content)
        .flatten()
        .filter_map(|part| part.get(text_key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

fn collect_text_array(items: Option<&Vec<Value>>, text_key: &str) -> String {
    items
        .into_iter()
        .flatten()
        .filter_map(|part| part.get(text_key).and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

fn collect_sse_text(raw: &str, text: impl for<'a> Fn(&'a Value) -> Option<&'a str>) -> String {
    let mut output = String::new();
    for value in sse_json_values(raw) {
        if let Some(delta) = text(&value) {
            output.push_str(delta);
        }
    }
    output
}

fn sse_data_payloads(raw: &str) -> impl Iterator<Item = String> + '_ {
    raw.lines().filter_map(|line| {
        line.trim()
            .strip_prefix("data:")
            .map(|payload| payload.trim().to_string())
    })
}

fn sse_json_values(raw: &str) -> impl Iterator<Item = Value> + '_ {
    sse_data_payloads(raw).filter_map(|payload| {
        if payload == "[DONE]" {
            None
        } else {
            serde_json::from_str(&payload).ok()
        }
    })
}

fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| text[start..=end].to_string())
}

pub async fn send_model_request(
    params: &ModelRequestParams,
    prompt: &ModelPrompt,
) -> Result<String, String> {
    use rig_core::client::CompletionClient;
    use rig_core::providers::{anthropic, openai};

    let response_recorder = ResponseRecorder::default();
    let http_client = RecordingHttpClient::new(response_recorder.clone());

    match params.protocol {
        WireProtocol::Responses => {
            let client = openai::Client::builder()
                .http_client(http_client)
                .api_key(&params.api_key)
                .base_url(&params.api_url)
                .build()
                .map_err(|err| format!("failed to build Rig OpenAI client: {err}"))?;
            let model = client.completion_model(params.model.clone());
            send_rig_completion(&model, params, prompt, &response_recorder).await
        }
        WireProtocol::ChatCompletions => {
            let client = openai::CompletionsClient::builder()
                .http_client(http_client)
                .api_key(&params.api_key)
                .base_url(&params.api_url)
                .build()
                .map_err(|err| format!("failed to build Rig OpenAI completions client: {err}"))?;
            let model = client.completion_model(params.model.clone());
            send_rig_completion(&model, params, prompt, &response_recorder).await
        }
        WireProtocol::AnthropicMessages => {
            let client = anthropic::Client::builder()
                .http_client(http_client)
                .api_key(&params.api_key)
                .base_url(&params.api_url)
                .build()
                .map_err(|err| format!("failed to build Rig Anthropic client: {err}"))?;
            let model = client.completion_model(params.model.clone());
            send_rig_completion(&model, params, prompt, &response_recorder).await
        }
    }
}

pub async fn probe_model_request(params: &ModelRequestParams) -> Result<String, String> {
    probe_model_request_with_prompt(params, &default_model_probe_prompt()).await
}

pub async fn probe_model_request_with_prompt(
    params: &ModelRequestParams,
    prompt: &ModelPrompt,
) -> Result<String, String> {
    let output = send_model_request(params, prompt).await?;
    validate_probe_json_text(&output)?;
    Ok(output)
}

async fn send_rig_completion<M>(
    model: &M,
    params: &ModelRequestParams,
    prompt: &ModelPrompt,
    response_recorder: &ResponseRecorder,
) -> Result<String, String>
where
    M: rig_core::completion::CompletionModel,
{
    use futures::StreamExt;
    use rig_core::completion::AssistantContent;
    use rig_core::streaming::StreamedAssistantContent;

    let request = model
        .completion_request(prompt.user.clone())
        .preamble(prompt.system.clone())
        .max_tokens(params.max_tokens as u64);
    let request = request.build();

    if params.stream {
        return with_timeout(params.timeout_ms, "streaming completion", async {
            let mut stream = model
                .stream(request)
                .await
                .map_err(|err| rig_error("streaming completion", err))?;
            let mut output = String::new();
            let mut diagnostics = Vec::new();
            while let Some(item) = stream.next().await {
                match item.map_err(|err| rig_error("streaming completion", err))? {
                    StreamedAssistantContent::Text(text) => output.push_str(&text.text),
                    other => diagnostics.push(serialized_debug_payload(&other)),
                }
            }
            if output.is_empty() && !diagnostics.is_empty() {
                return Ok(format!(
                    "[non_text_stream_items] {}\n{}",
                    diagnostics.join("\n"),
                    response_recorder.format()
                ));
            }
            Ok(output)
        })
        .await;
    }

    with_timeout(params.timeout_ms, "completion", async {
        let response = model
            .completion(request)
            .await
            .map_err(|err| rig_error("completion", err))?;
        let output = response
            .choice
            .iter()
            .filter_map(|item| match item {
                AssistantContent::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if output.is_empty() {
            return Ok(format!(
                "[non_text_completion_response] {}\n{}",
                serialized_debug_payload(&response.raw_response),
                response_recorder.format()
            ));
        }
        Ok(output)
    })
    .await
}

async fn with_timeout<T>(
    timeout_ms: u64,
    context: &str,
    future: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(Duration::from_millis(timeout_ms), future)
        .await
        .map_err(|_| format!("Rig {context} timed out after {timeout_ms} ms"))?
}

fn serialized_debug_payload<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|err| format!("<failed to serialize payload: {err}>"))
}

fn rig_error(context: &str, err: rig_core::completion::CompletionError) -> String {
    let text = err.to_string();
    if let Some(rest) = text.strip_prefix("HttpError: Invalid status code ") {
        format!("Rig {context} failed: HTTP {rest}")
    } else {
        format!("Rig {context} failed: {text}")
    }
}

#[derive(Clone, Default, Debug)]
struct ResponseRecorder {
    body: Arc<Mutex<Vec<u8>>>,
}

impl ResponseRecorder {
    fn record(&self, bytes: &[u8]) {
        if let Ok(mut body) = self.body.lock() {
            body.extend_from_slice(bytes);
        }
    }

    fn format(&self) -> String {
        let body = self
            .body
            .lock()
            .map(|body| body.clone())
            .unwrap_or_default();
        if body.is_empty() {
            return "[raw_gateway_response] <empty>".to_string();
        }
        let text = String::from_utf8_lossy(&body);
        format!("[raw_gateway_response] {}", compact_excerpt(&text, 4000))
    }
}

pub fn compact_excerpt(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "<empty>".to_string();
    }

    let excerpt: String = normalized.chars().take(max_chars).collect();
    let suffix = if normalized.chars().count() > max_chars {
        "..."
    } else {
        ""
    };
    format!("{excerpt}{suffix}")
}

#[derive(Clone, Debug, Default)]
struct RecordingHttpClient {
    inner: rig_core::http_client::ReqwestClient,
    recorder: ResponseRecorder,
}

impl RecordingHttpClient {
    fn new(recorder: ResponseRecorder) -> Self {
        Self {
            inner: rig_core::http_client::ReqwestClient::default(),
            recorder,
        }
    }
}

impl rig_core::http_client::HttpClientExt for RecordingHttpClient {
    fn send<T, U>(
        &self,
        req: rig_core::http_client::Request<T>,
    ) -> impl std::future::Future<
        Output = rig_core::http_client::Result<
            rig_core::http_client::Response<rig_core::http_client::LazyBody<U>>,
        >,
    > + rig_core::wasm_compat::WasmCompatSend
    + 'static
    where
        T: Into<bytes::Bytes>,
        T: rig_core::wasm_compat::WasmCompatSend,
        U: From<bytes::Bytes>,
        U: rig_core::wasm_compat::WasmCompatSend + 'static,
    {
        let (parts, body) = req.into_parts();
        let body = body.into();
        let client = self.inner.clone();
        let recorder = self.recorder.clone();

        async move {
            let req = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body);
            let response = req
                .send()
                .await
                .map_err(|err| rig_core::http_client::Error::Instance(err.into()))?;
            if !response.status().is_success() {
                let status = response.status();
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|err| format!("failed to read error response body: {err}"));
                return Err(rig_core::http_client::Error::InvalidStatusCodeWithMessage(
                    status, message,
                ));
            }

            let mut res = rig_core::http_client::Response::builder().status(response.status());
            if let Some(headers) = res.headers_mut() {
                *headers = response.headers().clone();
            }

            let body: rig_core::http_client::LazyBody<U> = Box::pin(async move {
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|err| rig_core::http_client::Error::Instance(err.into()))?;
                recorder.record(&bytes);
                Ok(U::from(bytes))
            });

            res.body(body)
                .map_err(rig_core::http_client::Error::Protocol)
        }
    }

    fn send_multipart<U>(
        &self,
        req: rig_core::http_client::Request<rig_core::http_client::MultipartForm>,
    ) -> impl std::future::Future<
        Output = rig_core::http_client::Result<
            rig_core::http_client::Response<rig_core::http_client::LazyBody<U>>,
        >,
    > + rig_core::wasm_compat::WasmCompatSend
    + 'static
    where
        U: From<bytes::Bytes>,
        U: rig_core::wasm_compat::WasmCompatSend + 'static,
    {
        let inner = self.inner.clone();
        async move { rig_core::http_client::HttpClientExt::send_multipart(&inner, req).await }
    }

    fn send_streaming<T>(
        &self,
        req: rig_core::http_client::Request<T>,
    ) -> impl std::future::Future<
        Output = rig_core::http_client::Result<rig_core::http_client::StreamingResponse>,
    > + rig_core::wasm_compat::WasmCompatSend
    where
        T: Into<bytes::Bytes> + rig_core::wasm_compat::WasmCompatSend,
    {
        use futures::StreamExt;

        let (parts, body) = req.into_parts();
        let client = self.inner.clone();
        let recorder = self.recorder.clone();

        async move {
            let req = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body.into())
                .build()
                .map_err(|err| rig_core::http_client::Error::Instance(err.into()))?;
            let response = client
                .execute(req)
                .await
                .map_err(|err| rig_core::http_client::Error::Instance(err.into()))?;
            if !response.status().is_success() {
                let status = response.status();
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|err| format!("failed to read error response body: {err}"));
                return Err(rig_core::http_client::Error::InvalidStatusCodeWithMessage(
                    status, message,
                ));
            }

            let mut res = rig_core::http_client::Response::builder()
                .status(response.status())
                .version(response.version());
            if let Some(headers) = res.headers_mut() {
                *headers = response.headers().clone();
            }

            let stream: rig_core::http_client::sse::BoxedStream =
                Box::pin(response.bytes_stream().map(move |chunk| {
                    chunk
                        .map(|bytes| {
                            recorder.record(&bytes);
                            bytes
                        })
                        .map_err(|err| rig_core::http_client::Error::Instance(err.into()))
                }));

            res.body(stream)
                .map_err(rig_core::http_client::Error::Protocol)
        }
    }
}
