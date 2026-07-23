#![cfg(feature = "c-binding")]

use std::ffi::{CStr, CString};
use std::fs;
use std::io::Write;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use sentra_lib::bindings::c::{
    sentra_import_rules, sentra_initialize, sentra_scan_skills, sentra_string_free,
};

fn init_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn call_initialize(workspace_root: Option<&str>) -> serde_json::Value {
    let workspace_c = workspace_root.map(|s| CString::new(s).unwrap());
    let ptr = workspace_c
        .as_ref()
        .map_or(std::ptr::null(), |c| c.as_ptr());
    let result = sentra_initialize(ptr);
    read_json_and_free(result)
}

fn call_scan(request: &str) -> serde_json::Value {
    let req_c = CString::new(request).unwrap();
    let result = sentra_scan_skills(req_c.as_ptr());
    read_json_and_free(result)
}

fn call_import(rule_source: &str) -> serde_json::Value {
    let source_c = CString::new(rule_source).unwrap();
    let result = sentra_import_rules(source_c.as_ptr());
    read_json_and_free(result)
}

fn read_json_and_free(ptr: *mut std::os::raw::c_char) -> serde_json::Value {
    assert!(!ptr.is_null(), "C API result must not be NULL");
    let json = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
    sentra_string_free(ptr);
    serde_json::from_str(&json).unwrap()
}

fn write_rule_zip(path: &std::path::Path, rule_content: &str) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("risk-rules.yar", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(rule_content.as_bytes()).unwrap();
    zip.finish().unwrap();
}

#[test]
fn initialize_creates_runtime_tree_and_returns_paths() {
    let _lock = init_test_lock();
    let workspace = tempfile::tempdir().unwrap();
    let value = call_initialize(Some(workspace.path().to_str().unwrap()));

    assert_eq!(value["workspaceRoot"], workspace.path().to_str().unwrap());
    assert_eq!(
        value["sentraHome"],
        workspace.path().join(".sentra").to_str().unwrap()
    );
    assert_eq!(
        value["configFile"],
        workspace
            .path()
            .join(".sentra")
            .join("config.json")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        value["ruleDirs"]["hash"],
        workspace
            .path()
            .join(".sentra")
            .join("hash")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        value["ruleDirs"]["yara"],
        workspace
            .path()
            .join(".sentra")
            .join("yara")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        value["ruleDirs"]["ti"],
        workspace
            .path()
            .join(".sentra")
            .join("ti")
            .to_str()
            .unwrap()
    );

    assert!(workspace.path().join(".sentra").is_dir());
    assert!(
        workspace
            .path()
            .join(".sentra")
            .join("config.json")
            .is_file()
    );
    assert!(workspace.path().join(".sentra").join("hash").is_dir());
    assert!(workspace.path().join(".sentra").join("yara").is_dir());
    assert!(workspace.path().join(".sentra").join("ti").is_dir());
}

#[test]
fn initialize_keeps_existing_config_and_reinitializes_to_latest_workspace() {
    let _lock = init_test_lock();
    let first = tempfile::tempdir().unwrap();
    let config = first.path().join(".sentra").join("config.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "{\"llm\":{\"model\":\"kept\"}}\n").unwrap();

    let first_value = call_initialize(Some(first.path().to_str().unwrap()));
    assert_eq!(
        fs::read_to_string(first_value["configFile"].as_str().unwrap()).unwrap(),
        "{\"llm\":{\"model\":\"kept\"}}\n"
    );

    let second = tempfile::tempdir().unwrap();
    let second_value = call_initialize(Some(second.path().to_str().unwrap()));
    assert_eq!(
        second.path().join(".sentra"),
        std::path::PathBuf::from(second_value["sentraHome"].as_str().unwrap())
    );
}

#[test]
fn initialize_rejects_invalid_inputs_and_file_conflicts() {
    let _lock = init_test_lock();
    let null_value = call_initialize(None);
    assert!(
        null_value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("NULL")
    );

    let empty = CString::new("").unwrap();
    let empty_value = read_json_and_free(sentra_initialize(empty.as_ptr()));
    assert!(
        empty_value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("empty")
    );

    let invalid: &[u8] = b"\xc0\x80";
    let bytes_with_nul: Vec<u8> = [invalid, b"\0"].concat();
    let invalid_c = CStr::from_bytes_with_nul(&bytes_with_nul).unwrap();
    let invalid_value = read_json_and_free(sentra_initialize(invalid_c.as_ptr()));
    assert!(
        invalid_value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("UTF-8")
    );

    let dir = tempfile::tempdir().unwrap();
    let config_conflict = dir.path().join(".sentra").join("config.json");
    fs::create_dir_all(&config_conflict).unwrap();
    let conflict_value = call_initialize(Some(dir.path().to_str().unwrap()));
    assert!(conflict_value.get("error").is_some());
}

#[test]
fn import_rules_rejects_without_initialize_in_fresh_process() {
    if std::env::var("SENTRA_CHILD_UNINITIALIZED_IMPORT").as_deref() == Ok("1") {
        let source = CString::new("/tmp/rules.zip").unwrap();
        let value = read_json_and_free(sentra_import_rules(source.as_ptr()));
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not initialized")
        );
        return;
    }

    let current_exe = std::env::current_exe().unwrap();
    let status = Command::new(current_exe)
        .env("SENTRA_CHILD_UNINITIALIZED_IMPORT", "1")
        .arg("--exact")
        .arg("import_rules_rejects_without_initialize_in_fresh_process")
        .arg("--nocapture")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn import_rules_imports_local_zip_after_initialize() {
    let _lock = init_test_lock();
    let workspace = tempfile::tempdir().unwrap();
    call_initialize(Some(workspace.path().to_str().unwrap()));

    let rule_zip = workspace.path().join("rules.zip");
    write_rule_zip(
        &rule_zip,
        r#"
rule ImportedRule {
    strings:
        $a = "imported-marker"
    condition:
        $a
}
"#,
    );

    let value = call_import(rule_zip.to_str().unwrap());
    assert_eq!(value["ok"], true);
    assert_eq!(value["code"], "ok");
    assert_eq!(value["imported"]["yara"], 1);
    assert!(
        workspace
            .path()
            .join(".sentra")
            .join("yara")
            .join("risk-rules.yar")
            .is_file()
    );
}

#[test]
fn import_rules_detects_zip_without_zip_extension() {
    let _lock = init_test_lock();
    let workspace = tempfile::tempdir().unwrap();
    call_initialize(Some(workspace.path().to_str().unwrap()));

    let rule_zip = workspace.path().join("rules-bundle");
    write_rule_zip(
        &rule_zip,
        r#"
rule ExtensionlessImportedRule {
    strings:
        $a = "extensionless-marker"
    condition:
        $a
}
"#,
    );

    let value = call_import(rule_zip.to_str().unwrap());
    assert_eq!(value["ok"], true);
    assert_eq!(value["imported"]["yara"], 1);
}

#[test]
fn import_rules_fails_when_imported_yara_cannot_compile() {
    let _lock = init_test_lock();
    let workspace = tempfile::tempdir().unwrap();
    call_initialize(Some(workspace.path().to_str().unwrap()));

    let rule_zip = workspace.path().join("bad-rules.zip");
    write_rule_zip(&rule_zip, "rule Broken { condition: }");

    let value = call_import(rule_zip.to_str().unwrap());
    assert!(value.get("error").is_some());
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("failed to compile YARA rule")
    );
}

#[test]
fn scan_uses_initialized_rule_dirs_but_request_home_for_agent_discovery() {
    let _lock = init_test_lock();
    let workspace = tempfile::tempdir().unwrap();
    call_initialize(Some(workspace.path().to_str().unwrap()));

    fs::write(
        workspace
            .path()
            .join(".sentra")
            .join("yara")
            .join("marker.yar"),
        r#"
rule WorkspaceRule {
    meta:
        severity = "HIGH"
        category = "PROMPT_INJECTION"
        title = "Workspace rule"
    strings:
        $a = "workspace-only-marker"
    condition:
        $a
}
"#,
    )
    .unwrap();

    let user_home = tempfile::tempdir().unwrap();
    let skill = user_home
        .path()
        .join(".codex")
        .join("skills")
        .join("workspace-rule-skill");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: workspace-rule-skill\n---\nworkspace-only-marker",
    )
    .unwrap();

    let request = serde_json::json!({
        "home": user_home.path(),
        "agents": ["codex"],
        "checkers": {
            "hash": false,
            "yara": true,
            "ti": false,
            "llm": false,
            "onlineTi": false
        }
    })
    .to_string();
    let value = call_scan(&request);
    let results = value.as_array().expect("scan must return an array");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["agentName"], "codex-cli");
    assert_eq!(results[0]["data"]["name"], "workspace-rule-skill");
    assert_eq!(
        results[0]["report"]["findings"][0]["title"],
        "Workspace rule"
    );
}
