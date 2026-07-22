use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::agents::{
    Agent,
    entries::{AgentEntry, SystemAgentPath, builtin_agent_entries},
};
use crate::interfaces::ProcessData;

fn titleize_agent_name(name: &str) -> String {
    name.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.len() <= 3 {
                part.to_ascii_uppercase()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn get_agent_title(agent_name: &str) -> String {
    builtin_agent_entries()
        .into_iter()
        .find(|entry| entry.name == agent_name)
        .and_then(|entry| entry.title.map(str::to_string))
        .unwrap_or_else(|| titleize_agent_name(agent_name))
}

pub fn discover_agents(user_home: impl AsRef<Path>) -> Vec<Agent> {
    let user_home = user_home.as_ref();
    let mut results = Vec::new();
    results.extend(crate::agents::codex::discover_agents(user_home));
    results.extend(crate::agents::claude_cli::discover_agents(user_home));
    results.extend(crate::agents::claude_app::discover_agents(user_home));
    results.extend(crate::agents::hermes::discover_agents(user_home));
    results.extend(crate::agents::kimi_code::discover_agents(user_home));
    results.extend(crate::agents::openclaw::discover_agents(user_home));
    results.extend(crate::agents::opencode::discover_agents(user_home));
    results.extend(crate::agents::pi::discover_agents(user_home));
    results.extend(crate::agents::sentra::discover_agents(user_home));
    results.extend(crate::agents::antigravity::discover_agents(user_home));
    results.extend(crate::agents::codebuddy::discover_agents(user_home));
    results.extend(crate::agents::coder::discover_agents(user_home));
    results.extend(crate::agents::cursor::discover_agents(user_home));
    results.extend(crate::agents::kiro::discover_agents(user_home));
    results.extend(crate::agents::lingcode::discover_agents(user_home));
    results.extend(crate::agents::marvis::discover_agents(user_home));
    results.extend(crate::agents::qoder::discover_agents(user_home));
    results.extend(crate::agents::qoderwork::discover_agents(user_home));
    results.extend(crate::agents::trae::discover_agents(user_home));
    results.extend(crate::agents::vscode::discover_agents(user_home));
    results.extend(crate::agents::workbuddy::discover_agents(user_home));
    results.extend(crate::agents::general::discover_agents(user_home));
    results
}

pub(crate) fn discover_entry_agents(user_home: &Path, entries: &[AgentEntry]) -> Vec<Agent> {
    let mut results = Vec::new();
    for entry in entries {
        let custom_homes = custom_homes_from_entry(user_home, entry);

        let mut detected_install_home = None;
        let mut static_home_found = false;
        for segments in entry.homes {
            let home = entry_home(user_home, segments);
            let home_exists = fs::metadata(&home)
                .map(|meta| meta.is_dir())
                .unwrap_or(false);
            if home_exists {
                static_home_found = true;
                push_agent_if_missing(&mut results, entry, home);
            } else if custom_homes.is_empty()
                && !static_home_found
                && detected_install_home.is_none()
                && (entry.is_installed)(entry.name, &home)
            {
                detected_install_home = Some(home);
            }
        }
        if !static_home_found && let Some(home) = detected_install_home {
            push_agent_if_missing(&mut results, entry, home);
        }

        for home in custom_homes {
            push_agent_if_missing(&mut results, entry, home);
        }
    }
    results
}

pub(crate) fn discover_installed_entry_agents(
    user_home: &Path,
    entries: &[&AgentEntry],
) -> Vec<Agent> {
    let mut results = Vec::new();
    for entry in entries {
        let entry = *entry;
        for segments in entry.homes {
            let home = entry_home(user_home, segments);
            if (entry.is_installed)(entry.name, &home) {
                push_agent_if_missing(&mut results, entry, home);
                break;
            }
        }
    }
    results
}

fn entry_home(user_home: &Path, segments: &[&str]) -> PathBuf {
    let mut home = user_home.to_path_buf();
    for segment in segments.iter() {
        home.push(segment);
    }
    home
}

fn custom_homes_from_entry(user_home: &Path, entry: &AgentEntry) -> Vec<PathBuf> {
    if entry.process_home_env_vars.is_empty() {
        return Vec::new();
    }

    let mut homes = Vec::new();
    let accept_external_homes =
        home::home_dir().is_some_and(|current_home| same_home(&current_home, user_home));
    for env_key in entry.process_home_env_vars {
        if let Some(value) = std::env::var_os(env_key)
            && let Some(home) = parse_process_home(user_home, &value.to_string_lossy())
            && (accept_external_homes || home_is_within(&home, user_home))
        {
            push_home_if_missing(&mut homes, home);
        }
    }
    for process in (entry.process_provider)() {
        for home in process_homes_from_env(user_home, entry, &process) {
            if accept_external_homes || home_is_within(&home, user_home) {
                push_home_if_missing(&mut homes, home);
            }
        }
    }
    homes
}

fn process_homes_from_env(
    user_home: &Path,
    entry: &AgentEntry,
    process: &ProcessData,
) -> Vec<PathBuf> {
    entry
        .process_home_env_vars
        .iter()
        .filter_map(|env_key| process_env_value(process, env_key))
        .filter_map(|value| parse_process_home(user_home, value))
        .collect()
}

fn process_env_value<'a>(process: &'a ProcessData, env_key: &str) -> Option<&'a str> {
    process.env.get(env_key).map(String::as_str).or_else(|| {
        process
            .env
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(env_key))
            .map(|(_, value)| value.as_str())
    })
}

fn parse_process_home(user_home: &Path, value: &str) -> Option<PathBuf> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if value.is_empty() {
        return None;
    }

    if value == "~" {
        return Some(user_home.to_path_buf());
    }
    if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return Some(user_home.join(relative));
    }

    Some(PathBuf::from(value))
}

fn push_agent_if_missing(results: &mut Vec<Agent>, entry: &AgentEntry, home: PathBuf) {
    if results
        .iter()
        .any(|agent| agent.name() == entry.name && same_home(agent.home(), &home))
    {
        return;
    }
    results.push(Agent::new(entry, home));
}

fn push_home_if_missing(homes: &mut Vec<PathBuf>, home: PathBuf) {
    if !homes.iter().any(|existing| same_home(existing, &home)) {
        homes.push(home);
    }
}

fn same_home(left: &Path, right: &Path) -> bool {
    let left = home_key(left);
    let right = home_key(right);
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn home_is_within(home: &Path, user_home: &Path) -> bool {
    let home = normalize_home(home);
    let user_home = normalize_home(user_home);
    #[cfg(windows)]
    {
        PathBuf::from(home.to_string_lossy().to_ascii_lowercase()).starts_with(PathBuf::from(
            user_home.to_string_lossy().to_ascii_lowercase(),
        ))
    }
    #[cfg(not(windows))]
    {
        home.starts_with(user_home)
    }
}

fn home_key(home: &Path) -> PathBuf {
    fs::canonicalize(home).unwrap_or_else(|_| normalize_home(home))
}

fn normalize_home(home: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in home.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn discover_system_agents(system_paths: &[SystemAgentPath]) -> Vec<Agent> {
    let mut results = Vec::new();
    for item in system_paths {
        let home = Path::new(item.system_path);
        if fs::metadata(home)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
            && !results
                .iter()
                .any(|agent: &Agent| agent.name() == item.entry.name && agent.home() == home)
        {
            results.push(Agent::new(item.entry, home));
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Mutex;

    use super::*;
    use crate::interfaces::{AssetType, ErasedAsset};

    static TEST_PROCESSES: Mutex<Vec<ProcessData>> = Mutex::new(Vec::new());

    #[test]
    fn process_env_homes_are_discovered_without_duplicate_static_home() {
        let dir = tempfile::tempdir().unwrap();
        let static_home = dir.path().join(".codex");
        let custom_home = dir.path().join("custom-codex");
        fs::create_dir_all(&static_home).unwrap();
        fs::create_dir_all(&custom_home).unwrap();

        set_test_processes(vec![
            process_with_home("CODEX_HOME", &static_home),
            process_with_home("codex_home", &static_home),
            process_with_home("CODEX_HOME", &custom_home),
        ]);

        let entry = test_entry(test_process_data, never_installed);
        let agents = discover_entry_agents(dir.path(), std::slice::from_ref(&entry));
        set_test_processes(Vec::new());

        let codex_homes = agents
            .iter()
            .filter(|agent| agent.name() == "codex-cli")
            .map(|agent| home_key(agent.home()))
            .collect::<Vec<_>>();

        assert_eq!(codex_homes.len(), 2);
        assert!(codex_homes.contains(&home_key(&static_home)));
        assert!(codex_homes.contains(&home_key(&custom_home)));

        let missing_static_dir = tempfile::tempdir().unwrap();
        let custom_home = missing_static_dir.path().join("custom-codex");
        set_test_processes(vec![process_with_home("CODEX_HOME", &custom_home)]);
        let entry = test_entry(test_process_data, always_installed);
        let agents = discover_entry_agents(missing_static_dir.path(), std::slice::from_ref(&entry));
        set_test_processes(Vec::new());

        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].home(), custom_home.as_path());
    }

    #[test]
    fn installed_detector_home_is_returned_when_home_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let entry = test_entry(crate::agents::entries::empty_process_data, always_installed);
        let agents = discover_entry_agents(dir.path(), std::slice::from_ref(&entry));
        let expected_home = dir.path().join(".codex");

        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].home(), expected_home.as_path());
    }

    #[cfg(windows)]
    #[test]
    fn windows_home_comparison_ignores_ascii_case() {
        assert!(same_home(
            Path::new(r"C:\Users\Me\Custom-Codex"),
            Path::new(r"c:\users\me\custom-codex")
        ));
    }

    fn test_entry(
        process_provider: crate::agents::entries::AgentProcessProvider,
        is_installed: crate::agents::entries::AgentInstallDetector,
    ) -> AgentEntry {
        AgentEntry {
            name: "codex-cli",
            title: Some("Codex"),
            homes: &[&[".codex"]],
            asset_for_type: test_assets,
            is_installed,
            process_provider,
            process_home_env_vars: &["CODEX_HOME"],
        }
    }

    fn test_assets(
        _agent_name: &str,
        _agent_home: &Path,
        _asset_type: AssetType,
    ) -> Vec<Box<dyn ErasedAsset>> {
        Vec::new()
    }

    fn test_process_data() -> Vec<ProcessData> {
        TEST_PROCESSES.lock().unwrap().clone()
    }

    fn set_test_processes(processes: Vec<ProcessData>) {
        *TEST_PROCESSES.lock().unwrap() = processes;
    }

    fn process_with_home(env_key: &str, home: &Path) -> ProcessData {
        let mut env = BTreeMap::new();
        env.insert(env_key.to_string(), home.to_string_lossy().to_string());
        ProcessData {
            pid: 1,
            name: "codex".to_string(),
            cmdline: vec!["codex".to_string()],
            started_at: 0,
            run_time_seconds: 0,
            path: None,
            env,
        }
    }

    fn never_installed(_agent_name: &str, _agent_home: &Path) -> bool {
        false
    }

    fn always_installed(_agent_name: &str, _agent_home: &Path) -> bool {
        true
    }
}
