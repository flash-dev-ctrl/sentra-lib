use std::fs;

use sentra_lib::agents::discover_agents;
use sentra_lib::collect_skills_from_dir;
use sentra_lib::interfaces::{
    AssetType, CronData, CronType, McpData, McpType, MetaData, PluginData, PluginInstallSource,
    PluginSourceKind, ProcessData, ProviderData, ProviderModel, SkillData,
};
use sentra_lib::protocol::WireProtocol;

#[test]
fn schema_round_trips_skill_data() {
    let skill = SkillData {
        name: "demo".to_string(),
        description: Some("A demo skill".to_string()),
        enabled: Some(true),
        tags: vec!["safe".to_string()],
        home: Some("/tmp/demo".into()),
        source: None,
        ..SkillData::default()
    };

    let json = serde_json::to_string(&skill).unwrap();
    let decoded: SkillData = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.name, "demo");
    assert_eq!(decoded.tags, vec!["safe"]);
    assert_eq!(decoded.enabled, Some(true));
}

#[test]
fn schema_round_trips_plugin_data() {
    let plugin = PluginData {
        id: Some("market/demo@1.0.0".to_string()),
        name: "demo".to_string(),
        display_name: Some("Demo Plugin".to_string()),
        enabled: Some(true),
        install_source: Some(PluginInstallSource {
            kind: PluginSourceKind::Marketplace,
            reference: "market/demo@1.0.0".to_string(),
            marketplace: Some("market".to_string()),
        }),
        capabilities: vec!["skills".to_string()],
        ..PluginData::default()
    };

    let json = serde_json::to_string(&plugin).unwrap();
    let decoded: PluginData = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.name, "demo");
    assert_eq!(decoded.display_name.as_deref(), Some("Demo Plugin"));
    assert_eq!(
        decoded.install_source.unwrap().kind,
        PluginSourceKind::Marketplace
    );
    assert_eq!(decoded.capabilities, vec!["skills"]);
}

#[test]
fn schema_round_trips_meta_installed_status() {
    let meta = MetaData {
        id: Some("codex".to_string()),
        name: "Codex".to_string(),
        installed: true,
        home: Some("/tmp/.codex".into()),
        ..MetaData::default()
    };

    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(json["installed"], true);

    let legacy: MetaData = serde_json::from_str(r#"{"name":"Codex"}"#).unwrap();
    assert!(!legacy.installed);
}

#[test]
fn schema_round_trips_process_data() {
    let mut env = std::collections::BTreeMap::new();
    env.insert("PATH".to_string(), "/usr/bin".to_string());
    env.insert("OPENAI_API_KEY".to_string(), "sk-****7890".to_string());

    let process = ProcessData {
        pid: 42,
        name: "codex".to_string(),
        cmdline: vec!["codex".to_string(), "--sandbox".to_string()],
        started_at: 1_700_000_000,
        run_time_seconds: 3_661,
        path: Some("/usr/local/bin/codex".into()),
        env,
    };

    let json = serde_json::to_value(&process).unwrap();
    assert_eq!(json["pid"], 42);
    assert_eq!(json["name"], "codex");
    assert_eq!(json["cmdline"][0], "codex");
    assert_eq!(json["startedAt"], 1_700_000_000);
    assert_eq!(json["runTimeSeconds"], 3_661);
    assert_eq!(json["env"]["OPENAI_API_KEY"], "sk-****7890");

    let decoded: ProcessData = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.pid, 42);
    assert_eq!(decoded.started_at, 1_700_000_000);
    assert_eq!(decoded.run_time_seconds, 3_661);
    assert_eq!(
        decoded.env.get("PATH").map(String::as_str),
        Some("/usr/bin")
    );

    let legacy: ProcessData =
        serde_json::from_str(r#"{"pid":7,"name":"codex","cmdline":["codex"]}"#).unwrap();
    assert_eq!(legacy.started_at, 0);
    assert_eq!(legacy.run_time_seconds, 0);
}

#[test]
fn cron_type_is_constrained_enum() {
    let cron: CronData = serde_json::from_str(
        r#"{"id":"daily","name":"Daily","prompt":"Run","enabled":true,"type":"every"}"#,
    )
    .unwrap();
    assert_eq!(cron.cron_type, Some(CronType::Every));

    let json = serde_json::to_value(&cron).unwrap();
    assert_eq!(json["type"], "every");

    let unknown = r#"{"id":"daily","name":"Daily","prompt":"Run","enabled":true,"type":"later"}"#;
    assert!(serde_json::from_str::<CronData>(unknown).is_err());
}

#[test]
fn mcp_type_is_constrained_enum() {
    let mcp: McpData = serde_json::from_str(r#"{"name":"local","type":"stdio"}"#).unwrap();
    assert_eq!(mcp.mcp_type, Some(McpType::Stdio));

    let json = serde_json::to_value(&mcp).unwrap();
    assert_eq!(json["type"], "stdio");

    let unknown = r#"{"name":"local","type":"websocket"}"#;
    assert!(serde_json::from_str::<McpData>(unknown).is_err());
}

#[test]
fn skill_directory_collection_returns_absolute_home_paths() {
    let cwd = std::env::current_dir().unwrap();
    let dir = tempfile::Builder::new()
        .prefix("sentra-skill-home-")
        .tempdir_in(&cwd)
        .unwrap();
    let skill_dir = dir.path().join("skills").join("demo");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "---\nname: demo\n---\nbody").unwrap();

    let relative = std::path::Path::new(".").join(dir.path().strip_prefix(&cwd).unwrap());
    let skills = collect_skills_from_dir(&relative).unwrap();

    let home = skills[0].home.as_ref().unwrap();
    assert!(home.is_absolute());
    assert!(!home.to_string_lossy().starts_with(r"\\?\"));
    assert!(home.ends_with("skills/demo"));
}

#[test]
fn agent_discovery_finds_codex_home_and_title() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();

    let agents = discover_agents(dir.path());

    assert!(agents.iter().any(|agent| agent.name() == "codex"));
    assert_eq!(
        agents
            .iter()
            .find(|agent| agent.name() == "codex")
            .unwrap()
            .title(),
        "Codex"
    );
}

#[test]
fn codex_and_opencode_meta_report_detected_install_markers() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    let opencode_home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&opencode_home).unwrap();
    fs::write(opencode_home.join("opencode.json"), "{}").unwrap();
    let bin_dir = dir.path().join(".local").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(bin_dir.join(test_binary_name("codex")), "").unwrap();
    fs::write(bin_dir.join(test_binary_name("opencode")), "").unwrap();

    let agents = discover_agents(dir.path());
    for agent_name in ["codex", "opencode"] {
        let agent = agents
            .iter()
            .find(|agent| agent.name() == agent_name)
            .unwrap();
        let meta = asset_data(agent, AssetType::Meta);

        assert_eq!(meta[0].data["installed"], true);
    }
}

#[test]
fn discovery_returns_installed_agent_without_initialized_home() {
    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join(".local").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(bin_dir.join(test_binary_name("codex")), "").unwrap();

    let expected_home = dir.path().join(".codex");
    assert!(!expected_home.exists());

    let agents = discover_agents(dir.path());
    let codex = agents.iter().find(|agent| agent.name() == "codex").unwrap();

    assert_eq!(codex.home(), expected_home.as_path());
    let meta = asset_data(codex, AssetType::Meta);
    assert_eq!(meta[0].data["installed"], true);
}

#[test]
fn discovery_returns_codex_desktop_app_without_cli_or_initialized_home() {
    let dir = tempfile::tempdir().unwrap();
    let app_home = dir.path().join("Applications").join("ChatGPT.app");
    fs::create_dir_all(&app_home).unwrap();

    let expected_home = dir.path().join(".codex");
    assert!(!expected_home.exists());

    let agents = discover_agents(dir.path());
    let codex = agents.iter().find(|agent| agent.name() == "codex").unwrap();

    assert_eq!(codex.home(), expected_home.as_path());
    let meta = asset_data(codex, AssetType::Meta);
    assert_eq!(meta[0].data["installed"], true);
}

#[test]
fn module_discovery_returns_agents_that_own_asset_factories() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();

    let agents: Vec<_> = discover_agents(dir.path())
        .into_iter()
        .filter(|agent| agent.name() == "codex")
        .collect();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name(), "codex");
    assert_eq!(agents[0].title(), "Codex");
    assert_eq!(agents[0].get_assets(AssetType::Mcp).unwrap().len(), 1);
}

#[test]
fn discovered_agents_identify_and_collect_supported_assets() {
    let dir = tempfile::tempdir().unwrap();

    let codex_home = dir.path().join(".codex");
    let codex_skill = codex_home.join("skills").join("codex-skill");
    fs::create_dir_all(&codex_skill).unwrap();
    fs::write(
        codex_skill.join("SKILL.md"),
        "---\nname: codex-skill\n---\nbody",
    )
    .unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"
model = "gpt-5"
model_provider = "openai"

[mcp_servers.local]
command = "node"

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
"#,
    )
    .unwrap();

    let sentra_home = dir.path().join(".sentra");
    let sentra_skill = sentra_home.join("skills").join("sentra-skill");
    fs::create_dir_all(&sentra_skill).unwrap();
    fs::write(
        sentra_skill.join("SKILL.md"),
        "---\nname: sentra-skill\n---\nbody",
    )
    .unwrap();
    fs::write(
        sentra_home.join("config.json"),
        r#"{"llm":{"api":"https://api.example.com/v1","key":"sk-test","model":"gpt-5"}}"#,
    )
    .unwrap();

    let cursor_home = dir.path().join(".cursor");
    let cursor_skill = cursor_home.join("cursor-skill");
    fs::create_dir_all(&cursor_skill).unwrap();
    fs::write(
        cursor_skill.join("SKILL.md"),
        "---\nname: cursor-skill\n---\nbody",
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let codex_agent = agents.iter().find(|agent| agent.name() == "codex").unwrap();
    let sentra_agent = agents
        .iter()
        .find(|agent| agent.name() == "sentra")
        .unwrap();
    let cursor_agent = agents
        .iter()
        .find(|agent| agent.name() == "cursor")
        .unwrap();

    let codex_skills = asset_data(codex_agent, AssetType::Skill);
    assert_eq!(codex_skills[0].asset_type, AssetType::Skill);
    assert_eq!(codex_skills[0].data[0]["name"], "codex-skill");
    let codex_mcp = codex_agent.get_assets(AssetType::Mcp).unwrap();
    assert_eq!(codex_mcp[0].asset_type(), AssetType::Mcp);
    let codex_provider = codex_agent.get_assets(AssetType::Provider).unwrap();
    assert_eq!(codex_provider[0].asset_type(), AssetType::Provider);

    let sentra_skills = asset_data(sentra_agent, AssetType::Skill);
    assert_eq!(sentra_skills[0].data[0]["name"], "sentra-skill");
    assert_eq!(
        sentra_agent.get_assets(AssetType::Provider).unwrap()[0].asset_type(),
        AssetType::Provider
    );

    let cursor_skills = asset_data(cursor_agent, AssetType::Skill);
    assert_eq!(cursor_skills[0].data[0]["name"], "cursor-skill");
    assert!(
        cursor_agent
            .get_assets(AssetType::Provider)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn agent_assets_match_ts_supported_types_and_read_configs() {
    let dir = tempfile::tempdir().unwrap();
    let codex_home = dir.path().join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        codex_home.join("config.toml"),
        r#"
model = "gpt-5"
model_provider = "openai"

[mcp_servers.local]
command = "node"
args = ["server.js"]

[mcp_servers.empty]

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
env_key = "SENTRA_RT_CODEX_TEST_KEY"
"#,
    )
    .unwrap();

    let codex = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "codex")
        .unwrap();

    let mcp = asset_data(&codex, AssetType::Mcp);
    let local_mcp = mcp[0]
        .data
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "local")
        .unwrap();
    assert_eq!(local_mcp["command"], "node");
    let empty_mcp = mcp[0]
        .data
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "empty")
        .unwrap();
    assert_eq!(empty_mcp["type"], "stdio");

    let providers = asset_data(&codex, AssetType::Provider);
    assert_eq!(providers[0].data[0]["baseUrl"], "https://api.openai.com/v1");
    assert!(providers[0].data[0]["apiKey"].is_null());
    assert_eq!(providers[0].data[0]["models"][0]["id"], "gpt-5");
}

#[test]
fn codex_skill_asset_reads_enabled_config_and_plugin_cache() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".codex");
    let skill_dir = home.join("skills").join("local");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "---\nname: local\n---\nbody").unwrap();
    fs::write(
        home.join("config.toml"),
        format!(
            "[[skills.config]]\npath = \"{}\"\nenabled = false\n",
            skill_dir
                .join("SKILL.md")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();

    let plugin_skills = home
        .join("plugins")
        .join("cache")
        .join("vendor")
        .join("stable")
        .join("plugin-root")
        .join("skills")
        .join("remote");
    fs::create_dir_all(&plugin_skills).unwrap();
    fs::write(
        plugin_skills.join("SKILL.md"),
        "---\nname: remote\n---\nbody",
    )
    .unwrap();
    fs::create_dir_all(
        plugin_skills
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".codex-plugin"),
    )
    .unwrap();
    fs::write(
        plugin_skills
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".codex-plugin")
            .join("plugin.json"),
        r#"{"name":"plugin-a","version":"1.2.3","skills":"skills","author":{"name":"Alice"}}"#,
    )
    .unwrap();

    let codex = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "codex")
        .unwrap();
    let skills = asset_data(&codex, AssetType::Skill);
    let skills = skills[0].data.as_array().unwrap();

    let local = skills
        .iter()
        .find(|skill| skill["name"] == "local")
        .unwrap();
    assert_eq!(local["enabled"], false);
    let remote = skills
        .iter()
        .find(|skill| skill["name"] == "remote")
        .unwrap();
    assert_eq!(remote["source"], "plugin-a");
    assert_eq!(remote["version"], "1.2.3");
    assert_eq!(remote["author"], "Alice");
}

#[test]
fn codex_plugin_asset_reads_cache_manifest_without_raw_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".codex");
    let plugin_root = home
        .join("plugins")
        .join("cache")
        .join("vendor")
        .join("stable")
        .join("plugin-root");
    let manifest_dir = plugin_root.join(".codex-plugin");
    fs::create_dir_all(&manifest_dir).unwrap();
    fs::write(
        manifest_dir.join("plugin.json"),
        r#"{
          "id": "codex-demo-id",
          "name": "codex-demo",
          "version": "1.2.3",
          "author": {"name": "Alice"},
          "skills": "skills",
          "apiKey": "sk-should-not-leak",
          "interface": {
            "displayName": "Codex Demo",
            "description": "Demo plugin"
          }
        }"#,
    )
    .unwrap();

    let codex = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "codex")
        .unwrap();
    let plugins = asset_data(&codex, AssetType::Plugin);
    let items = plugins[0].data.as_array().unwrap();

    assert_eq!(items.len(), 1);
    let plugin = &items[0];
    assert_eq!(plugin["name"], "codex-demo");
    assert_eq!(plugin["displayName"], "Codex Demo");
    assert_eq!(plugin["description"], "Demo plugin");
    assert_eq!(plugin["version"], "1.2.3");
    assert_eq!(plugin["author"], "Alice");
    assert_eq!(plugin["enabled"], true);
    assert_eq!(plugin["installSource"]["kind"], "marketplace");
    assert_eq!(
        plugin["installSource"]["reference"],
        "vendor/codex-demo@1.2.3"
    );
    assert_eq!(plugin["installSource"]["marketplace"], "vendor");
    assert!(
        plugin["capabilities"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("skills"))
    );
    assert!(
        !serde_json::to_string(plugin)
            .unwrap()
            .contains("sk-should-not-leak")
    );
}

#[test]
fn claude_cli_plugin_asset_reads_cache_and_skills_dir_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".claude");
    let cache_plugin = home
        .join("plugins")
        .join("cache")
        .join("official")
        .join("claude-cache")
        .join("0.1.0");
    fs::create_dir_all(cache_plugin.join(".claude-plugin")).unwrap();
    fs::write(
        cache_plugin.join(".claude-plugin").join("plugin.json"),
        r#"{
          "name": "claude-cache",
          "version": "0.1.0",
          "author": "Claude Team",
          "interface": {"displayName": "Claude Cache"},
          "slashCommands": ["demo"]
        }"#,
    )
    .unwrap();

    let skills_plugin = home.join("skills").join("local-plugin");
    fs::create_dir_all(skills_plugin.join(".claude-plugin")).unwrap();
    fs::write(
        skills_plugin.join(".claude-plugin").join("plugin.json"),
        r#"{
          "name": "local-plugin",
          "description": "Local Claude plugin",
          "author": {"name": "Local Author"}
        }"#,
    )
    .unwrap();

    let claude = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "claude-cli")
        .unwrap();
    let plugins = asset_data(&claude, AssetType::Plugin);
    let items = plugins[0].data.as_array().unwrap();
    assert_eq!(items.len(), 2);

    let cache = items
        .iter()
        .find(|item| item["name"] == "claude-cache")
        .unwrap();
    assert_eq!(cache["displayName"], "Claude Cache");
    assert_eq!(cache["author"], "Claude Team");
    assert_eq!(cache["installSource"]["kind"], "marketplace");
    assert_eq!(
        cache["installSource"]["reference"],
        "official/claude-cache@0.1.0"
    );
    assert!(
        cache["capabilities"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("slashCommands"))
    );

    let local = items
        .iter()
        .find(|item| item["name"] == "local-plugin")
        .unwrap();
    assert_eq!(local["description"], "Local Claude plugin");
    assert_eq!(local["author"], "Local Author");
    assert_eq!(local["origin"], "skills_dir");
    assert_eq!(local["installSource"]["kind"], "local_path");
    assert!(
        local["installSource"]["reference"]
            .as_str()
            .unwrap()
            .replace('\\', "/")
            .ends_with(".claude/skills/local-plugin")
    );
}

#[test]
fn skill_collection_matches_ts_boundaries_and_memory_keeps_empty_files() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".codex");
    let parent_skill = home.join("skills").join("parent");
    let nested_skill = parent_skill.join("nested");
    fs::create_dir_all(&nested_skill).unwrap();
    fs::write(
        parent_skill.join("SKILL.md"),
        "---\nname: parent\n---\nbody",
    )
    .unwrap();
    fs::write(
        nested_skill.join("SKILL.md"),
        "---\nname: nested\n---\nbody",
    )
    .unwrap();

    let memories_dir = home.join("memories");
    fs::create_dir_all(&memories_dir).unwrap();
    fs::write(memories_dir.join("empty.txt"), "").unwrap();

    let codex = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "codex")
        .unwrap();

    let skills = asset_data(&codex, AssetType::Skill);
    let skills = skills[0].data.as_array().unwrap();
    assert!(skills.iter().any(|skill| skill["name"] == "parent"));
    assert!(!skills.iter().any(|skill| skill["name"] == "nested"));

    let memories = asset_data(&codex, AssetType::Memory);
    assert!(
        memories[0]
            .data
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["name"] == "empty.txt")
    );
}

#[test]
fn malformed_skill_frontmatter_does_not_break_asset_listing() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".codex");
    let skill_dir = home.join("skills").join("bad-frontmatter");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: bad-frontmatter\ndescription: \"bad \"quote\" here\"\n---\nbody",
    )
    .unwrap();

    let codex = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "codex")
        .unwrap();
    let skills = asset_data(&codex, AssetType::Skill);
    let skills = skills[0].data.as_array().unwrap();

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["name"], "bad-frontmatter");
}

#[test]
fn general_exposes_only_ts_supported_skill_asset() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".cursor")).unwrap();

    let agents = discover_agents(dir.path());
    let cursor = agents
        .iter()
        .find(|agent| agent.name() == "cursor")
        .unwrap();
    assert_eq!(cursor.get_assets(AssetType::Skill).unwrap().len(), 1);
    assert!(cursor.get_assets(AssetType::Mcp).unwrap().is_empty());
}

#[test]
fn pi_agent_is_discovered_and_reads_llm_provider_config() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".pi").join("agent");
    let skill_dir = home.join("skills").join("pi-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "---\nname: pi-skill\n---\nbody").unwrap();
    fs::write(
        home.join("settings.json"),
        r#"{"defaultProvider":"svip","defaultModel":"svip/gpt-5.5"}"#,
    )
    .unwrap();
    fs::write(
        home.join("models.json"),
        r#"{"providers":{"svip":{"name":"SVIP Gateway","api":"openai-responses","baseURL":"https://svip.example.com/v1","apiKey":"$SENTRA_PI_TEST_KEY","models":[{"id":"svip/gpt-5.5","name":"SVIP GPT 5.5"}]}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let pi = agents.iter().find(|agent| agent.name() == "pi").unwrap();

    assert_eq!(pi.title(), "Pi");
    assert_eq!(pi.get_assets(AssetType::Skill).unwrap().len(), 1);

    let skills = asset_data(pi, AssetType::Skill);
    assert_eq!(skills[0].data[0]["name"], "pi-skill");

    let providers = asset_data(pi, AssetType::Provider);
    let provider = &providers[0].data[0];
    assert_eq!(provider["name"], "SVIP Gateway");
    assert_eq!(provider["baseUrl"], "https://svip.example.com/v1");
    assert_eq!(provider["apiKey"], "$SENTRA_PI_TEST_KEY");
    assert_eq!(provider["enabled"], true);
    assert!(provider["protocol"].is_null());
    assert_eq!(provider["models"][0]["id"], "svip/gpt-5.5");
}

#[test]
fn pi_provider_reads_auth_without_executing_command_keys() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".pi").join("agent");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("settings.json"),
        r#"{"defaultProvider":"cmd","defaultModel":"cmd-model"}"#,
    )
    .unwrap();
    fs::write(
        home.join("models.json"),
        r#"{"providers":{"cmd":{"api":"openai-completions","baseURL":"https://cmd.example.com/v1","models":["cmd-model"]}}}"#,
    )
    .unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"providers":{"cmd":{"key":"!printf should-not-run"}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let pi = agents.iter().find(|agent| agent.name() == "pi").unwrap();
    let providers = asset_data(pi, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert_eq!(provider["baseUrl"], "https://cmd.example.com/v1");
    assert!(provider["apiKey"].is_null());
    assert!(provider["protocol"].is_null());
}

#[test]
fn pi_provider_uses_builtin_base_url_without_models_config() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".pi").join("agent");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("settings.json"),
        r#"{"defaultProvider":"opencode-go","defaultModel":"deepseek-v4-flash"}"#,
    )
    .unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"opencode-go":{"type":"api_key","key":"sk-opencode"}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let pi = agents.iter().find(|agent| agent.name() == "pi").unwrap();
    let providers = asset_data(pi, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert_eq!(provider["name"], "opencode-go");
    assert_eq!(provider["baseUrl"], "https://opencode.ai/zen/go/v1");
    assert_eq!(provider["apiKey"], "sk-opencode");
    assert_eq!(provider["enabled"], true);
    assert!(provider["protocol"].is_null());
    assert_eq!(provider["models"][0]["id"], "deepseek-v4-flash");
}

#[test]
fn pi_provider_lists_inactive_auth_providers_without_models_config() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".pi").join("agent");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("settings.json"),
        r#"{"defaultProvider":"deepseek","defaultModel":"deepseek-v4-flash"}"#,
    )
    .unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"opencode-go":{"type":"api_key","key":"sk-opencode"},"deepseek":{"type":"api_key","key":"sk-deepseek"},"minimax-cn":{"type":"api_key","key":"sk-minimax-cn"}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let pi = agents.iter().find(|agent| agent.name() == "pi").unwrap();
    let providers = asset_data(pi, AssetType::Provider);
    let provider_items = providers[0].data.as_array().unwrap();

    assert_eq!(provider_items.len(), 3);
    let deepseek = provider_items
        .iter()
        .find(|provider| provider["name"] == "deepseek")
        .unwrap();
    let opencode_go = provider_items
        .iter()
        .find(|provider| provider["name"] == "opencode-go")
        .unwrap();
    let minimax_cn = provider_items
        .iter()
        .find(|provider| provider["name"] == "minimax-cn")
        .unwrap();

    assert_eq!(deepseek["enabled"], true);
    assert_eq!(deepseek["baseUrl"], "https://api.deepseek.com");
    assert_eq!(deepseek["apiKey"], "sk-deepseek");
    assert_eq!(deepseek["models"][0]["id"], "deepseek-v4-flash");
    assert_eq!(opencode_go["enabled"], false);
    assert_eq!(opencode_go["baseUrl"], "https://opencode.ai/zen/go/v1");
    assert_eq!(opencode_go["apiKey"], "sk-opencode");
    assert_eq!(minimax_cn["enabled"], false);
    assert_eq!(
        minimax_cn["baseUrl"],
        "https://api.minimaxi.com/anthropic/v1"
    );
    assert_eq!(minimax_cn["apiKey"], "sk-minimax-cn");
    assert!(minimax_cn["protocol"].is_null());
}

#[test]
fn pi_provider_enables_auth_provider_without_default_provider() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".pi").join("agent");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"deepseek":{"type":"api_key","key":"sk-deepseek"}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let pi = agents.iter().find(|agent| agent.name() == "pi").unwrap();
    let providers = asset_data(pi, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert_eq!(provider["name"], "deepseek");
    assert_eq!(provider["enabled"], true);
    assert_eq!(provider["baseUrl"], "https://api.deepseek.com");
}

#[test]
fn openclaw_provider_reads_configured_entries_without_catalog_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".openclaw");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("openclaw.json"),
        r#"{"providers":{"deepseek":{"apiKey":"sk-deepseek"},"future-provider":{"apiKey":"sk-future"},"corp-gateway":{"baseUrl":"https://llm.example.test/v1"}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let openclaw = agents
        .iter()
        .find(|agent| agent.name() == "openclaw")
        .unwrap();
    let providers = asset_data(openclaw, AssetType::Provider);
    let items = providers[0].data.as_array().unwrap();
    let deepseek = items
        .iter()
        .find(|provider| provider["name"] == "deepseek")
        .unwrap();
    let future = items
        .iter()
        .find(|provider| provider["name"] == "future-provider")
        .unwrap();
    let custom = items
        .iter()
        .find(|provider| provider["name"] == "corp-gateway")
        .unwrap();

    assert!(deepseek["baseUrl"].is_null());
    assert_eq!(deepseek["enabled"], true);
    assert!(deepseek["apiKey"].as_str().unwrap().contains("****"));
    assert!(future["baseUrl"].is_null());
    assert_eq!(custom["baseUrl"], "https://llm.example.test/v1");
}

#[test]
fn openclaw_provider_does_not_infer_from_default_model_without_provider_table() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".openclaw");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("openclaw.json"),
        r#"{
          "agents": {
            "defaults": {
              "model": {
                "primary": "opencode-go/kimi-k2.6",
                "fallbacks": ["minimax-cn/MiniMax-M2.7"]
              },
              "models": {
                "opencode-go/kimi-k2.6": {"alias": "Kimi"},
                "minimax-cn/MiniMax-M2.7": {}
              }
            }
          }
        }"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let openclaw = agents
        .iter()
        .find(|agent| agent.name() == "openclaw")
        .unwrap();
    let providers = asset_data(openclaw, AssetType::Provider);
    let items = providers[0].data.as_array().unwrap();
    assert!(items.is_empty());
}

#[test]
fn openclaw_provider_does_not_infer_opencode_go_endpoint_from_model_name() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".openclaw");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("openclaw.json"),
        r#"{"agents":{"defaults":{"model":"opencode-go/minimax-m2.7"}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let openclaw = agents
        .iter()
        .find(|agent| agent.name() == "openclaw")
        .unwrap();
    let providers = asset_data(openclaw, AssetType::Provider);
    assert!(providers[0].data.as_array().unwrap().is_empty());
}

#[test]
fn opencode_discovery_uses_config_home() {
    let config_only = tempfile::tempdir().unwrap();
    let config_home = config_only.path().join(".config").join("opencode");
    fs::create_dir_all(&config_home).unwrap();
    let agents = discover_agents(config_only.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    assert_eq!(opencode.title(), "OpenCode");
    assert_eq!(opencode.home(), config_home);

    let data_only = tempfile::tempdir().unwrap();
    fs::create_dir_all(
        data_only
            .path()
            .join(".local")
            .join("share")
            .join("opencode"),
    )
    .unwrap();
    let expected_without_config_home = usize::from(opencode_command_exists());
    let agents = discover_agents(data_only.path());
    assert_eq!(
        agents
            .iter()
            .filter(|agent| agent.name() == "opencode")
            .count(),
        expected_without_config_home
    );

    let legacy_only = tempfile::tempdir().unwrap();
    let legacy_home = legacy_only.path().join(".opencode");
    fs::create_dir_all(&legacy_home).unwrap();
    fs::write(legacy_home.join("opencode.json"), "{}").unwrap();
    let agents = discover_agents(legacy_only.path());
    assert_eq!(
        agents
            .iter()
            .filter(|agent| agent.name() == "opencode")
            .count(),
        expected_without_config_home
    );
}

#[test]
fn opencode_provider_reads_chaitin_gateway_and_masks_api_key() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("opencode.json"),
        r#"{
          "$schema": "https://opencode.ai/config.json",
          "model": "chaitin/dev/gpt-5.4",
          "provider": {
            "chaitin": {
              "npm": "@ai-sdk/openai-compatible",
              "name": "Baizhi Gateway",
              "options": {
                "baseURL": "https://ai-api-gateway.app.baizhi.cloud/api/openai",
                "apiKey": "sk-chaitin-secret"
              },
              "models": {
                "dev/gpt-5.4": {
                  "name": "Dev GPT-5.4"
                }
              }
            }
          }
        }"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let providers = asset_data(opencode, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert_eq!(provider["name"], "Baizhi Gateway");
    assert_eq!(
        provider["baseUrl"],
        "https://ai-api-gateway.app.baizhi.cloud/api/openai"
    );
    assert_eq!(provider["models"][0]["id"], "dev/gpt-5.4");
    assert_eq!(provider["models"][0]["name"], "Dev GPT-5.4");
    assert_eq!(provider["enabled"], true);
    assert!(provider["protocol"].is_null());
    assert!(provider["apiKey"].as_str().unwrap().contains("****"));
    assert_ne!(provider["apiKey"], "sk-chaitin-secret");
}

#[test]
fn opencode_provider_reads_legacy_dot_opencode_config() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".config").join("opencode")).unwrap();
    let home = dir.path().join(".opencode");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("opencode.json"),
        r#"{
          "model": "chaitin/dev/gpt-5.4",
          "provider": {
            "chaitin": {
              "npm": "@ai-sdk/openai-compatible",
              "name": "Baizhi Gateway",
              "options": {
                "baseURL": "https://ai-api-gateway.app.baizhi.cloud/api/openai",
                "apiKey": "sk-legacy-secret"
              },
              "models": {"dev/gpt-5.4": {"name": "Dev GPT-5.4"}}
            }
          }
        }"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let providers = asset_data(opencode, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert_eq!(opencode.home(), dir.path().join(".config").join("opencode"));
    assert_eq!(
        provider["baseUrl"],
        "https://ai-api-gateway.app.baizhi.cloud/api/openai"
    );
    assert_eq!(provider["models"][0]["id"], "dev/gpt-5.4");
    assert_eq!(provider["enabled"], true);
    assert!(provider["protocol"].is_null());
    assert!(provider["apiKey"].as_str().unwrap().contains("****"));
    assert_ne!(provider["apiKey"], "sk-legacy-secret");
}

#[test]
fn opencode_provider_prefers_legacy_dot_opencode_model_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_home = dir.path().join(".config").join("opencode");
    let legacy_home = dir.path().join(".opencode");
    fs::create_dir_all(&config_home).unwrap();
    fs::create_dir_all(&legacy_home).unwrap();
    fs::write(
        config_home.join("opencode.json"),
        r#"{"model":"chaitin/config-model","provider":{"chaitin":{"name":"Config Gateway","options":{"baseURL":"https://config.example.test/v1","apiKey":"sk-config-secret"},"models":{"config-model":{"name":"Config Model"}}}}}"#,
    )
    .unwrap();
    fs::write(
        legacy_home.join("opencode.json"),
        r#"{"model":"chaitin/legacy-model","provider":{"chaitin":{"name":"Legacy Gateway","options":{"baseURL":"https://legacy.example.test/v1","apiKey":"sk-legacy-secret"},"models":{"legacy-model":{"name":"Legacy Model"}}}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let providers = asset_data(opencode, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert_eq!(provider["name"], "Legacy Gateway");
    assert_eq!(provider["baseUrl"], "https://legacy.example.test/v1");
    assert_eq!(provider["models"][0]["id"], "legacy-model");
    assert_eq!(provider["models"][0]["name"], "Legacy Model");
    assert_eq!(provider["enabled"], true);
}

#[test]
fn opencode_provider_can_read_masked_api_key_from_auth_json() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    let data_home = dir.path().join(".local").join("share").join("opencode");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&data_home).unwrap();
    fs::write(
        home.join("opencode.json"),
        r#"{"provider":{"chaitin":{"name":"Baizhi Gateway","options":{"baseURL":"https://gateway.example.test/v1"},"models":{"dev/gpt-5.4":{"name":"Dev GPT-5.4"}}}}}"#,
    )
    .unwrap();
    fs::write(
        data_home.join("auth.json"),
        r#"{"providers":{"chaitin":{"apiKey":"sk-auth-secret"}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let providers = asset_data(opencode, AssetType::Provider);
    let provider = &providers[0].data[0];

    assert!(provider["apiKey"].as_str().unwrap().contains("****"));
    assert_ne!(provider["apiKey"], "sk-auth-secret");
    assert_eq!(provider["enabled"], true);
}

#[test]
fn opencode_provider_runtime_data_keeps_api_key_for_model_probe() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("opencode.json"),
        r#"{"model":"chaitin/dev/gpt-5.4","provider":{"chaitin":{"npm":"@ai-sdk/openai-compatible","name":"Baizhi Gateway","options":{"baseURL":"https://gateway.example.test/v1","apiKey":"sk-chaitin-secret"},"models":{"dev/gpt-5.4":{"name":"Dev GPT-5.4"}}}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let provider_asset = opencode
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let masked: Vec<ProviderData> =
        serde_json::from_str(&provider_asset.data().unwrap().to_string()).unwrap();
    let runtime: Vec<ProviderData> =
        serde_json::from_str(&provider_asset.runtime_data().unwrap().to_string()).unwrap();

    assert_ne!(masked[0].api_key.as_deref(), Some("sk-chaitin-secret"));
    assert_eq!(runtime[0].api_key.as_deref(), Some("sk-chaitin-secret"));
}

#[test]
fn opencode_provider_set_data_writes_provider_config() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let provider_asset = opencode
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let result = provider_asset
        .set_provider_data(ProviderData {
            name: "Baizhi Gateway".to_string(),
            raw_provider_id: Some("chaitin".to_string()),
            base_url: Some("https://ai-api-gateway.app.baizhi.cloud/api/openai".to_string()),
            api_key: Some("sk-chaitin-secret".to_string()),
            enabled: true,
            models: vec![ProviderModel {
                id: "dev/gpt-5.4".to_string(),
                name: Some("Dev GPT-5.4".to_string()),
                enabled: true,
            }],
            protocol: Some(WireProtocol::ChatCompletions),
            ..ProviderData::default()
        })
        .unwrap();

    assert!(result.changed);
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join("opencode.json")).unwrap()).unwrap();
    assert_eq!(config["model"], "chaitin/dev/gpt-5.4");
    assert_eq!(config["provider"]["chaitin"]["name"], "Baizhi Gateway");
    assert_eq!(
        config["provider"]["chaitin"]["npm"],
        "@ai-sdk/openai-compatible"
    );
    assert_eq!(
        config["provider"]["chaitin"]["api"],
        "openai-chat-completions"
    );
    assert_eq!(
        config["provider"]["chaitin"]["options"]["baseURL"],
        "https://ai-api-gateway.app.baizhi.cloud/api/openai"
    );
    assert_eq!(
        config["provider"]["chaitin"]["options"]["apiKey"],
        "sk-chaitin-secret"
    );
    assert_eq!(
        config["provider"]["chaitin"]["models"]["dev/gpt-5.4"]["name"],
        "Dev GPT-5.4"
    );

    let providers = asset_data(opencode, AssetType::Provider);
    let provider = &providers[0].data[0];
    assert_eq!(provider["name"], "Baizhi Gateway");
    assert_eq!(provider["enabled"], true);
    assert!(provider["apiKey"].as_str().unwrap().contains("****"));
    assert_ne!(provider["apiKey"], "sk-chaitin-secret");
}

#[test]
fn opencode_provider_set_data_writes_openai_npm_package() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let provider_asset = opencode
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    provider_asset
        .set_provider_data(ProviderData {
            name: "OpenAI".to_string(),
            raw_provider_id: Some("openai".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            api_key: Some("sk-openai-secret".to_string()),
            enabled: true,
            models: vec![ProviderModel {
                id: "gpt-5".to_string(),
                name: Some("GPT-5".to_string()),
                enabled: true,
            }],
            protocol: Some(WireProtocol::Responses),
            ..ProviderData::default()
        })
        .unwrap();

    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join("opencode.json")).unwrap()).unwrap();
    assert_eq!(config["model"], "openai/gpt-5");
    assert_eq!(config["provider"]["openai"]["npm"], "@ai-sdk/openai");
    assert_eq!(config["provider"]["openai"]["api"], "openai-responses");
}

#[test]
fn opencode_provider_set_data_writes_anthropic_npm_package() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let provider_asset = opencode
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    provider_asset
        .set_provider_data(ProviderData {
            name: "Anthropic Gateway".to_string(),
            raw_provider_id: Some("anthropic".to_string()),
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some("sk-ant-secret".to_string()),
            enabled: true,
            models: vec![ProviderModel {
                id: "claude-sonnet-4".to_string(),
                name: Some("Claude Sonnet 4".to_string()),
                enabled: true,
            }],
            protocol: Some(WireProtocol::AnthropicMessages),
            ..ProviderData::default()
        })
        .unwrap();

    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join("opencode.json")).unwrap()).unwrap();
    assert_eq!(config["model"], "anthropic/claude-sonnet-4");
    assert_eq!(config["provider"]["anthropic"]["npm"], "@ai-sdk/anthropic");
    assert_eq!(config["provider"]["anthropic"]["api"], "anthropic");
}

#[test]
fn opencode_provider_set_data_updates_existing_legacy_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_home = dir.path().join(".config").join("opencode");
    let legacy_home = dir.path().join(".opencode");
    fs::create_dir_all(&config_home).unwrap();
    fs::create_dir_all(&legacy_home).unwrap();
    fs::write(
        legacy_home.join("opencode.json"),
        r#"{
          "$schema": "https://opencode.ai/config.json",
          "model": "chaitin/dev/gpt-5.4",
          "plugin": ["superpowers@git+https://github.com/obra/superpowers.git"],
          "provider": {
            "chaitin3": {
              "npm": "@ai-sdk/anthropic",
              "name": "Baizhi Gateway Anthropic",
              "options": {
                "baseURL": "https://ai-api-gateway.app.baizhi.cloud/api/anthropic",
                "apiKey": "sk-old"
              },
              "models": {
                "dev/gpt-5.4": { "name": "Dev GPT-5.4" }
              }
            }
          }
        }"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let provider_asset = opencode
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    provider_asset
        .set_provider_data(ProviderData {
            name: "Baizhi Gateway Anthropic".to_string(),
            raw_provider_id: Some("chaitin3".to_string()),
            base_url: Some("https://ai-api-gateway.app.baizhi.cloud/api/anthropic".to_string()),
            api_key: Some("sk-new".to_string()),
            enabled: true,
            models: vec![ProviderModel {
                id: "dev/gpt-5.5".to_string(),
                name: Some("Dev GPT-5.5".to_string()),
                enabled: true,
            }],
            protocol: Some(WireProtocol::AnthropicMessages),
            ..ProviderData::default()
        })
        .unwrap();

    assert!(!config_home.join("opencode.json").exists());
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(legacy_home.join("opencode.json")).unwrap())
            .unwrap();
    assert_eq!(config["model"], "chaitin3/dev/gpt-5.5");
    assert_eq!(
        config["plugin"][0],
        "superpowers@git+https://github.com/obra/superpowers.git"
    );
    assert_eq!(
        config["provider"]["chaitin3"]["name"],
        "Baizhi Gateway Anthropic"
    );
    assert_eq!(config["provider"]["chaitin3"]["api"], "anthropic");
    assert_eq!(
        config["provider"]["chaitin3"]["options"]["apiKey"],
        "sk-new"
    );
    assert_eq!(
        config["provider"]["chaitin3"]["models"]["dev/gpt-5.5"]["name"],
        "Dev GPT-5.5"
    );
}

#[test]
fn opencode_plugin_asset_reads_config_and_local_plugin_files() {
    let dir = tempfile::tempdir().unwrap();
    let config_home = dir.path().join(".config").join("opencode");
    let legacy_home = dir.path().join(".opencode");
    fs::create_dir_all(config_home.join("plugins")).unwrap();
    fs::create_dir_all(&legacy_home).unwrap();
    fs::write(
        config_home.join("plugins").join("local.ts"),
        "export default {}",
    )
    .unwrap();
    let config = r#"{
      "plugin": [
        "superpowers@git+https://github.com/obra/superpowers.git",
        "@scope/pkg@1.2.3"
      ],
      "provider": {
        "secret": {
          "options": {"apiKey": "sk-opencode-plugin-test-secret"}
        }
      }
    }"#;
    fs::write(config_home.join("opencode.json"), config).unwrap();
    fs::write(
        legacy_home.join("opencode.json"),
        r#"{"plugin":["superpowers@git+https://github.com/obra/superpowers.git"]}"#,
    )
    .unwrap();

    let opencode = discover_agents(dir.path())
        .into_iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let plugins = asset_data(&opencode, AssetType::Plugin);
    let items = plugins[0].data.as_array().unwrap();
    assert_eq!(items.len(), 3);

    let git = items
        .iter()
        .find(|item| item["name"] == "superpowers")
        .unwrap();
    assert_eq!(git["origin"], "config");
    assert_eq!(git["installSource"]["kind"], "git");
    assert_eq!(
        git["installSource"]["reference"],
        "superpowers@git+https://github.com/obra/superpowers.git"
    );

    let npm = items
        .iter()
        .find(|item| item["name"] == "@scope/pkg")
        .unwrap();
    assert_eq!(npm["installSource"]["kind"], "npm");
    assert_eq!(npm["installSource"]["reference"], "@scope/pkg@1.2.3");

    let local = items.iter().find(|item| item["name"] == "local").unwrap();
    assert_eq!(local["origin"], "local_path");
    assert_eq!(local["installSource"]["kind"], "local_path");
    assert!(
        local["home"]
            .as_str()
            .unwrap()
            .replace('\\', "/")
            .ends_with(".config/opencode/plugins/local.ts")
    );
    assert!(
        !serde_json::to_string(items)
            .unwrap()
            .contains("sk-opencode-plugin-test-secret")
    );
}

#[test]
fn opencode_provider_probe_uses_agent_title_request_body() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let provider = opencode
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let requests = provider.provider_requests("dev/gpt-5.4");

    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .any(|request| request.protocol == WireProtocol::Responses)
    );
    assert!(
        requests
            .iter()
            .any(|request| request.protocol == WireProtocol::AnthropicMessages)
    );
    let chat_request = requests
        .iter()
        .find(|request| request.protocol == WireProtocol::ChatCompletions)
        .unwrap();
    assert!(requests.iter().all(|request| request.prompt.is_none()));
    let body: serde_json::Value =
        serde_json::from_str(chat_request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["model"], "dev/gpt-5.4");
    assert_eq!(body["max_tokens"], 32000);
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("You are a title generator")
    );
    assert_eq!(
        body["messages"][1]["content"],
        "Generate a title for this conversation:\n"
    );
    assert_eq!(body["messages"][2]["content"], "hello");
}

#[test]
fn opencode_mcp_maps_local_and_remote_servers() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("opencode.json"),
        r#"{
          "mcp": {
            "local": {
              "type": "local",
              "command": ["node", "server.js"],
              "environment": {"TOKEN": "test"}
            },
            "remote": {
              "type": "remote",
              "url": "https://mcp.example.test/mcp",
              "enabled": false
            }
          }
        }"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let mcps = asset_data(opencode, AssetType::Mcp);
    let items = mcps[0].data.as_array().unwrap();
    let local = items.iter().find(|item| item["name"] == "local").unwrap();
    let remote = items.iter().find(|item| item["name"] == "remote").unwrap();

    assert_eq!(local["type"], "stdio");
    assert_eq!(local["command"], "node");
    assert_eq!(local["args"][0], "server.js");
    assert_eq!(local["env"]["TOKEN"], "test");
    assert_eq!(remote["type"], "http");
    assert_eq!(remote["url"], "https://mcp.example.test/mcp");
    assert_eq!(remote["enabled"], false);
    assert!(opencode.get_assets(AssetType::Cron).unwrap().is_empty());
}

#[test]
fn opencode_skill_reads_single_file_and_directory_skills() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    fs::create_dir_all(home.join("skill")).unwrap();
    fs::create_dir_all(home.join("skills").join("demo")).unwrap();
    fs::write(
        home.join("skill").join("review.md"),
        "---\nname: review\ndescription: Review code\n---\nbody",
    )
    .unwrap();
    fs::write(
        home.join("skills").join("demo").join("SKILL.md"),
        "---\nname: demo\ndescription: Demo skill\n---\nbody",
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    let skills = asset_data(opencode, AssetType::Skill);
    let items = skills[0].data.as_array().unwrap();
    let review = items.iter().find(|item| item["name"] == "review").unwrap();
    let demo = items.iter().find(|item| item["name"] == "demo").unwrap();

    assert!(review["home"].as_str().unwrap().ends_with("review.md"));
    assert_eq!(review["files"][0]["path"], "review.md");
    assert_eq!(review["tags"][0], "opencode");
    assert!(
        demo["home"]
            .as_str()
            .unwrap()
            .replace('\\', "/")
            .ends_with("skills/demo")
    );
    assert_eq!(demo["description"], "Demo skill");
}

#[test]
fn opencode_does_not_expose_memory_assets() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".config").join("opencode");
    let data_home = dir.path().join(".local").join("share").join("opencode");
    fs::create_dir_all(home.join("agent")).unwrap();
    fs::create_dir_all(home.join("command")).unwrap();
    fs::create_dir_all(home.join("plugin")).unwrap();
    fs::create_dir_all(home.join("rule")).unwrap();
    fs::create_dir_all(data_home.join("log")).unwrap();
    fs::create_dir_all(data_home.join("tool-output")).unwrap();
    fs::create_dir_all(data_home.join("snapshot").join("repo")).unwrap();
    fs::create_dir_all(data_home.join("repos").join("project")).unwrap();
    fs::write(home.join("opencode.json"), r#"{"model":"chaitin/dev"}"#).unwrap();
    fs::write(home.join("agent").join("review.md"), "agent prompt").unwrap();
    fs::write(home.join("command").join("build.md"), "command prompt").unwrap();
    fs::write(home.join("plugin").join("plugin.json"), "{}").unwrap();
    fs::write(home.join("rule").join("rules.json"), "{}").unwrap();
    fs::write(
        data_home.join("auth.json"),
        r#"{"providers":{"chaitin":{"apiKey":"sk-memory-secret"}}}"#,
    )
    .unwrap();
    fs::write(data_home.join("opencode.db"), "sqlite").unwrap();
    fs::write(data_home.join("opencode.db-wal"), "wal").unwrap();
    fs::write(data_home.join("opencode.db-shm"), "shm").unwrap();
    fs::write(data_home.join("log").join("opencode.log"), "log").unwrap();
    fs::write(data_home.join("tool-output").join("tool-1"), "output").unwrap();
    fs::write(
        data_home.join("snapshot").join("repo").join("config"),
        "snapshot",
    )
    .unwrap();
    fs::write(
        data_home.join("repos").join("project").join("state"),
        "repo",
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let opencode = agents
        .iter()
        .find(|agent| agent.name() == "opencode")
        .unwrap();
    assert!(opencode.get_assets(AssetType::Memory).unwrap().is_empty());
}

#[test]
fn kimi_code_discovers_default_home() {
    let dir = tempfile::tempdir().unwrap();
    let default_home = dir.path().join(".kimi-code");
    fs::create_dir_all(&default_home).unwrap();

    let agents = discover_agents(dir.path());

    let kimi_homes = agents
        .iter()
        .filter(|agent| agent.name() == "kimi-code")
        .map(|agent| agent.home().to_path_buf())
        .collect::<Vec<_>>();

    assert_eq!(kimi_homes, vec![default_home.clone()]);
    let kimi = agents
        .iter()
        .find(|agent| agent.name() == "kimi-code" && agent.home() == default_home)
        .unwrap();
    assert_eq!(kimi.title(), "Kimi Code");
    assert!(kimi.get_assets(AssetType::Cron).unwrap().is_empty());
    assert_eq!(kimi.get_assets(AssetType::Process).unwrap().len(), 1);
}

#[test]
fn kimi_code_provider_parses_config_and_masks_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".kimi-code");
    fs::create_dir_all(home.join("credentials")).unwrap();
    fs::write(
        home.join("config.toml"),
        r#"
default_model = "kimi-code/kimi-for-coding"

[providers."managed:kimi-code"]
type = "kimi"
base_url = "https://api.kimi.com/coding/v1"
api_key = "sk-kimi-secret"

[models."kimi-code/kimi-for-coding"]
provider = "managed:kimi-code"
model = "kimi-k2-0711-preview"
"#,
    )
    .unwrap();
    fs::write(
        home.join("credentials").join("oauth.json"),
        r#"{"access_token":"oauth-kimi-secret"}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let kimi = agents
        .iter()
        .find(|agent| agent.name() == "kimi-code")
        .unwrap();
    let providers = asset_data(kimi, AssetType::Provider);
    let provider = &providers[0].data[0];
    let serialized = providers[0].data.to_string();

    assert_eq!(provider["baseUrl"], "https://api.kimi.com/coding/v1");
    assert_eq!(provider["enabled"], true);
    assert!(provider["protocol"].is_null());
    assert_eq!(provider["models"][0]["id"], "kimi-k2-0711-preview");
    assert_eq!(provider["models"][0]["name"], "kimi-code/kimi-for-coding");
    assert!(provider["apiKey"].as_str().unwrap().contains("****"));
    assert!(!serialized.contains("sk-kimi-secret"));
    assert!(!serialized.contains("oauth-kimi-secret"));
}

#[test]
fn kimi_code_provider_set_data_writes_config_toml() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".kimi-code");
    fs::create_dir_all(&home).unwrap();

    let agents = discover_agents(dir.path());
    let kimi = agents
        .iter()
        .find(|agent| agent.name() == "kimi-code")
        .unwrap();
    let provider_asset = kimi
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let result = provider_asset
        .set_provider_data(ProviderData {
            name: "Kimi Code".to_string(),
            raw_provider_id: Some("managed:kimi-code".to_string()),
            base_url: Some("https://api.kimi.com/coding/v1".to_string()),
            api_key: Some("sk-kimi-set-secret".to_string()),
            enabled: true,
            models: vec![ProviderModel {
                id: "kimi-k2-0711-preview".to_string(),
                name: Some("Kimi K2".to_string()),
                enabled: true,
            }],
            protocol: Some(WireProtocol::Responses),
            ..ProviderData::default()
        })
        .unwrap();

    assert!(result.changed);
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(home.join("config.toml")).unwrap()).unwrap();
    assert_eq!(
        config["default_model"].as_str(),
        Some("kimi-code/kimi-k2-0711-preview")
    );
    assert_eq!(
        config["providers"]["managed:kimi-code"]["type"].as_str(),
        Some("openai_responses")
    );
    assert_eq!(
        config["providers"]["managed:kimi-code"]["base_url"].as_str(),
        Some("https://api.kimi.com/coding/v1")
    );
    assert_eq!(
        config["providers"]["managed:kimi-code"]["api_key"].as_str(),
        Some("sk-kimi-set-secret")
    );
    assert_eq!(
        config["models"]["kimi-code/kimi-k2-0711-preview"]["provider"].as_str(),
        Some("managed:kimi-code")
    );
    assert_eq!(
        config["models"]["kimi-code/kimi-k2-0711-preview"]["model"].as_str(),
        Some("kimi-k2-0711-preview")
    );
}

#[test]
fn kimi_code_provider_delete_removes_provider_config() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".kimi-code");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.toml"),
        r#"
default_model = "kimi-code/kimi-old"

[providers."managed:kimi-code"]
type = "kimi"
base_url = "https://api.kimi.com/coding/v1"
api_key = "sk-old"

[providers.other]
type = "anthropic"
base_url = "https://anthropic.example.test"
api_key = "sk-other"

[models."kimi-code/kimi-old"]
provider = "managed:kimi-code"
model = "kimi-old"

[models."kimi-code/claude"]
provider = "other"
model = "claude"
"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let kimi = agents
        .iter()
        .find(|agent| agent.name() == "kimi-code")
        .unwrap();
    let provider_asset = kimi
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let result = provider_asset
        .del_provider_data(&ProviderData {
            name: "Kimi Code".to_string(),
            base_url: Some("https://api.kimi.com/coding/v1".to_string()),
            ..ProviderData::default()
        })
        .unwrap();

    assert!(result.changed);
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(home.join("config.toml")).unwrap()).unwrap();
    assert!(config.get("default_model").is_none());
    assert!(config["providers"].get("managed:kimi-code").is_none());
    assert!(config["models"].get("kimi-code/kimi-old").is_none());
    assert_eq!(
        config["providers"]["other"]["type"].as_str(),
        Some("anthropic")
    );
    assert_eq!(
        config["models"]["kimi-code/claude"]["provider"].as_str(),
        Some("other")
    );
}

#[test]
fn kimi_code_mcp_maps_http_sse_stdio_and_plugin_servers() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".kimi-code");
    let plugin_root = home.join("plugins").join("managed").join("demo");
    fs::create_dir_all(&plugin_root).unwrap();
    fs::write(
        home.join("mcp.json"),
        r#"{
          "mcpServers": {
            "http-server": {"url": "https://mcp.example.test/mcp"},
            "sse-server": {"url": "https://mcp.example.test/sse", "transport": "sse"},
            "local-server": {"command": "node", "args": ["server.js"], "env": {"TOKEN": "test"}}
          }
        }"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join("kimi.plugin.json"),
        r#"{
          "name": "demo-plugin",
          "mcpServers": {
            "plugin-server": {"command": ["python", "server.py"]}
          }
        }"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let kimi = agents
        .iter()
        .find(|agent| agent.name() == "kimi-code")
        .unwrap();
    let mcps = asset_data(kimi, AssetType::Mcp);
    let items = mcps[0].data.as_array().unwrap();
    let http = items
        .iter()
        .find(|item| item["name"] == "http-server")
        .unwrap();
    let sse = items
        .iter()
        .find(|item| item["name"] == "sse-server")
        .unwrap();
    let local = items
        .iter()
        .find(|item| item["name"] == "local-server")
        .unwrap();
    let plugin = items
        .iter()
        .find(|item| item["name"] == "plugin-server")
        .unwrap();

    assert_eq!(http["type"], "http");
    assert_eq!(sse["type"], "sse");
    assert_eq!(local["type"], "stdio");
    assert_eq!(local["command"], "node");
    assert_eq!(local["args"][0], "server.js");
    assert_eq!(local["env"]["TOKEN"], "test");
    assert_eq!(plugin["type"], "stdio");
    assert_eq!(plugin["command"], "python");
    assert_eq!(plugin["args"][0], "server.py");
}

#[test]
fn kimi_code_collects_skills_plugins_and_memory_without_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".kimi-code");
    let local_skill = home.join("skills").join("local");
    let global_skill = dir.path().join(".agents").join("skills").join("global");
    let plugin_root = home
        .join("plugins")
        .join("managed")
        .join("vendor")
        .join("plugin");
    let plugin_skill = plugin_root.join("skills").join("plugin-skill");
    fs::create_dir_all(&local_skill).unwrap();
    fs::create_dir_all(&global_skill).unwrap();
    fs::create_dir_all(&plugin_skill).unwrap();
    fs::create_dir_all(home.join("credentials")).unwrap();
    fs::create_dir_all(home.join("bin")).unwrap();
    fs::create_dir_all(home.join("updates")).unwrap();
    fs::create_dir_all(home.join("logs")).unwrap();
    fs::create_dir_all(home.join("sessions").join("2026")).unwrap();
    fs::write(local_skill.join("SKILL.md"), "---\nname: local\n---\nbody").unwrap();
    fs::write(
        global_skill.join("SKILL.md"),
        "---\nname: global\n---\nbody",
    )
    .unwrap();
    fs::write(
        plugin_skill.join("SKILL.md"),
        "---\nname: plugin-skill\n---\nbody",
    )
    .unwrap();
    fs::write(
        plugin_root.join("kimi.plugin.json"),
        r#"{
          "name": "demo-plugin",
          "version": "1.2.3",
          "author": {"name": "Kimi Team"},
          "apiKey": "sk-plugin-secret",
          "interface": {"displayName": "Demo Plugin", "shortDescription": "Demo"},
          "skills": "skills",
          "commands": []
        }"#,
    )
    .unwrap();
    fs::write(home.join("config.toml"), "default_model = \"kimi\"\n").unwrap();
    fs::write(home.join("tui.toml"), "theme = \"dark\"\n").unwrap();
    fs::write(home.join("AGENTS.md"), "User instructions").unwrap();
    fs::write(home.join("mcp.json"), r#"{"mcpServers":{}}"#).unwrap();
    fs::create_dir_all(home.join("plugins")).unwrap();
    fs::write(home.join("plugins").join("installed.json"), "{}").unwrap();
    fs::write(home.join("session_index.jsonl"), "{}\n").unwrap();
    fs::write(home.join("logs").join("kimi-code.log"), "log").unwrap();
    fs::write(
        home.join("sessions").join("2026").join("thread.jsonl"),
        "{}\n",
    )
    .unwrap();
    fs::write(
        home.join("credentials").join("oauth.json"),
        r#"{"access_token":"oauth-kimi-secret"}"#,
    )
    .unwrap();
    fs::write(home.join("bin").join("tool.json"), "{}").unwrap();
    fs::write(home.join("updates").join("update.json"), "{}").unwrap();

    let agents = discover_agents(dir.path());
    let kimi = agents
        .iter()
        .find(|agent| agent.name() == "kimi-code")
        .unwrap();
    let skills = asset_data(kimi, AssetType::Skill);
    let skill_items = skills[0].data.as_array().unwrap();
    assert!(skill_items.iter().any(|item| item["name"] == "local"));
    assert!(skill_items.iter().any(|item| item["name"] == "global"));
    let plugin_skill = skill_items
        .iter()
        .find(|item| item["name"] == "plugin-skill")
        .unwrap();
    assert_eq!(plugin_skill["source"], "demo-plugin");
    assert_eq!(plugin_skill["author"], "Kimi Team");
    assert_eq!(plugin_skill["version"], "1.2.3");

    let plugins = asset_data(kimi, AssetType::Plugin);
    let plugin = &plugins[0].data[0];
    let plugin_json = plugins[0].data.to_string();
    assert_eq!(plugin["name"], "demo-plugin");
    assert_eq!(plugin["displayName"], "Demo Plugin");
    assert_eq!(plugin["enabled"], true);
    assert_eq!(plugin["installSource"]["kind"], "local_path");
    assert!(
        plugin["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "skills")
    );
    assert!(!plugin_json.contains("sk-plugin-secret"));

    let memories = asset_data(kimi, AssetType::Memory);
    let memory_json = memories[0].data.to_string();
    let memory_names = memories[0]
        .data
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(memory_names.contains(&"config.toml".to_string()));
    assert!(memory_names.contains(&"kimi.plugin.json".to_string()));
    assert!(memory_names.contains(&"thread.jsonl".to_string()));
    assert!(!memory_names.contains(&"oauth.json".to_string()));
    assert!(!memory_json.contains("credentials"));
    assert!(!memory_json.contains("oauth-kimi-secret"));
}

#[test]
fn hermes_provider_reads_modern_model_and_auth_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".hermes");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.yaml"),
        "model:\n  provider: minimax-cn\n  default: MiniMax-M2.7\n",
    )
    .unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"active_provider":"minimax-cn","providers":{"minimax-cn":{"access_token":"secret-minimax-key"}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let hermes = agents
        .iter()
        .find(|agent| agent.name() == "hermes")
        .unwrap();
    let providers = asset_data(hermes, AssetType::Provider);
    let items = providers[0].data.as_array().unwrap();
    let provider = items
        .iter()
        .find(|provider| provider["name"] == "minimax-cn")
        .unwrap();

    assert!(provider["baseUrl"].is_null());
    assert!(provider["protocol"].is_null());
    assert_eq!(provider["enabled"], true);
    assert_eq!(provider["models"][0]["id"], "MiniMax-M2.7");
    assert!(provider["apiKey"].is_null());
}

#[test]
fn hermes_provider_can_be_discovered_from_auth_without_config_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".hermes");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("auth.json"),
        r#"{"active_provider":"deepseek","credential_pool":{"deepseek":{"api_key":"secret-deepseek-key"},"opencode-go":{"api_key":"secret-opencode-key"}}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let hermes = agents
        .iter()
        .find(|agent| agent.name() == "hermes")
        .unwrap();
    let providers = asset_data(hermes, AssetType::Provider);
    let items = providers[0].data.as_array().unwrap();
    assert!(items.is_empty());
}

#[test]
fn codex_provider_without_base_url_is_not_enriched_from_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".codex");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.toml"),
        r#"model = "deepseek-chat"
model_provider = "deepseek"

[model_providers.deepseek]
name = "DeepSeek"
"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let codex = agents.iter().find(|agent| agent.name() == "codex").unwrap();
    let providers = asset_data(codex, AssetType::Provider);
    assert!(providers[0].data.as_array().unwrap().is_empty());
}

#[test]
fn pi_provider_probe_declares_supported_protocols() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".pi").join("agent");
    fs::create_dir_all(&home).unwrap();

    let agents = discover_agents(dir.path());
    let pi = agents.iter().find(|agent| agent.name() == "pi").unwrap();
    let provider = pi
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let requests = provider.provider_requests("svip/gpt-5.5");

    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .any(|request| request.protocol == WireProtocol::Responses)
    );
    assert!(
        requests
            .iter()
            .any(|request| request.protocol == WireProtocol::ChatCompletions)
    );
    assert!(
        requests
            .iter()
            .any(|request| request.protocol == WireProtocol::AnthropicMessages)
    );
    assert!(requests.iter().all(|request| request.body.is_none()));
    assert!(requests.iter().all(|request| request.prompt.is_some()));
}

#[test]
fn sentra_agent_is_discovered_and_exposes_skill_and_provider_assets() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".sentra");
    fs::create_dir_all(&home).unwrap();
    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(bin_dir.join(test_binary_name("sentra")), "").unwrap();
    fs::write(
        home.join("config.json"),
        r#"{"llm":{"api":"https://api.example.com/v1","key":"sk-test","model":"gpt-5","protocol":"responses"}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let sentra_agent = agents
        .iter()
        .find(|agent| agent.name() == "sentra")
        .unwrap();

    let meta = asset_data(sentra_agent, AssetType::Meta);
    assert_eq!(meta[0].data["installed"], true);
    assert_eq!(sentra_agent.get_assets(AssetType::Skill).unwrap().len(), 1);
    let providers = asset_data(sentra_agent, AssetType::Provider);

    assert_eq!(
        providers[0].data[0]["baseUrl"],
        "https://api.example.com/v1"
    );
    assert_eq!(providers[0].data[0]["models"][0]["id"], "gpt-5");
}

#[test]
fn devin_general_agent_reports_detected_install_marker() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".devin");
    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(bin_dir.join(test_binary_name("devin")), "").unwrap();

    let agents = discover_agents(dir.path());
    let devin_agent = agents.iter().find(|agent| agent.name() == "devin").unwrap();
    let meta = asset_data(devin_agent, AssetType::Meta);

    assert_eq!(meta[0].data["installed"], true);
}

#[test]
fn sentra_provider_probe_declares_protocols_without_http_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".sentra");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.json"),
        r#"{"llm":{"api":"https://api.example.com/v1","key":"sk-test","model":"svip/gpt-5.5","protocol":"responses"}}"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let sentra_agent = agents
        .iter()
        .find(|agent| agent.name() == "sentra")
        .unwrap();
    let provider = sentra_agent
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    let requests = provider.provider_requests("svip/gpt-5.5");
    let first = &requests[0];

    assert_eq!(first.protocol, WireProtocol::Responses);
    assert!(first.body.is_none());
    let prompt = first.prompt.as_ref().unwrap();
    assert!(prompt.system.contains("valid JSON"));
    assert!(prompt.user.contains("demo://sentra-probe"));
    assert!(prompt.user.contains(r#"{"results":[]}"#));

    let anthropic = requests
        .iter()
        .find(|request| request.protocol == WireProtocol::AnthropicMessages)
        .unwrap();
    assert!(anthropic.body.is_none());
    assert!(anthropic.prompt.is_some());
}

#[test]
fn codex_provider_probe_uses_openai_responses_message_body() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".codex");
    fs::create_dir_all(&home).unwrap();
    fs::write(
        home.join("config.toml"),
        r#"
model = "gpt-5"
model_provider = "openai"

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
experimental_bearer_token = "sk-test"
"#,
    )
    .unwrap();

    let agents = discover_agents(dir.path());
    let codex_agent = agents.iter().find(|agent| agent.name() == "codex").unwrap();
    let provider = codex_agent
        .get_assets(AssetType::Provider)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    let requests = provider.provider_requests("gpt-5");
    let first = &requests[0];

    assert_eq!(first.protocol, WireProtocol::Responses);
    assert!(first.prompt.is_none());
    let body: serde_json::Value = serde_json::from_str(first.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["model"], "gpt-5");
    assert!(
        body["instructions"]
            .as_str()
            .unwrap()
            .contains("valid JSON")
    );
    assert_eq!(body["input"][0]["role"], "developer");
    assert_eq!(body["input"][0]["type"], "message");
    assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
    assert!(
        body["input"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("valid JSON")
    );
    assert_eq!(body["input"][1]["role"], "user");
    assert_eq!(body["input"][1]["type"], "message");
    assert_eq!(body["input"][1]["content"][0]["type"], "input_text");
    assert!(
        body["input"][1]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(r#"{"results":[]}"#)
    );
}

#[test]
fn migrated_builtin_agents_discover_and_parse_representative_assets() {
    let dir = tempfile::tempdir().unwrap();

    let claude_cli_home = dir.path().join(".claude");
    fs::create_dir_all(&claude_cli_home).unwrap();
    fs::write(
        claude_cli_home.join(".claude.json"),
        r#"{"mcpServers":{"global":{"command":"node"}},"projects":{"/work":{"mcpServers":{"project":{"url":"https://mcp.example.com/sse"}}}}}"#,
    )
    .unwrap();
    fs::write(
        claude_cli_home.join("settings.json"),
        r#"{"env":{"ANTHROPIC_BASE_URL":"https://anthropic.example.com","ANTHROPIC_AUTH_TOKEN":"sk-cli","ANTHROPIC_MODEL":"claude-sonnet-4"}}"#,
    )
    .unwrap();
    fs::write(
        claude_cli_home.join("scheduled_tasks.json"),
        r#"{"tasks":[{"id":"daily","cron":"0 9 * * *","prompt":"ship","recurring":true}]}"#,
    )
    .unwrap();

    let claude_app_home = dir.path().join("AppData").join("Local").join("Claude");
    fs::create_dir_all(claude_app_home.join("configLibrary")).unwrap();
    fs::write(
        claude_app_home.join("claude_desktop_config.json"),
        r#"{"mcpServers":{"desktop":{"command":"uvx","args":["tool"]}}}"#,
    )
    .unwrap();
    fs::write(
        claude_app_home.join("configLibrary").join("_meta.json"),
        r#"{"appliedId":"active"}"#,
    )
    .unwrap();
    fs::write(
        claude_app_home.join("configLibrary").join("active.json"),
        r#"{"inferenceGatewayBaseUrl":"https://gateway.example.com","inferenceGatewayApiKey":"sk-app","inferenceModels":[{"name":"claude-opus-4","labelOverride":"Opus"}]}"#,
    )
    .unwrap();
    let bad_app_tasks = claude_app_home
        .join("claude-code-sessions")
        .join("bad")
        .join("scheduled-tasks.json");
    fs::create_dir_all(bad_app_tasks.parent().unwrap()).unwrap();
    fs::write(&bad_app_tasks, "{not json").unwrap();
    let app_skill_dir = claude_app_home
        .join("local-agent-mode-sessions")
        .join("skills-plugin")
        .join("sentra")
        .join("skills")
        .join("scheduled");
    fs::create_dir_all(&app_skill_dir).unwrap();
    let app_skill_file = app_skill_dir.join("SKILL.md");
    fs::write(
        &app_skill_file,
        "---\nname: scheduled skill\ndescription: Run from Claude App\n---\nbody",
    )
    .unwrap();
    let good_app_tasks = claude_app_home
        .join("claude-code-sessions")
        .join("good")
        .join("scheduled-tasks.json");
    fs::create_dir_all(good_app_tasks.parent().unwrap()).unwrap();
    fs::write(
        &good_app_tasks,
        format!(
            r#"{{"scheduledTasks":[{{"id":"app-task","cronExpression":"*/5 * * * *","enabled":true,"filePath":{},"createdAt":1,"updatedAt":2,"cwd":"/app"}}]}}"#,
            serde_json::to_string(&app_skill_file.to_string_lossy()).unwrap()
        ),
    )
    .unwrap();

    let hermes_home = dir.path().join(".hermes");
    fs::create_dir_all(hermes_home.join("cron")).unwrap();
    fs::write(
        hermes_home.join("config.yaml"),
        "_config_version: 7\nmcp_servers:\n  hermes:\n    command: python\nchat_providers:\n  - name: nous\n    base_url: https://nous.example.com\n    api_key: sk12\n    models:\n      hermes-4:\n        name: Hermes 4\n",
    )
    .unwrap();
    fs::write(
        hermes_home.join("cron").join("jobs.json"),
        r#"{"jobs":[{"id":"h1","name":"Hermes job","prompt":"learn","enabled":true,"schedule":{"kind":"interval","minutes":15},"workdir":"/tmp"}]}"#,
    )
    .unwrap();

    let openclaw_home = dir.path().join(".openclaw");
    fs::create_dir_all(openclaw_home.join("cron")).unwrap();
    fs::write(
        openclaw_home.join("openclaw.json"),
        r#"{"name":"OpenClaw","mcp":{"servers":{"screen":{"command":"python"}}},"providers":{"local":{"baseUrl":"https://openclaw.example.com","apiKey":"sk12","models":[{"id":"oc-1","name":"OC One"}]}}}"#,
    )
    .unwrap();
    fs::write(
        openclaw_home.join("cron").join("jobs.json"),
        r#"{"jobs":[{"id":"o1","enabled":true,"schedule":{"kind":"every","every":"10m"},"payload":{"prompt":"observe","cwd":"/workspace"}}]}"#,
    )
    .unwrap();
    let bin_dir = dir.path().join(".local").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    for binary in ["claude", "hermes", "openclaw"] {
        fs::write(bin_dir.join(test_binary_name(binary)), "").unwrap();
    }
    let claude_app_bin = dir
        .path()
        .join("AppData")
        .join("Local")
        .join("Programs")
        .join("Claude");
    fs::create_dir_all(&claude_app_bin).unwrap();
    fs::write(claude_app_bin.join(test_binary_name("Claude")), "").unwrap();

    let agents = discover_agents(dir.path());
    for (name, title) in [
        ("claude-cli", "Claude Code"),
        ("claude-app", "Claude App"),
        ("hermes", "Hermes"),
        ("openclaw", "OpenClaw"),
    ] {
        let agent = agents.iter().find(|agent| agent.name() == name).unwrap();
        assert_eq!(agent.title(), title);
        assert_eq!(agent.get_assets(AssetType::Meta).unwrap().len(), 1);
        let meta = asset_data(agent, AssetType::Meta);
        assert_eq!(meta[0].data["installed"], true);
        assert_eq!(agent.get_assets(AssetType::Skill).unwrap().len(), 1);
        assert_eq!(agent.get_assets(AssetType::Mcp).unwrap().len(), 1);
        assert_eq!(agent.get_assets(AssetType::Cron).unwrap().len(), 1);
        assert_eq!(agent.get_assets(AssetType::Provider).unwrap().len(), 1);
    }

    let claude_cli = agents
        .iter()
        .find(|agent| agent.name() == "claude-cli")
        .unwrap();
    let cli_mcp = asset_data(claude_cli, AssetType::Mcp);
    assert!(cli_mcp[0].data.as_array().unwrap().iter().any(|mcp| {
        mcp["name"] == "project" && mcp["project"] == "/work" && mcp["type"] == "sse"
    }));
    let cli_provider = asset_data(claude_cli, AssetType::Provider);
    assert_eq!(
        cli_provider[0].data[0]["models"][0]["id"],
        "claude-sonnet-4"
    );
    assert_eq!(cli_provider[0].data[0]["enabled"], true);
    assert!(cli_provider[0].data[0]["protocol"].is_null());
    let cli_cron = asset_data(claude_cli, AssetType::Cron);
    assert_eq!(cli_cron[0].data[0]["schedule"], "0 9 * * *");

    let claude_app = agents
        .iter()
        .find(|agent| agent.name() == "claude-app")
        .unwrap();
    let app_provider = asset_data(claude_app, AssetType::Provider);
    assert_eq!(
        app_provider[0].data[0]["baseUrl"],
        "https://gateway.example.com"
    );
    assert_eq!(app_provider[0].data[0]["models"][0]["name"], "Opus");
    assert_eq!(app_provider[0].data[0]["enabled"], true);
    let app_cron = asset_data(claude_app, AssetType::Cron);
    assert_eq!(app_cron[0].data[0]["name"], "scheduled skill");
    assert_eq!(app_cron[0].data[0]["prompt"], "Run from Claude App");
    assert_eq!(app_cron[0].data[0]["files"][0]["path"], "SKILL.md");

    let hermes = agents
        .iter()
        .find(|agent| agent.name() == "hermes")
        .unwrap();
    let hermes_meta = asset_data(hermes, AssetType::Meta);
    assert_eq!(hermes_meta[0].data["version"], "7");
    let hermes_provider = asset_data(hermes, AssetType::Provider);
    assert_eq!(hermes_provider[0].data[0]["models"][0]["id"], "hermes-4");
    assert_eq!(hermes_provider[0].data[0]["apiKey"], "sk****12");
    assert_eq!(hermes_provider[0].data[0]["enabled"], true);
    let hermes_cron = asset_data(hermes, AssetType::Cron);
    assert_eq!(hermes_cron[0].data[0]["type"], "every");
    assert_eq!(hermes_cron[0].data[0]["schedule"], "15m");

    let openclaw = agents
        .iter()
        .find(|agent| agent.name() == "openclaw")
        .unwrap();
    let openclaw_provider = asset_data(openclaw, AssetType::Provider);
    assert_eq!(openclaw_provider[0].data[0]["models"][0]["id"], "oc-1");
    assert_eq!(openclaw_provider[0].data[0]["apiKey"], "sk****12");
    assert_eq!(openclaw_provider[0].data[0]["enabled"], true);
    let openclaw_cron = asset_data(openclaw, AssetType::Cron);
    assert_eq!(openclaw_cron[0].data[0]["cwds"][0], "/workspace");
}

fn test_binary_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn opencode_command_exists() -> bool {
    let output = if cfg!(windows) {
        std::process::Command::new("where").arg("opencode").output()
    } else {
        std::process::Command::new("sh")
            .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "sentra"])
            .arg("opencode")
            .output()
    };
    output.is_ok_and(|output| output.status.success())
}

fn asset_data(agent: &sentra_lib::agents::Agent, asset_type: AssetType) -> Vec<AssetData> {
    agent
        .get_assets(asset_type)
        .unwrap()
        .into_iter()
        .map(|asset| AssetData {
            asset_type: asset.asset_type(),
            data: serde_json::from_str(&asset.data().unwrap().to_string()).unwrap(),
        })
        .collect()
}

struct AssetData {
    asset_type: AssetType,
    data: serde_json::Value,
}
