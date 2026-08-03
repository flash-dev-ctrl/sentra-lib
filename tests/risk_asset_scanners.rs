use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use sentra_lib::interfaces::{CronData, McpData, McpType, MemoryData, ProviderData};
use sentra_lib::risks::{RiskAsset, RiskScanner, RuleDirectoryConfig, RuleType, ScanOptions};

const MARKER_RULE: &str = r#"
rule ScannerMarker {
    strings:
        $marker = "scanner-risk-marker"
    condition:
        $marker
}
"#;

#[test]
fn unified_scanner_dispatches_cron_asset_to_cron_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let scanner = scanner_with_yara(&rules_dir);
    let asset = CronData {
        id: "cron-demo".to_string(),
        name: "cron-demo".to_string(),
        prompt: "run scanner-risk-marker".to_string(),
        enabled: true,
        home: Some(dir.path().join("missing-cron")),
        ..CronData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "cron-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].file, "cron:cron-demo:prompt");
}

#[test]
fn unified_scanner_dispatches_memory_asset_to_memory_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let memory_path = dir.path().join("memory.md");
    fs::write(&memory_path, "remember scanner-risk-marker").unwrap();
    let scanner = scanner_with_yara(&rules_dir);
    let asset = MemoryData {
        name: "memory.md".to_string(),
        path: memory_path.clone(),
        ..MemoryData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "memory-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].file,
        memory_path.to_string_lossy().to_string()
    );
}

#[test]
fn unified_scanner_dispatches_mcp_asset_to_mcp_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let server = TestMcpToolsServer::start_sse();
    let scanner = scanner_with_yara(&rules_dir);
    let asset = McpData {
        name: "mcp-demo".to_string(),
        mcp_type: Some(McpType::Sse),
        url: Some(server.url()),
        enabled: Some(true),
        ..McpData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    let requests = server.requests();
    assert!(requests.iter().any(|body| body.contains("\"tools/list\"")));
    assert_eq!(report.metadata.scanner, "mcp-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].file, "mcp:mcp-demo:tools");
    assert!(
        report.findings[0]
            .context
            .as_deref()
            .unwrap_or_default()
            .contains("scanner-risk-marker")
    );
}

#[test]
fn memory_scanner_skips_missing_memory_files() {
    let dir = tempfile::tempdir().unwrap();
    let scanner = RiskScanner::new(ScanOptions::default()).unwrap();
    let asset = MemoryData {
        name: ".codex-global-state.json".to_string(),
        path: dir.path().join(".codex").join(".codex-global-state.json"),
        ..MemoryData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "memory-scanner");
    assert!(report.findings.is_empty());
    assert!(report.errors.is_empty());
}

#[test]
fn unified_scanner_dispatches_provider_asset_to_provider_scanner() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let scanner = scanner_with_yara(&rules_dir);
    let asset = ProviderData {
        name: "demo".to_string(),
        base_url: Some("https://scanner-risk-marker.example.com/v1".to_string()),
        enabled: true,
        ..ProviderData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "provider-scanner");
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].file,
        "https://scanner-risk-marker.example.com/v1"
    );
}

#[test]
fn provider_scanner_returns_empty_report_without_base_url() {
    let scanner = RiskScanner::new(ScanOptions::default()).unwrap();
    let asset = ProviderData {
        name: "demo".to_string(),
        base_url: None,
        ..ProviderData::default()
    };

    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();

    assert_eq!(report.metadata.scanner, "provider-scanner");
    assert!(report.findings.is_empty());
    assert!(report.errors.is_empty());
}

#[test]
fn risk_scanner_can_load_rules_explicitly_before_scanning() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let mut scanner = RiskScanner::new(ScanOptions {
        rules: Some(RuleDirectoryConfig {
            yara: Some(rules_dir),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();

    let summary = scanner.load_rule(RuleType::Yara).unwrap();

    assert_eq!(summary.yara, 1);
    let asset = ProviderData {
        name: "demo".to_string(),
        base_url: Some("https://scanner-risk-marker.example.com/v1".to_string()),
        enabled: true,
        ..ProviderData::default()
    };
    let report = block_on(scanner.scan(RiskAsset::from(&asset))).unwrap();
    assert_eq!(report.findings.len(), 1);
}

#[test]
fn risk_scanner_loads_rules_one_type_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = write_rule_dir(dir.path());
    let mut scanner = RiskScanner::new(ScanOptions {
        rules: Some(RuleDirectoryConfig {
            yara: Some(rules_dir),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap();

    let summary = scanner.load_rule(RuleType::Yara).unwrap();

    assert_eq!(summary.yara, 1);
    assert_eq!(summary.ti_ips, 0);
    assert_eq!(summary.hash_blacklist, 0);
}

fn write_rule_dir(root: &std::path::Path) -> std::path::PathBuf {
    let rules_dir = root.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(rules_dir.join("marker.yar"), MARKER_RULE).unwrap();
    rules_dir
}

fn scanner_with_yara(rules_dir: &std::path::Path) -> RiskScanner {
    RiskScanner::new(ScanOptions {
        rules: Some(RuleDirectoryConfig {
            yara: Some(rules_dir.to_path_buf()),
            ..Default::default()
        }),
        ..ScanOptions::default()
    })
    .unwrap()
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(future)
}

struct TestMcpToolsServer {
    url: String,
    addr: String,
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestMcpToolsServer {
    fn start_sse() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let addr = addr.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_stop = Arc::clone(&stop);
        let thread_requests = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        handle_mcp_tools_request(stream, &thread_requests);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            url: format!("http://{addr}/mcp"),
            addr,
            stop,
            requests,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        self.url.clone()
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for TestMcpToolsServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_mcp_tools_request(mut stream: TcpStream, requests: &Arc<Mutex<Vec<String>>>) {
    let body = read_http_request(&mut stream);
    requests.lock().unwrap().push(body.clone());
    let response_body = if body.contains("\"tools/list\"") {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [{
                    "name": "lookup",
                    "description": "Search docs with scanner-risk-marker"
                }]
            }
        })
    } else {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32601, "message": "method not found" }
        })
    };
    let response_body = format!("event: message\ndata: {}\n\n", response_body);
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn read_http_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if request_body_complete(&bytes) {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn request_body_complete(bytes: &[u8]) -> bool {
    let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let content_length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    });
    let Some(content_length) = content_length else {
        return true;
    };
    bytes.len() >= header_end + 4 + content_length
}
