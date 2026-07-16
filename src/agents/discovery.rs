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
    results.extend(crate::agents::openclaw::discover_agents(user_home));
    results.extend(crate::agents::opencode::discover_agents(user_home));
    results.extend(crate::agents::pi::discover_agents(user_home));
    results.extend(crate::agents::sentra::discover_agents(user_home));
    results.extend(crate::agents::general::discover_agents(user_home));
    results
}

pub(crate) fn discover_entry_agents(user_home: &Path, entries: &[AgentEntry]) -> Vec<Agent> {
    let mut results = Vec::new();
    for entry in entries {
        let process_homes = process_homes_from_entry(user_home, entry);

        let mut installed_home = None;
        for segments in entry.homes {
            let home = entry_home(user_home, segments);
            let home_exists = fs::metadata(&home)
                .map(|meta| meta.is_dir())
                .unwrap_or(false);
            if home_exists {
                installed_home = Some(home.clone());
                push_agent_if_missing(&mut results, entry, home);
            } else if installed_home.is_none() && (entry.is_installed)(entry.name, &home) {
                installed_home = Some(home);
            }
        }
        if let Some(home) = installed_home {
            push_agent_if_missing(&mut results, entry, home);
        }

        for home in process_homes {
            push_agent_if_missing(&mut results, entry, home);
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

fn process_homes_from_entry(user_home: &Path, entry: &AgentEntry) -> Vec<PathBuf> {
    if entry.process_home_env_vars.is_empty() {
        return Vec::new();
    }

    (entry.process_provider)()
        .iter()
        .flat_map(|process| process_homes_from_env(user_home, entry, process))
        .collect()
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

fn same_home(left: &Path, right: &Path) -> bool {
    home_key(left) == home_key(right)
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
            .filter(|agent| agent.name() == "codex")
            .map(|agent| home_key(agent.home()))
            .collect::<Vec<_>>();

        assert_eq!(codex_homes.len(), 2);
        assert!(codex_homes.contains(&home_key(&static_home)));
        assert!(codex_homes.contains(&home_key(&custom_home)));
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

    fn test_entry(
        process_provider: crate::agents::entries::AgentProcessProvider,
        is_installed: crate::agents::entries::AgentInstallDetector,
    ) -> AgentEntry {
        AgentEntry {
            name: "codex",
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
