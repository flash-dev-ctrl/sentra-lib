use std::fs;

#[test]
fn crate_root_exposes_direct_public_surface() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).unwrap();

    assert!(
        !root.join("src/sdk.rs").exists(),
        "sdk facade source must be removed after merging into crate root"
    );
    assert!(
        !lib_rs.contains("pub mod sdk;"),
        "sdk facade must not remain public; consumers should use sentra_lib root modules"
    );
    assert!(
        !lib_rs.contains("pub use sdk::*;"),
        "crate root must expose explicit public modules instead of re-exporting a sdk facade"
    );

    for module in ["agents", "config", "interfaces"] {
        assert!(
            lib_rs.contains(&format!("pub mod {module};")),
            "public module `{module}` must be exposed at sentra_lib::{module}"
        );
    }
    assert!(
        lib_rs.contains("pub use crate::utils::protocol;"),
        "public module `protocol` must be exposed at sentra_lib::protocol"
    );
    assert!(
        lib_rs.contains("pub mod users {"),
        "public module `users` must be exposed at sentra_lib::users"
    );
    assert!(
        lib_rs.contains("pub use error::{SentraError, SentraResult};"),
        "crate root must expose SentraError and SentraResult"
    );
}

#[test]
fn binding_files_are_removed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    assert!(
        !root.join("src/bin/sentra-rt-binding.rs").exists(),
        "Rust binding binary must not exist; CLI should call library API directly"
    );
    assert!(
        !root.join("binding").exists(),
        "TypeScript binding package must not exist"
    );
}

#[test]
fn agents_module_exports_only_public_discovery_surface() {
    let agents_rs =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/agents/mod.rs")).unwrap();

    assert!(agents_rs.contains("pub use base::Agent;"));
    assert!(agents_rs.contains("pub use discovery::discover_agents;"));
    for symbol in [
        "pub mod claude_app",
        "pub mod claude_cli",
        "pub mod codex",
        "pub mod general",
        "pub mod hermes",
        "pub mod openclaw",
        "pub mod sentra",
        "pub use discovery::{discover_agents, get_agent_title}",
        "pub use entries::{",
    ] {
        assert!(
            !agents_rs.contains(symbol),
            "sentra_lib::agents must not expose internal symbol `{symbol}`"
        );
    }
}

#[test]
fn crate_root_users_exports_only_combined_user_listing() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();
    let users_block = lib_rs
        .split("pub mod users {")
        .nth(1)
        .and_then(|text| text.split("\n}").next())
        .unwrap();

    assert!(users_block.contains("UserHome"));
    assert!(users_block.contains("list_users"));
    for symbol in [
        "current_user_home",
        "list_local_users",
        "list_container_users",
    ] {
        assert!(
            !users_block.contains(symbol),
            "sentra_lib::users must not export `{symbol}`"
        );
    }
}

#[test]
fn crate_root_exports_skill_directory_discovery_by_name() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();

    assert!(
        lib_rs.contains("pub use crate::utils::collect_skills_from_dir;"),
        "crate root must expose collect_skills_from_dir for CLI path scans"
    );
    assert!(
        !lib_rs.contains("discover_skill"),
        "crate root must not alias collect_skills_from_dir as discover_skill"
    );
    assert!(
        !lib_rs.contains("utils_copy_dir_all"),
        "crate root must not expose copy_dir_all through a CLI-only alias"
    );
}

#[test]
fn config_module_exports_standard_paths_and_config_shape() {
    let config_rs =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/config.rs")).unwrap();

    for symbol in [
        "pub const SENTRA_HOME_DIR_NAME",
        "pub const SENTRA_CONFIG_FILE_NAME",
        "pub const SENTRA_HASH_RULE_DIR_NAME",
        "pub const SENTRA_YARA_RULE_DIR_NAME",
        "pub const SENTRA_TI_RULE_DIR_NAME",
        "pub struct SentraConfig",
        "pub struct LlmConfig",
        "pub checker: Option<CheckerConfig>",
        "pub llm: Option<LlmConfig>",
        "pub rules: Option<RuleDirectoryConfig>",
        "pub online_ti: Option<OnlineTiConfig>",
        "pub fn sentra_home",
        "pub fn sentra_config_file",
        "pub fn sentra_hash_rule_dir",
        "pub fn sentra_yara_rule_dir",
        "pub fn sentra_ti_rule_dir",
    ] {
        assert!(
            config_rs.contains(symbol),
            "config module must export `{symbol}`"
        );
    }
    assert!(
        !config_rs.contains("pub scan: Option<ScanOptions>"),
        "SentraConfig must be flat and must not nest scan: Option<ScanOptions>"
    );
    assert!(
        !config_rs.contains("pub prompt: Option<String>"),
        "sentra_lib::config::LlmConfig must not expose prompt"
    );
    for symbol in [
        "pub const SENTRA_HOME_DIR: &str = \"~/.sentra\"",
        "pub const SENTRA_CONFIG_FILE: &str = \"~/.sentra/config.json\"",
        "pub const SENTRA_HASH_RULE_DIR: &str = \"~/.sentra/hash\"",
        "pub const SENTRA_YARA_RULE_DIR: &str = \"~/.sentra/yara\"",
        "pub const SENTRA_TI_RULE_DIR: &str = \"~/.sentra/ti\"",
    ] {
        assert!(
            !config_rs.contains(symbol),
            "config module must not export platform-specific path string `{symbol}`"
        );
    }
}

#[test]
fn crate_root_does_not_expose_binding_module() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();

    assert!(
        !lib_rs.contains("pub mod binding;"),
        "crate root must not expose the legacy binding-specific facade"
    );
    assert!(
        !lib_rs.contains("pub mod ffi"),
        "crate root must not expose the legacy ffi module"
    );
    assert!(
        !lib_rs.contains("pub use binding::*"),
        "crate root must not re-export the legacy binding module"
    );
}

#[test]
fn crate_root_does_not_use_snapshot_asset_api_names() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();

    for symbol in [
        "AgentSnapshot",
        "AssetSnapshot",
        "discover_agent_snapshots",
        "asset_snapshots",
        "snapshot_asset",
    ] {
        assert!(
            !lib_rs.contains(symbol),
            "public API must not use binding-era snapshot name `{symbol}`"
        );
    }
}

#[test]
fn crate_root_does_not_expose_cli_asset_listing_helpers() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();

    for symbol in [
        "pub mod assets",
        "pub use assets::*",
        "pub fn list_agents",
        "pub fn list_assets",
        "pub fn set_provider_asset_data",
        "AgentRecord",
        "AssetRecord",
    ] {
        assert!(
            !lib_rs.contains(symbol),
            "public API must not expose CLI asset helper `{symbol}`"
        );
    }
}

#[test]
fn crate_root_uses_explicit_grouped_exports_only() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();

    for symbol in [
        "pub use interfaces::*",
        "pub use protocol::*",
        "pub use users::*",
        "pub use utils::*",
        "pub mod utils",
        "download_source_to_temp",
    ] {
        assert!(
            !lib_rs.contains(symbol),
            "crate root must not expose broad or unused export `{symbol}`"
        );
    }
}

#[test]
fn unused_download_utils_are_moved_to_trash() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    assert!(
        !root.join("src/utils/download.rs").exists(),
        "unused download utilities must not remain in src"
    );
}

#[test]
fn crate_root_protocol_exports_model_interaction_helpers_explicitly() {
    let lib_rs = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();
    assert!(
        lib_rs.contains("pub use crate::utils::protocol;"),
        "crate root must expose sentra_lib::protocol without a duplicated export list"
    );
    assert!(
        !lib_rs.contains("pub mod protocol {"),
        "crate root protocol export should stay collapsed to the utils::protocol module"
    );
}

#[test]
fn obsolete_protocol_helpers_are_not_restored_under_old_names() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let protocol_rs =
        fs::read_to_string(root.join("src/utils/protocol.rs")).expect("read protocol source");

    for symbol in [
        "pub struct ModelEntry",
        "pub fn detect_protocols",
        "pub fn fetch_models",
        "pub fn normalize_api_base_url",
        "pub fn probe_provider_request",
        "pub fn build_model_request",
        "pub fn extract_model_response_text",
        "pub fn model_api_url",
        "pub fn standard_api_url",
        "pub struct ModelHttpRequest",
    ] {
        assert!(
            !protocol_rs.contains(symbol),
            "obsolete protocol helper `{symbol}` must not be restored under the old API name"
        );
    }
}
