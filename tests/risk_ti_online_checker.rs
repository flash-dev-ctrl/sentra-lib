use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use sentra_lib::interfaces::{CheckInput, FileCategory, FileExtType, RiskSeverity};
use sentra_lib::risks::checkers::RiskChecker;
use sentra_lib::risks::types::CheckerConfig;
use sentra_lib::risks::{OnlineTiConfig, RiskAsset, RiskScanner, ScanOptions};

fn script_input(source: &str, content: &str) -> CheckInput {
    CheckInput::content(source, content).with_file_meta(FileCategory::Script, FileExtType::Ts)
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn mock_http_server<F>(handler: F) -> (String, Arc<AtomicUsize>)
where
    F: Fn(String, usize) -> String + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let server_count = Arc::clone(&count);
    let handler = Arc::new(handler);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                break;
            };
            let request_number = server_count.fetch_add(1, Ordering::SeqCst) + 1;
            let mut buffer = [0_u8; 2048];
            let bytes = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
            let response = handler(request, request_number);
            let _ = stream.write_all(response.as_bytes());
        }
    });

    (url, count)
}

fn json_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

fn error_response() -> String {
    "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        .to_string()
}

#[test]
fn scan_options_online_ti_reaches_threat_intel_checker() {
    let (url, _count) =
        mock_http_server(|_request, _| json_response(r#"{"Answer":[{"data":"0.0.0.0"}]}"#));
    let scanner = RiskScanner::new(ScanOptions {
        online_ti: Some(OnlineTiConfig {
            cloudflare_url: Some(url),
            ..OnlineTiConfig::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();

    let report = block_on(
        scanner.scan(RiskAsset::from(
            CheckInput::content("SKILL.md", "fetch https://sinkholed.example/path")
                .with_file_meta(FileCategory::Prompt, FileExtType::Md),
        )),
    )
    .unwrap();

    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].severity, RiskSeverity::High);
    assert_eq!(report.findings[0].checker, "threat-intel-checker");
    assert!(report.findings[0].description.contains("Cloudflare DNS"));
}

#[test]
fn cloudflare_sinkhole_after_offline_miss_produces_source_context_and_description() {
    let (url, _count) = mock_http_server(|request, _| {
        assert!(request.contains("sinkholed.example"));
        json_response(r#"{"Answer":[{"data":"0.0.0.0"}]}"#)
    });
    let checker = online_ti_checker(OnlineTiConfig {
        cloudflare_url: Some(url),
        ..OnlineTiConfig::default()
    });

    let output = block_on(checker.scan(&[script_input(
        "agent.ts",
        "connect to https://sinkholed.example/c2",
    )]))
    .unwrap();

    assert_eq!(output.findings.len(), 1);
    let finding = &output.findings[0];
    assert_eq!(finding.id, "ti-domain-0-sinkholed.example");
    assert!(finding.description.contains("Cloudflare DNS"));
    assert!(
        finding
            .description_zh
            .as_deref()
            .unwrap()
            .contains("Cloudflare DNS")
    );
    assert_eq!(
        finding.context.as_deref(),
        Some("connect to https://sinkholed.example/c2")
    );
}

#[test]
fn threatbook_malicious_domain_response_produces_finding_with_source() {
    let (url, _count) = mock_http_server(|request, _| {
        assert!(request.contains("/scene/dns"));
        assert!(request.contains("resource=evil.example.com"));
        json_response(
            r#"{"response_code":0,"data":{"domains":{"evil.example.com":{"is_malicious":true,"severity":"high","judgments":["botnet"]}}}}"#,
        )
    });
    let checker = online_ti_checker(OnlineTiConfig {
        threatbook_key: Some("test-key".to_string()),
        threatbook_url: Some(url),
        ..OnlineTiConfig::default()
    });

    let output = block_on(checker.scan(&[script_input(
        "agent.ts",
        "fetch https://evil.example.com/payload",
    )]))
    .unwrap();

    assert_eq!(output.findings.len(), 1);
    assert!(output.findings[0].description.contains("ThreatBook"));
}

#[test]
fn chaitin_malicious_ip_response_produces_finding_with_source() {
    let (url, _count) = mock_http_server(|request, _| {
        assert!(request.contains("/api/share/s"));
        assert!(request.contains("ip=203.0.113.44"));
        json_response(
            r#"{"success":true,"data":{"ip_info":{"status":"malicious","tags":["c2"]},"tip":"malicious ip"}}"#,
        )
    });
    let checker = online_ti_checker(OnlineTiConfig {
        chaitin_key: Some("test-key".to_string()),
        chaitin_url: Some(url),
        ..OnlineTiConfig::default()
    });

    let output = block_on(checker.scan(&[script_input("agent.ts", "dial 203.0.113.44")])).unwrap();

    assert_eq!(output.findings.len(), 1);
    assert!(output.findings[0].description.contains("Chaitin Rivers"));
}

#[test]
fn provider_circuit_breaks_after_three_consecutive_failures() {
    let (url, count) = mock_http_server(|_request, _| error_response());
    let checker = online_ti_checker(OnlineTiConfig {
        threatbook_key: Some("test-key".to_string()),
        threatbook_url: Some(url),
        ..OnlineTiConfig::default()
    });

    for index in 0..5 {
        let input = script_input(
            "agent.ts",
            &format!("fetch https://candidate-{index}.example.com"),
        );
        let output = block_on(checker.scan(&[input])).unwrap();
        assert!(output.findings.is_empty());
    }

    assert_eq!(count.load(Ordering::SeqCst), 3);
}

fn online_ti_checker(online_ti: OnlineTiConfig) -> RiskChecker {
    RiskChecker::new(ScanOptions {
        checker: Some(CheckerConfig {
            enable_hash: Some(false),
            enable_yara: Some(false),
            enable_llm: Some(false),
            enable_local_ti: Some(true),
            enable_online_ti: Some(true),
        }),
        online_ti: Some(online_ti),
        ..ScanOptions::default()
    })
    .unwrap()
}
