use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use serde::Serialize;

use crate::risks::RuleStore;

use super::adapter;
use super::rule_import_prep::extract_rules_zip;
use super::runtime;
use super::types::ScanRequest;

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

#[derive(Serialize)]
struct ImportRulesEnvelope {
    ok: bool,
    code: &'static str,
    imported: ImportResultDto,
}

/// Local mirror of `crate::risks::ImportResult` so we can derive `Serialize`
/// here without modifying the shared `risks` module.
#[derive(Serialize)]
struct ImportResultDto {
    yara: usize,
    ti: usize,
    hash: usize,
    skipped: usize,
}

impl From<crate::risks::ImportResult> for ImportResultDto {
    fn from(r: crate::risks::ImportResult) -> Self {
        Self {
            yara: r.yara,
            ti: r.ti,
            hash: r.hash,
            skipped: r.skipped,
        }
    }
}

/// Free a string returned by any sentra_* function. Safe to call with NULL.
#[unsafe(no_mangle)]
pub extern "C" fn sentra_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}

/// Return the version string. The caller does NOT own the returned pointer.
#[unsafe(no_mangle)]
pub extern "C" fn sentra_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

/// Initialize Sentra runtime files under workspace_root/.sentra.
///
/// Returns a JSON object string (caller must free with sentra_string_free).
/// On error, returns a JSON error envelope.
#[unsafe(no_mangle)]
pub extern "C" fn sentra_initialize(workspace_root: *const c_char) -> *mut c_char {
    if workspace_root.is_null() {
        return error_json("workspace_root must not be NULL");
    }
    let workspace_root = match unsafe { CStr::from_ptr(workspace_root) }.to_str() {
        Ok(s) if !s.is_empty() => s.to_string(),
        Ok(_) => return error_json("workspace_root must not be empty"),
        Err(e) => return error_json(&format!("invalid UTF-8 in workspace_root: {e}")),
    };
    ffi_catch(AssertUnwindSafe(move || {
        let paths = runtime::initialize_workspace(&workspace_root).map_err(|e| e.to_string())?;
        Ok(
            serde_json::to_string_pretty(&paths)
                .map_err(|e| format!("serialization error: {e}"))?,
        )
    }))
}

/// Import Sentra risk rules from a local zip archive into the active runtime
/// rule directories. `zip_path` must be an absolute path to a local zip file;
/// the caller is responsible for ensuring the file is a valid zip archive.
/// sentra_initialize must be called first.
///
/// Returns a JSON object string (caller must free with sentra_string_free).
/// On error, returns a JSON error envelope.
#[unsafe(no_mangle)]
pub extern "C" fn sentra_import_rules(zip_path: *const c_char) -> *mut c_char {
    if zip_path.is_null() {
        return error_json("zip_path must not be NULL");
    }
    let zip_path = match unsafe { CStr::from_ptr(zip_path) }.to_str() {
        Ok(s) if !s.is_empty() => s.to_string(),
        Ok(_) => return error_json("zip_path must not be empty"),
        Err(e) => return error_json(&format!("invalid UTF-8 in zip_path: {e}")),
    };
    ffi_catch(AssertUnwindSafe(move || {
        let rules = runtime::initialized_rule_directory_config()
            .ok_or_else(|| "sentra runtime is not initialized".to_string())?;
        let extracted = extract_rules_zip(Path::new(&zip_path)).map_err(|e| e.to_string())?;
        let mut store = RuleStore::new(rules);
        let extracted_dir = extracted
            .path()
            .to_str()
            .ok_or_else(|| "invalid temporary import path".to_string())?;
        let imported = store.import(extracted_dir).map_err(|e| e.to_string())?;
        store.refresh().map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&ImportRulesEnvelope {
            ok: true,
            code: "ok",
            imported: ImportResultDto::from(imported),
        })
        .map_err(|e| format!("serialization error: {e}"))?)
    }))
}

/// Collect all AI agent assets. Returns a JSON array string (caller must free
/// with sentra_string_free). On error, returns a JSON error envelope.
///
/// Pass NULL for home_path to use the current user's home directory.
#[unsafe(no_mangle)]
pub extern "C" fn sentra_collect_assets(home_path: *const c_char) -> *mut c_char {
    let home = if home_path.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(home_path) }.to_str() {
            Ok(s) => Some(s.to_string()),
            Err(e) => return error_json(&format!("invalid UTF-8 in home_path: {e}")),
        }
    };
    ffi_catch(AssertUnwindSafe(move || {
        let home = home.as_deref().map(Path::new);
        let assets = adapter::collect_all_assets(home).map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&assets)
            .map_err(|e| format!("serialization error: {e}"))?)
    }))
}

/// Scan skills. request_json is a JSON object (see ScanRequest schema).
/// Returns a JSON array string (caller must free with sentra_string_free).
/// On error, returns a JSON error envelope.
#[unsafe(no_mangle)]
pub extern "C" fn sentra_scan_skills(request_json: *const c_char) -> *mut c_char {
    let json_str = match unsafe { CStr::from_ptr(request_json) }.to_str() {
        Ok(s) => s.to_string(),
        Err(e) => return error_json(&format!("invalid UTF-8 in request_json: {e}")),
    };
    ffi_catch(AssertUnwindSafe(move || {
        let request: ScanRequest =
            serde_json::from_str(&json_str).map_err(|e| format!("invalid request JSON: {e}"))?;
        let results = adapter::scan_skills(&request).map_err(|e| e.to_string())?;
        Ok(serde_json::to_string_pretty(&results)
            .map_err(|e| format!("serialization error: {e}"))?)
    }))
}

fn ffi_catch<F>(f: AssertUnwindSafe<F>) -> *mut c_char
where
    F: FnOnce() -> Result<String, String>,
{
    match panic::catch_unwind(f) {
        Ok(Ok(json)) => CString::new(json).unwrap_or_default().into_raw(),
        Ok(Err(msg)) => error_json(&msg),
        Err(_panic) => error_json("internal panic"),
    }
}

fn error_json(message: &str) -> *mut c_char {
    let envelope = ErrorEnvelope {
        error: ErrorBody {
            code: "Error".to_string(),
            message: message.to_string(),
        },
    };
    let json = serde_json::to_string(&envelope).unwrap_or_else(|_| {
        r#"{"error":{"code":"Error","message":"serialization failure"}}"#.to_string()
    });
    CString::new(json).unwrap_or_default().into_raw()
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};
    use std::fs;

    use super::*;

    // ── helpers ────────────────────────────────────────────────────────

    /// Call sentra_collect_assets, read the returned C string, and parse as JSON.
    /// The returned pointer is leaked (tests are short-lived).
    fn call_collect(home: Option<&str>) -> serde_json::Value {
        let home_c = home.map(|s| CString::new(s).unwrap());
        let ptr = home_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
        let result = sentra_collect_assets(ptr);
        let json = unsafe { CStr::from_ptr(result) }
            .to_str()
            .unwrap()
            .to_string();
        serde_json::from_str(&json).unwrap()
    }

    /// Call sentra_scan_skills, read the returned C string, and parse as JSON.
    fn call_scan(request: &str) -> serde_json::Value {
        let req_c = CString::new(request).unwrap();
        let result = sentra_scan_skills(req_c.as_ptr());
        let json = unsafe { CStr::from_ptr(result) }
            .to_str()
            .unwrap()
            .to_string();
        serde_json::from_str(&json).unwrap()
    }

    /// Like call_scan but returns the raw JSON string for format inspection.
    fn call_scan_raw(request: &str) -> String {
        let req_c = CString::new(request).unwrap();
        let result = sentra_scan_skills(req_c.as_ptr());
        let s = unsafe { CStr::from_ptr(result) }
            .to_str()
            .unwrap()
            .to_string();
        s
    }

    // ── sentra_version ─────────────────────────────────────────────────

    #[test]
    fn version_returns_non_null() {
        let ptr = sentra_version();
        assert!(!ptr.is_null(), "version must not return NULL");
    }

    #[test]
    fn version_returns_valid_cstring() {
        let ptr = sentra_version();
        let s = unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .expect("version must be valid UTF-8");
        assert!(!s.is_empty(), "version must not be empty");
    }

    #[test]
    fn version_returns_correct_value() {
        let ptr = sentra_version();
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(
            s,
            env!("CARGO_PKG_VERSION"),
            "version must match Cargo.toml"
        );
    }

    #[test]
    fn version_returns_stable_pointer() {
        // The version string is a static — repeated calls return the same address.
        let p1 = sentra_version();
        let p2 = sentra_version();
        assert_eq!(p1, p2, "static version pointer must be stable across calls");
    }

    // ── sentra_string_free ─────────────────────────────────────────────

    #[test]
    fn string_free_accepts_null() {
        // Must not panic/crash.
        sentra_string_free(std::ptr::null_mut());
    }

    #[test]
    fn string_free_on_valid_collect_result() {
        let dir = tempfile::tempdir().unwrap();
        let home_c = CString::new(dir.path().to_str().unwrap()).unwrap();
        let result = sentra_collect_assets(home_c.as_ptr());
        assert!(!result.is_null(), "collect must return non-null");

        // Read the content BEFORE freeing (use-after-free is UB).
        let json = unsafe { CStr::from_ptr(result) }
            .to_str()
            .unwrap()
            .to_string();
        assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());

        // Free — must not crash.
        sentra_string_free(result);
    }

    // ── sentra_collect_assets ──────────────────────────────────────────

    #[test]
    fn collects_assets_via_ffi() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join(".codex").join("skills").join("ffi-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: ffi-test\n---\nbody").unwrap();

        let value = call_collect(Some(dir.path().to_str().unwrap()));
        let assets = value.as_array().unwrap();
        let skills: Vec<_> = assets.iter().filter(|a| a["type"] == "skill").collect();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["agent"], "codex");
    }

    #[test]
    fn collect_null_home_does_not_crash() {
        // NULL home uses the real user home — must not crash.
        let result = sentra_collect_assets(std::ptr::null());
        assert!(!result.is_null(), "must return non-null");
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(json).expect("collect(null) must return valid JSON");
        assert!(
            parsed.is_array() || parsed.get("error").is_some(),
            "collect(null) must return an array or an error envelope"
        );
    }

    #[test]
    fn collect_nonexistent_home_returns_empty_array() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("nonexistent");
        let value = call_collect(Some(nonexistent.to_str().unwrap()));
        let arr = value.as_array().expect("should return an array");
        assert!(arr.is_empty(), "nonexistent home should yield empty array");
    }

    #[test]
    fn collect_output_has_expected_fields() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join(".codex").join("skills").join("fields-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: fields-test\ndescription: Check fields\n---\nbody",
        )
        .unwrap();

        let value = call_collect(Some(dir.path().to_str().unwrap()));
        let skills: Vec<_> = value
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["type"] == "skill")
            .collect();
        assert_eq!(skills.len(), 1);
        let skill = &skills[0];
        // UnifiedAsset with rename_all="camelCase" + #[serde(rename="type")]
        assert!(skill.get("user").is_some(), "missing field: user");
        assert!(skill.get("agent").is_some(), "missing field: agent");
        assert!(
            skill.get("agentTitle").is_some(),
            "missing field: agentTitle"
        );
        assert!(skill.get("agentHome").is_some(), "missing field: agentHome");
        assert!(skill.get("type").is_some(), "missing field: type");
        assert!(skill.get("data").is_some(), "missing field: data");
        assert!(
            skill.get("data").unwrap().is_array(),
            "data must be an array"
        );
    }

    #[test]
    fn collect_invalid_utf8_returns_error_envelope() {
        // Create a CString with bytes that are NOT valid UTF-8.
        let invalid: &[u8] = b"\xc0\x80"; // overlong encoding, invalid UTF-8
        let bytes_with_nul: Vec<u8> = [invalid, b"\0"].concat();
        let c_str = CStr::from_bytes_with_nul(&bytes_with_nul).unwrap();
        let result = sentra_collect_assets(c_str.as_ptr());
        assert!(!result.is_null());
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let err = value
            .get("error")
            .expect("invalid UTF-8 must produce error envelope");
        assert_eq!(err["code"], "Error");
        assert!(err["message"].as_str().unwrap().contains("UTF-8"));
    }

    // ── sentra_scan_skills ─────────────────────────────────────────────

    #[test]
    fn scan_skills_via_ffi() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("ffi-scan");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: ffi-scan\n---\nbody").unwrap();

        let request = serde_json::json!({ "path": skill_dir }).to_string();
        let value = call_scan(&request);
        let results = value.as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["data"]["name"], "ffi-scan");
    }

    #[test]
    fn scan_skills_invalid_json_returns_error() {
        let value = call_scan("not valid json");
        assert!(value.get("error").is_some(), "should return error envelope");
    }

    #[test]
    fn scan_skills_error_envelope_has_correct_format() {
        let value = call_scan("not valid json");
        let err = value.get("error").expect("must have error key");
        assert_eq!(err["code"], "Error", "error.code must be 'Error'");
        assert!(
            err["message"].as_str().unwrap().len() > 0,
            "error.message must be non-empty"
        );
        // Ensure no other top-level keys.
        assert_eq!(
            value.as_object().unwrap().len(),
            1,
            "error envelope must have only 'error' key"
        );
    }

    #[test]
    fn scan_skills_nonexistent_path_returns_error_envelope() {
        let request = serde_json::json!({
            "path": "/tmp/nonexistent-sentra-ffi-test-dir"
        })
        .to_string();
        let value = call_scan(&request);
        let err = value
            .get("error")
            .expect("nonexistent path must produce error");
        assert_eq!(err["code"], "Error");
    }

    #[test]
    fn scan_skills_result_has_expected_structure() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("struct-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: struct-test\n---\nbody",
        )
        .unwrap();

        let request = serde_json::json!({ "path": skill_dir }).to_string();
        let value = call_scan(&request);
        let results = value.as_array().unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];

        // Top-level fields
        assert!(r.get("assetType").is_some(), "missing: assetType");
        assert!(r.get("source").is_some(), "missing: source");
        assert!(r.get("data").is_some(), "missing: data");
        assert!(r.get("report").is_some(), "missing: report");
        // agentName/agentTitle/agentHome are skip_serializing_if=None for path scans
        assert!(
            r.get("agentName").is_none(),
            "agentName should be absent for path scans"
        );
        assert!(
            r.get("agentTitle").is_none(),
            "agentTitle should be absent for path scans"
        );

        // Report structure
        let report = &r["report"];
        assert!(report.get("metadata").is_some(), "missing: report.metadata");
        assert!(report.get("summary").is_some(), "missing: report.summary");
        assert!(report.get("findings").is_some(), "missing: report.findings");
        assert!(report.get("errors").is_some(), "missing: report.errors");

        // Metadata fields — ScanMetadata does NOT use rename_all,
        // so fields are serialized as-is: scanner, scan_time, scan_duration_ms.
        let meta = &report["metadata"];
        assert!(meta.get("scanner").is_some(), "missing: metadata.scanner");
        assert!(
            meta.get("scan_time").is_some(),
            "missing: metadata.scan_time"
        );
        assert!(
            meta.get("scan_duration_ms").is_some(),
            "missing: metadata.scan_duration_ms"
        );

        // Summary fields
        let summary = &report["summary"];
        for field in &["critical", "high", "medium", "low", "info"] {
            assert!(summary.get(field).is_some(), "missing: summary.{field}");
            assert!(summary[field].is_u64(), "summary.{field} must be a number");
        }

        // Findings and errors must be arrays
        assert!(report["findings"].is_array(), "findings must be an array");
        assert!(report["errors"].is_array(), "errors must be an array");
    }

    #[test]
    fn scan_skills_result_fields_are_camelcase() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("camel-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: camel-test\n---\nbody",
        )
        .unwrap();

        let request = serde_json::json!({ "path": skill_dir }).to_string();
        let raw = call_scan_raw(&request);
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let r = &value.as_array().unwrap()[0];

        // Verify snake_case fields are NOT present (camelCase should be used)
        assert!(
            r.get("asset_type").is_none(),
            "must not contain snake_case: asset_type"
        );
        assert!(
            r.get("agent_name").is_none(),
            "must not contain snake_case: agent_name"
        );
        assert!(
            r.get("agent_title").is_none(),
            "must not contain snake_case: agent_title"
        );
        assert!(
            r.get("agent_home").is_none(),
            "must not contain snake_case: agent_home"
        );

        let report = &r["report"];
        // ScanMetadata does NOT use serde rename_all, so scan_time / scan_duration_ms
        // are the correct field names inside metadata (not camelCase scanTime / scanDurationMs).
        let meta = &report["metadata"];
        assert!(
            meta.get("scan_time").is_some(),
            "metadata.scan_time must be present (ScanMetadata has no rename_all)"
        );
        assert!(
            meta.get("scan_duration_ms").is_some(),
            "metadata.scan_duration_ms must be present (ScanMetadata has no rename_all)"
        );
    }

    #[test]
    fn scan_skills_with_all_checkers_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("disabled-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: disabled-test\n---\nbody",
        )
        .unwrap();

        let request = serde_json::json!({
            "path": skill_dir,
            "checkers": {
                "hash": false,
                "yara": false,
                "ti": false,
                "llm": false,
                "onlineTi": false
            }
        })
        .to_string();
        let value = call_scan(&request);
        let results = value.as_array().unwrap();
        assert_eq!(results.len(), 1);
        let report = &results[0]["report"];
        // With all checkers disabled, findings should be empty and no errors.
        let findings = report["findings"].as_array().unwrap();
        assert!(findings.is_empty(), "all checkers disabled → zero findings");
        let errors = report["errors"].as_array().unwrap();
        assert!(errors.is_empty(), "all checkers disabled → zero errors");
    }

    #[test]
    fn scan_skills_request_accepts_camelcase_fields() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("camel-req-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: camel-req-test\n---\nbody",
        )
        .unwrap();

        // Request fields are deserialized with rename_all="camelCase",
        // so "onlineTi" (camelCase) should work.
        let request = serde_json::json!({
            "path": skill_dir,
            "checkers": { "onlineTi": true }
        })
        .to_string();
        let value = call_scan(&request);
        assert!(
            value.is_array(),
            "valid camelCase request must return array"
        );
    }

    #[test]
    fn scan_skills_utf8_error_returns_error_envelope() {
        // Pass invalid UTF-8 bytes as the request JSON.
        let invalid: &[u8] = b"\xc0\x80";
        let bytes_with_nul: Vec<u8> = [invalid, b"\0"].concat();
        let c_str = CStr::from_bytes_with_nul(&bytes_with_nul).unwrap();
        let result = sentra_scan_skills(c_str.as_ptr());
        assert!(!result.is_null());
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert!(
            value.get("error").is_some(),
            "invalid UTF-8 must return error envelope"
        );
    }

    #[test]
    fn roundtrip_free_does_not_crash() {
        // Integration: scan → read → free → no crash.
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("roundtrip-test");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: roundtrip-test\n---\nbody",
        )
        .unwrap();

        let request = serde_json::json!({ "path": skill_dir }).to_string();
        let req_c = CString::new(request).unwrap();
        let result = sentra_scan_skills(req_c.as_ptr());
        assert!(!result.is_null());

        // Read then free.
        let json = unsafe { CStr::from_ptr(result) }
            .to_str()
            .unwrap()
            .to_string();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.is_array());

        sentra_string_free(result);
        // If we get here without crash or UB, the test passes.
    }
}
