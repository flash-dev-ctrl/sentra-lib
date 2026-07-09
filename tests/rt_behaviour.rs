use std::fs;

use sentra_lib::agents::discover_agents;
use sentra_lib::collect_skills_from_dir;
use sentra_lib::interfaces::{AssetType, CronData, CronType, McpData, McpType, SkillData};
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
    unsafe {
        std::env::set_var("SENTRA_RT_CODEX_TEST_KEY", "sk-from-env");
    }
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
    assert_eq!(providers[0].data[0]["apiKey"], "sk-from-env");
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
    unsafe {
        std::env::set_var("SENTRA_PI_TEST_KEY", "sk-pi");
    }

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
    assert_eq!(provider["apiKey"], "sk-pi");
    assert_eq!(provider["enabled"], true);
    assert_eq!(provider["protocol"], "responses");
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
    assert_eq!(provider["protocol"], "chat_completions");
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

    assert_eq!(sentra_agent.get_assets(AssetType::Skill).unwrap().len(), 1);
    let providers = asset_data(sentra_agent, AssetType::Provider);

    assert_eq!(
        providers[0].data[0]["baseUrl"],
        "https://api.example.com/v1"
    );
    assert_eq!(providers[0].data[0]["models"][0]["id"], "gpt-5");
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
    let openclaw_cron = asset_data(openclaw, AssetType::Cron);
    assert_eq!(openclaw_cron[0].data[0]["cwds"][0], "/workspace");
}

fn asset_data(agent: &sentra_lib::agents::Agent, asset_type: AssetType) -> Vec<AssetData> {
    agent
        .get_assets(asset_type)
        .unwrap()
        .into_iter()
        .map(|asset| AssetData {
            asset_type: asset.asset_type(),
            data: asset.data().unwrap(),
        })
        .collect()
}

struct AssetData {
    asset_type: AssetType,
    data: serde_json::Value,
}
