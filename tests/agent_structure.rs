use std::path::Path;

#[test]
fn rust_agents_mirror_typescript_agent_object_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src").join("agents");

    assert!(
        !src.join("assets.rs").exists(),
        "agent asset implementations must live in each agent directory"
    );

    for file in ["base.rs", "discovery.rs", "entries.rs", "mod.rs"] {
        assert!(src.join(file).is_file(), "missing shared agent file {file}");
    }

    assert_agent_files(
        &src,
        "codex",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "cron.rs",
            "provider.rs",
        ],
    );
    assert_agent_files(
        &src,
        "claude",
        &[
            "mod.rs",
            "meta.rs",
            "install.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "cron.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
            "app_meta.rs",
            "app_skill.rs",
            "app_mcp.rs",
            "app_memory.rs",
            "app_cron.rs",
            "app_provider.rs",
            "app_process.rs",
        ],
    );
    for agent in ["hermes", "openclaw"] {
        assert_agent_files(
            &src,
            agent,
            &[
                "mod.rs",
                "meta.rs",
                "skill.rs",
                "mcp.rs",
                "memory.rs",
                "cron.rs",
                "provider.rs",
            ],
        );
    }
    assert_agent_files(
        &src,
        "kimi",
        &[
            "mod.rs",
            "meta.rs",
            "install.rs",
            "skill.rs",
            "mcp.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "opencode",
        &["mod.rs", "meta.rs", "skill.rs", "mcp.rs", "provider.rs"],
    );
    assert_agent_files(
        &src,
        "pi",
        &["mod.rs", "meta.rs", "install.rs", "skill.rs", "provider.rs"],
    );
    assert_agent_files(&src, "general", &["mod.rs", "meta.rs", "skill.rs"]);
    assert_agent_files(
        &src,
        "antigravity",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "cron.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "codebuddy",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "coder",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "cursor",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "cron.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "kiro",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "cron.rs",
            "provider.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "lingcode",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "provider.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "marvis",
        &[
            "mod.rs",
            "meta.rs",
            "mcp.rs",
            "memory.rs",
            "provider.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "qoder",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "cron.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "qoderwork",
        &["mod.rs", "meta.rs", "skill.rs", "memory.rs", "process.rs"],
    );
    assert_agent_files(
        &src,
        "trae",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "memory.rs",
            "cron.rs",
            "provider.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "vscode",
        &[
            "mod.rs",
            "meta.rs",
            "skill.rs",
            "mcp.rs",
            "cron.rs",
            "provider.rs",
            "plugin.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "workbuddy",
        &[
            "mod.rs",
            "meta.rs",
            "install.rs",
            "skill.rs",
            "mcp.rs",
            "provider.rs",
            "process.rs",
        ],
    );
    assert_agent_files(
        &src,
        "sentra",
        &["mod.rs", "meta.rs", "skill.rs", "provider.rs"],
    );
}

#[test]
fn each_agent_module_owns_discovery() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src").join("agents");
    let shared_discovery = std::fs::read_to_string(src.join("discovery.rs")).unwrap();

    for module in [
        "claude",
        "codex",
        "antigravity",
        "codebuddy",
        "coder",
        "cursor",
        "general",
        "hermes",
        "kimi",
        "kiro",
        "lingcode",
        "marvis",
        "openclaw",
        "opencode",
        "pi",
        "qoder",
        "qoderwork",
        "sentra",
        "trae",
        "vscode",
        "workbuddy",
    ] {
        let content = std::fs::read_to_string(src.join(module).join("mod.rs")).unwrap();
        assert!(
            content.contains("pub(crate) fn discover_agents"),
            "{module}/mod.rs must keep discover_agents crate-internal"
        );
        assert!(
            shared_discovery.contains(&format!("{module}::discover_agents")),
            "shared discover_agents must aggregate {module}::discover_agents"
        );
    }
}

#[test]
fn agent_asset_modules_are_not_public() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src").join("agents");

    for module in [
        "claude",
        "codex",
        "antigravity",
        "codebuddy",
        "coder",
        "cursor",
        "general",
        "hermes",
        "kimi",
        "kiro",
        "lingcode",
        "marvis",
        "openclaw",
        "opencode",
        "pi",
        "qoder",
        "qoderwork",
        "sentra",
        "trae",
        "vscode",
        "workbuddy",
    ] {
        let content = std::fs::read_to_string(src.join(module).join("mod.rs")).unwrap();
        assert!(
            !content.contains("pub mod "),
            "{module}/mod.rs must not expose asset modules"
        );
    }
}

#[test]
fn concrete_asset_types_are_agent_module_internal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let agents_dir = root.join("src").join("agents");
    let mut stack = vec![agents_dir.clone()];

    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some("mod.rs") {
                continue;
            }
            if path.parent() == Some(agents_dir.as_path()) {
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }

            let content = std::fs::read_to_string(&path).unwrap();
            assert!(
                !content.contains("pub struct "),
                "{} must not expose concrete asset structs",
                path.strip_prefix(root).unwrap().display()
            );
            assert!(
                !content.contains("pub fn new("),
                "{} must not expose concrete asset constructors",
                path.strip_prefix(root).unwrap().display()
            );
        }
    }
}

#[test]
fn agent_module_wrappers_do_not_expose_asset_mutator_shortcuts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src").join("agents");

    for module in [
        "claude",
        "codex",
        "antigravity",
        "codebuddy",
        "coder",
        "cursor",
        "general",
        "hermes",
        "kimi",
        "kiro",
        "lingcode",
        "marvis",
        "openclaw",
        "opencode",
        "pi",
        "qoder",
        "qoderwork",
        "sentra",
        "trae",
        "vscode",
        "workbuddy",
    ] {
        let content = std::fs::read_to_string(src.join(module).join("mod.rs")).unwrap();
        for forbidden in [
            "pub fn add_",
            "pub fn del_",
            "pub fn delete_",
            "pub fn remove_",
            "pub fn set_",
            "pub fn update_",
        ] {
            assert!(
                !content.contains(forbidden),
                "{module}/mod.rs must only expose standard agent wrapper methods; move {forbidden} shortcuts into asset modules or SDK APIs"
            );
        }
    }
}

#[test]
fn meta_assets_do_not_expose_install_shortcuts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let agents_dir = root.join("src").join("agents");

    for module in ["claude", "codex"] {
        let content = std::fs::read_to_string(agents_dir.join(module).join("meta.rs")).unwrap();
        for forbidden in ["fn install(", "fn uninstall("] {
            assert!(
                !content.contains(forbidden),
                "{module}/meta.rs must not expose {forbidden}; use sentra_lib::agents install/uninstall APIs"
            );
        }
    }

    let interfaces = std::fs::read_to_string(root.join("src").join("interfaces.rs")).unwrap();
    for forbidden in ["fn install(&self)", "fn uninstall(&self)"] {
        assert!(
            !interfaces.contains(forbidden),
            "ErasedAsset must not expose agent CLI lifecycle shortcut {forbidden}"
        );
    }
}

#[test]
fn agent_modules_do_not_define_redundant_typed_wrappers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src").join("agents");

    for module in [
        "claude",
        "codex",
        "antigravity",
        "codebuddy",
        "coder",
        "cursor",
        "general",
        "hermes",
        "kimi",
        "kiro",
        "lingcode",
        "marvis",
        "openclaw",
        "opencode",
        "qoder",
        "qoderwork",
        "sentra",
        "trae",
        "vscode",
        "workbuddy",
    ] {
        let content = std::fs::read_to_string(src.join(module).join("mod.rs")).unwrap();
        assert!(
            !content.contains("pub struct "),
            "{module}/mod.rs must use the shared Agent type instead of defining a typed wrapper"
        );
        assert!(
            !content.contains(&format!("pub fn discover_{module}")),
            "{module}/mod.rs must expose only discover_agents, not discover_{module}"
        );
    }
}

#[test]
fn agent_entries_are_defined_in_shared_entries_file() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let agents_dir = root.join("src").join("agents");
    let mod_rs = std::fs::read_to_string(agents_dir.join("mod.rs")).unwrap();
    let entries_rs = std::fs::read_to_string(agents_dir.join("entries.rs")).unwrap();

    for symbol in [
        "CLAUDE_APP_AGENT_ENTRY",
        "CLAUDE_CLI_IDE_AGENT_ENTRY",
        "CLAUDE_CLI_AGENT_ENTRY",
        "CODEX_APP_AGENT_ENTRY",
        "CODEX_CLI_AGENT_ENTRY",
        "CODEX_CLI_IDE_AGENT_ENTRY",
        "ANTIGRAVITY_AGENT_ENTRY",
        "CODEBUDDY_AGENT_ENTRY",
        "CODER_AGENT_ENTRY",
        "CURSOR_AGENT_ENTRY",
        "GENERAL_AGENT_ENTRIES",
        "HERMES_AGENT_ENTRY",
        "KIMI_APP_AGENT_ENTRY",
        "KIMI_CLI_AGENT_ENTRY",
        "KIMI_CLI_IDE_AGENT_ENTRY",
        "KIRO_AGENT_ENTRY",
        "LINGCODE_AGENT_ENTRY",
        "MARVIS_AGENT_ENTRY",
        "OPENCLAW_AGENT_ENTRY",
        "OPENCODE_AGENT_ENTRY",
        "PI_AGENT_ENTRY",
        "QODER_AGENT_ENTRY",
        "QODER_CN_AGENT_ENTRY",
        "QODER_AGENT_ENTRIES",
        "QODERWORK_AGENT_ENTRY",
        "SENTRA_AGENT_ENTRY",
        "TRAE_AGENT_ENTRY",
        "VSCODE_AGENT_ENTRY",
        "WORKBUDDY_AGENT_ENTRY",
    ] {
        assert!(
            !mod_rs.contains(&format!("pub use {symbol}")),
            "agents/mod.rs must not re-export {symbol}; keep agent entries in entries.rs"
        );
        assert!(
            !mod_rs.contains(&format!("pub(crate) use {symbol}")),
            "agents/mod.rs must not crate-re-export {symbol}; keep agent entries referenced through entries.rs"
        );
        assert!(
            entries_rs.contains(symbol),
            "agents/entries.rs must define or expose {symbol}"
        );
    }

    assert!(
        !mod_rs.contains("pub(crate) use entries::"),
        "agents/mod.rs must not crate-re-export internal entry registry symbols"
    );
    assert!(
        !mod_rs.contains("pub(crate) use discovery::get_agent_title"),
        "agents/mod.rs must not crate-re-export internal discovery helpers"
    );
}

#[test]
fn agent_modules_reference_entry_registry_directly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src").join("agents");

    for (module, symbol) in [
        ("claude", "CLAUDE_APP_AGENT_ENTRY"),
        ("claude", "CLAUDE_CLI_IDE_AGENT_ENTRY"),
        ("claude", "CLAUDE_CLI_AGENT_ENTRY"),
        ("codex", "CODEX_CLI_AGENT_ENTRY"),
        ("codex", "CODEX_CLI_IDE_AGENT_ENTRY"),
        ("antigravity", "ANTIGRAVITY_AGENT_ENTRY"),
        ("codebuddy", "CODEBUDDY_AGENT_ENTRY"),
        ("coder", "CODER_AGENT_ENTRY"),
        ("cursor", "CURSOR_AGENT_ENTRY"),
        ("hermes", "HERMES_AGENT_ENTRY"),
        ("kimi", "KIMI_APP_AGENT_ENTRY"),
        ("kimi", "KIMI_CLI_AGENT_ENTRY"),
        ("kimi", "KIMI_CLI_IDE_AGENT_ENTRY"),
        ("kiro", "KIRO_AGENT_ENTRY"),
        ("lingcode", "LINGCODE_AGENT_ENTRY"),
        ("marvis", "MARVIS_AGENT_ENTRY"),
        ("openclaw", "OPENCLAW_AGENT_ENTRY"),
        ("opencode", "OPENCODE_AGENT_ENTRY"),
        ("pi", "PI_AGENT_ENTRY"),
        ("qoder", "QODER_AGENT_ENTRIES"),
        ("qoderwork", "QODERWORK_AGENT_ENTRY"),
        ("sentra", "SENTRA_AGENT_ENTRY"),
        ("trae", "TRAE_AGENT_ENTRY"),
        ("vscode", "VSCODE_AGENT_ENTRY"),
        ("workbuddy", "WORKBUDDY_AGENT_ENTRY"),
    ] {
        let content = std::fs::read_to_string(src.join(module).join("mod.rs")).unwrap();
        assert!(
            content.contains(&format!("crate::agents::entries::{symbol}")),
            "{module}/mod.rs must reference {symbol} through the private entries module"
        );
        assert!(
            !content.contains(&format!("crate::agents::{symbol}")),
            "{module}/mod.rs must not rely on agents/mod.rs re-exporting {symbol}"
        );
    }

    let general = std::fs::read_to_string(src.join("general").join("mod.rs")).unwrap();
    for symbol in ["GENERAL_AGENT_ENTRIES", "SYSTEM_AGENT_PATHS"] {
        assert!(
            general.contains(&format!("crate::agents::entries::{symbol}")),
            "general/mod.rs must reference {symbol} through the private entries module"
        );
        assert!(
            !general.contains(&format!("crate::agents::{symbol}")),
            "general/mod.rs must not rely on agents/mod.rs re-exporting {symbol}"
        );
    }
}

#[test]
fn base_agent_exposes_agent_accessors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base_rs = std::fs::read_to_string(root.join("src").join("agents").join("base.rs")).unwrap();
    let agent_impl = base_rs
        .split("impl Agent {")
        .nth(1)
        .and_then(|text| text.split("\n}\n").next())
        .unwrap();

    for method in ["pub fn name(", "pub fn title(", "pub fn home("] {
        assert!(
            agent_impl.contains(method),
            "Agent impl must expose {method}"
        );
    }
}

#[test]
fn migrated_agents_keep_asset_logic_in_each_asset_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("codex"),
        &[
            "fn meta_data",
            "fn mcp_data",
            "fn memory_data",
            "fn cron_data",
            "fn provider_data",
        ],
        &["meta.rs", "mcp.rs", "memory.rs", "cron.rs", "provider.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("sentra"),
        &[
            "fn meta_data",
            "fn provider_data",
            "fn set_provider_data",
            "fn delete_provider_data",
        ],
        &["meta.rs", "provider.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("general"),
        &["fn meta_data"],
        &["meta.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("pi"),
        &["fn meta_data", "fn provider_data"],
        &["meta.rs", "provider.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("claude"),
        &[
            "fn meta_data",
            "fn mcp_data",
            "fn memory_data",
            "fn cron_data",
            "fn provider_data",
        ],
        &["meta.rs", "mcp.rs", "memory.rs", "cron.rs", "provider.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("claude"),
        &[
            "fn meta_data",
            "fn mcp_data",
            "fn memory_data",
            "fn cron_data",
            "fn provider_data",
        ],
        &[
            "app_meta.rs",
            "app_mcp.rs",
            "app_memory.rs",
            "app_cron.rs",
            "app_provider.rs",
        ],
    );
    for agent in ["hermes", "openclaw"] {
        assert_asset_logic_is_colocated(
            &root.join("src").join("agents").join(agent),
            &[
                "fn meta_data",
                "fn mcp_data",
                "fn memory_data",
                "fn cron_data",
                "fn provider_data",
            ],
            &["meta.rs", "mcp.rs", "memory.rs", "cron.rs", "provider.rs"],
        );
    }
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("kimi"),
        &["fn meta_data", "fn mcp_data", "fn provider_data"],
        &["meta.rs", "mcp.rs", "provider.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("opencode"),
        &["fn meta_data", "fn mcp_data", "fn provider_data"],
        &["meta.rs", "mcp.rs", "provider.rs"],
    );
    assert_asset_logic_is_colocated(
        &root.join("src").join("agents").join("workbuddy"),
        &["fn meta_data", "fn mcp_data", "fn provider_data"],
        &["meta.rs", "mcp.rs", "provider.rs"],
    );
}

#[test]
fn agents_do_not_depend_on_risks_or_skills_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let agents_dir = root.join("src").join("agents");
    let mut stack = vec![agents_dir];

    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap();
            for forbidden in ["crate::risks", "crate::skills"] {
                assert!(
                    !content.contains(forbidden),
                    "{} must not depend on {forbidden}",
                    path.strip_prefix(root).unwrap().display()
                );
            }
        }
    }
}

fn assert_asset_logic_is_colocated(dir: &Path, helpers: &[&str], asset_files: &[&str]) {
    let mod_rs = std::fs::read_to_string(dir.join("mod.rs")).unwrap();
    for helper in helpers {
        assert!(
            !mod_rs.contains(helper),
            "{}/mod.rs must only route assets; move {helper} into its asset module",
            dir.file_name().unwrap().to_string_lossy()
        );
    }

    for file in asset_files {
        let content = std::fs::read_to_string(dir.join(file)).unwrap();
        assert!(
            !content.contains("super::"),
            "{}/{} should own its parsing logic instead of delegating back to mod.rs",
            dir.file_name().unwrap().to_string_lossy(),
            file
        );
    }
}

fn assert_agent_files(src: &Path, agent: &str, files: &[&str]) {
    let dir = src.join(agent);
    assert!(dir.is_dir(), "missing agent directory {agent}");
    for file in files {
        assert!(
            dir.join(file).is_file(),
            "missing {agent}/{file}; keep agent responsibilities colocated"
        );
    }
}
