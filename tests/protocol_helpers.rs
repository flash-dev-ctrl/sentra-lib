use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use sentra_lib::protocol::{
    ModelPrompt, ModelRequestParams, WireProtocol, build_model_probe_request, parse_wire_protocol,
    probe_model_request, send_model_request, validate_model_probe_response,
};
use serde_json::json;

#[test]
fn wire_protocol_names_and_defaults_are_centralized() {
    assert_eq!(WireProtocol::Responses.to_string(), "responses");
    assert_eq!(
        WireProtocol::ChatCompletions.to_string(),
        "chat_completions"
    );
    assert_eq!(
        WireProtocol::AnthropicMessages.to_string(),
        "anthropic_messages"
    );

    assert_eq!(
        parse_wire_protocol("responses").unwrap(),
        WireProtocol::Responses
    );
    assert!(parse_wire_protocol("unknown").is_err());

    assert!(WireProtocol::Responses.default_stream());
    assert!(WireProtocol::ChatCompletions.default_stream());
    assert!(!WireProtocol::AnthropicMessages.default_stream());
}

#[test]
fn public_protocol_api_builds_probe_requests_matching_runtime_message_shapes() {
    let request = build_model_probe_request(WireProtocol::Responses, "svip/gpt-5.5");
    let body: serde_json::Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();

    assert_eq!(request.protocol, WireProtocol::Responses);
    assert_eq!(body["model"], "svip/gpt-5.5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_output_tokens"], 1024);
    assert_eq!(body["input"][0]["role"], "system");
    assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
    assert!(
        body["input"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("valid JSON")
    );
    assert_eq!(body["input"][1]["role"], "user");
    assert!(
        body["input"][1]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("demo://sentra-probe")
    );
    assert!(
        body["input"][1]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(r#"{"results":[]}"#)
    );
    assert!(body.get("instructions").is_none());
    assert!(body.get("tool_choice").is_none());

    let request = build_model_probe_request(WireProtocol::ChatCompletions, "svip/gpt-5.5");
    let body: serde_json::Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();

    assert_eq!(request.protocol, WireProtocol::ChatCompletions);
    assert_eq!(body["model"], "svip/gpt-5.5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 1024);
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("valid JSON")
    );
    assert_eq!(body["messages"][1]["role"], "user");
    assert!(
        body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("demo://sentra-probe")
    );
    assert!(
        body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains(r#"{"results":[]}"#)
    );

    let request = build_model_probe_request(WireProtocol::AnthropicMessages, "svip/gpt-5.5");
    let body: serde_json::Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();

    assert_eq!(request.protocol, WireProtocol::AnthropicMessages);
    assert_eq!(body["model"], "svip/gpt-5.5");
    assert_eq!(body["max_tokens"], 1024);
    assert_eq!(body["system"][0]["type"], "text");
    assert!(
        body["system"][0]["text"]
            .as_str()
            .unwrap()
            .contains("valid JSON")
    );
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert!(
        body["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("demo://sentra-probe")
    );
    assert!(
        body["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(r#"{"results":[]}"#)
    );
    assert!(body.get("stream").is_none());
}

#[test]
fn public_protocol_api_validates_probe_response_text_as_json_results() {
    let responses_sse = concat!(
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"results\\\":[]}\"}\n\n",
        "data: [DONE]\n"
    );
    assert_eq!(
        validate_model_probe_response(WireProtocol::Responses, responses_sse).unwrap(),
        r#"{"results":[]}"#
    );

    let chat_sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"results\\\":[]}\"}}]}\n\n",
        "data: [DONE]\n"
    );
    assert_eq!(
        validate_model_probe_response(WireProtocol::ChatCompletions, chat_sse).unwrap(),
        r#"{"results":[]}"#
    );

    let anthropic = r#"{"content":[{"type":"text","text":"{\"results\":[]}"}]}"#;
    assert_eq!(
        validate_model_probe_response(WireProtocol::AnthropicMessages, anthropic).unwrap(),
        r#"{"results":[]}"#
    );
}

#[test]
fn public_protocol_api_ignores_responses_reasoning_summary_probe_text() {
    let responses_sse = concat!(
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"thinking\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"results\\\":[]}\"}\n\n",
        "data: [DONE]\n"
    );

    assert_eq!(
        validate_model_probe_response(WireProtocol::Responses, responses_sse).unwrap(),
        r#"{"results":[]}"#
    );

    let reasoning_only = concat!(
        "event: response.reasoning_summary_text.delta\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"{\\\"results\\\":[]}\"}\n\n",
        "data: [DONE]\n"
    );
    let reason =
        validate_model_probe_response(WireProtocol::Responses, reasoning_only).unwrap_err();
    assert!(reason.contains("empty text"));
}

#[test]
fn public_protocol_api_rejects_probe_error_events_and_non_json_text() {
    let error = r#"event: error
data: {"type":"error","error":{"message":"System messages are not allowed"}}"#;
    let reason = validate_model_probe_response(WireProtocol::Responses, error).unwrap_err();
    assert!(reason.contains("provider error"));
    assert!(reason.contains("System messages are not allowed"));

    let non_json = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n";
    let reason =
        validate_model_probe_response(WireProtocol::ChatCompletions, non_json).unwrap_err();
    assert!(reason.contains("did not contain JSON"));
}

#[test]
fn public_protocol_api_sends_model_request_through_rig() {
    let observed = run_mock_server(200, &responses_text_response("ok"));

    let output = block_on(send_model_request(
        &ModelRequestParams {
            api_url: observed.url("/api"),
            api_key: "sk-test".to_string(),
            model: "gpt-test".to_string(),
            protocol: WireProtocol::Responses,
            max_tokens: 99,
            stream: false,
            timeout_ms: 1234,
        },
        &ModelPrompt {
            system: "system".to_string(),
            user: "user".to_string(),
        },
    ))
    .unwrap();

    assert_eq!(output, "ok");
    let request = observed.request();
    assert_eq!(request.path, "/api/responses");
    assert_eq!(request.body["input"][0]["role"], "system");
    assert_eq!(request.body["input"][0]["content"][0]["text"], "system");
    assert_eq!(request.body["input"][1]["role"], "user");
    assert_eq!(request.body["input"][1]["content"][0]["text"], "user");
}

#[test]
fn public_protocol_api_probes_model_through_rig_and_validates_json() {
    let observed = run_mock_server(200, &responses_text_response(r#"{"results":[]}"#));

    let output = block_on(probe_model_request(&ModelRequestParams {
        api_url: observed.url("/api"),
        api_key: "sk-test".to_string(),
        model: "gpt-test".to_string(),
        protocol: WireProtocol::Responses,
        max_tokens: 99,
        stream: false,
        timeout_ms: 1234,
    }))
    .unwrap();

    assert_eq!(output, r#"{"results":[]}"#);
    let request = observed.request();
    assert_eq!(request.path, "/api/responses");
    assert_eq!(request.body["input"][0]["role"], "system");
    assert_eq!(request.body["input"][1]["role"], "user");
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
        let request = self.rx.recv().unwrap();
        self.handle.join().unwrap();
        request
    }
}

struct ObservedRequest {
    path: String,
    body: serde_json::Value,
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

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':')
            && key.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap();
        }
    }

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    let body = serde_json::from_slice(&body).unwrap();
    tx.send(ObservedRequest { path, body }).unwrap();

    let reason = if status == 200 { "OK" } else { "Error" };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    )
    .unwrap();
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
