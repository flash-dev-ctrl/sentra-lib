use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::agents::{
    Agent,
    entries::{AgentEntry, SYSTEM_AGENT_PATHS, SystemAgentPath, builtin_agent_entries},
};
use crate::interfaces::{AssetType, ProcessData};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgentDiscoveryOptions {
    pub skip_install_probe: bool,
}

pub fn discover_agents(user_home: impl AsRef<Path>) -> Vec<Agent> {
    discover_agents_with_options(user_home, AgentDiscoveryOptions::default())
}

pub fn discover_agents_with_options(
    user_home: impl AsRef<Path>,
    options: AgentDiscoveryOptions,
) -> Vec<Agent> {
    discover_agents_from_entries(user_home.as_ref(), options, |_| true)
}

pub fn discover_agents_matching(
    user_home: impl AsRef<Path>,
    matches: impl FnMut(&str) -> bool,
) -> Vec<Agent> {
    discover_agents_matching_with_options(user_home, AgentDiscoveryOptions::default(), matches)
}

pub fn discover_agents_matching_with_options(
    user_home: impl AsRef<Path>,
    options: AgentDiscoveryOptions,
    mut matches: impl FnMut(&str) -> bool,
) -> Vec<Agent> {
    discover_agents_from_entries(user_home.as_ref(), options, |entry| matches(entry.name))
}

pub fn discover_agents_with_asset(
    user_home: impl AsRef<Path>,
    asset_type: AssetType,
) -> Vec<Agent> {
    discover_agents_with_asset_and_options(user_home, asset_type, AgentDiscoveryOptions::default())
}

pub fn discover_agents_with_asset_and_options(
    user_home: impl AsRef<Path>,
    asset_type: AssetType,
    options: AgentDiscoveryOptions,
) -> Vec<Agent> {
    let user_home = user_home.as_ref();
    if asset_type == AssetType::Provider {
        return discover_provider_agents(user_home, options);
    }
    discover_agents_with_options(user_home, options)
}

fn discover_agents_from_entries(
    user_home: &Path,
    options: AgentDiscoveryOptions,
    mut include_entry: impl FnMut(&AgentEntry) -> bool,
) -> Vec<Agent> {
    let user_home = user_home.as_ref();
    let entries = builtin_agent_entries()
        .into_iter()
        .filter(|entry| include_entry(entry))
        .collect::<Vec<_>>();
    let mut results = if options == AgentDiscoveryOptions::default() {
        discover_entry_agents(user_home, &entries)
    } else {
        discover_entry_agents_with_options(user_home, &entries, options)
    };
    let system_paths = SYSTEM_AGENT_PATHS
        .iter()
        .copied()
        .filter(|path| include_entry(path.entry))
        .collect::<Vec<_>>();
    results.extend(discover_system_agents(&system_paths));
    results
}

fn entry_supports_asset(entry: &AgentEntry, asset_type: AssetType) -> bool {
    !(entry.asset_for_type)(entry.name, Path::new(""), asset_type).is_empty()
}

fn discover_provider_agents(user_home: &Path, options: AgentDiscoveryOptions) -> Vec<Agent> {
    discover_agents_from_entries(user_home, options, |entry| {
        entry_supports_asset(entry, AssetType::Provider)
    })
}

pub(crate) fn discover_entry_agents(user_home: &Path, entries: &[AgentEntry]) -> Vec<Agent> {
    discover_entry_agents_with_options(user_home, entries, AgentDiscoveryOptions::default())
}

pub(crate) fn discover_entry_agents_with_options(
    user_home: &Path,
    entries: &[AgentEntry],
    options: AgentDiscoveryOptions,
) -> Vec<Agent> {
    let mut results = Vec::new();
    for entry in entries {
        let mut home_found = false;
        for segments in entry.homes {
            let home = entry_home(user_home, segments);
            let home_exists = fs::metadata(&home)
                .map(|meta| meta.is_dir())
                .unwrap_or(false);
            if home_exists {
                home_found = true;
                push_agent_if_missing(&mut results, entry, home);
            }
        }

        if !home_found && should_probe_installed_entries(user_home, options) {
            for segments in entry.homes {
                let home = entry_home(user_home, segments);
                if (entry.is_installed)(entry.name, &home) {
                    push_agent_if_missing(&mut results, entry, home);
                    break;
                }
            }
        }

        for home in custom_homes_from_entry(user_home, entry) {
            push_agent_if_missing(&mut results, entry, home);
        }
    }
    results
}

fn should_probe_installed_entries(user_home: &Path, options: AgentDiscoveryOptions) -> bool {
    !options.skip_install_probe
        && home::home_dir().is_some_and(|current_home| same_home(&current_home, user_home))
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
    if !accept_external_homes {
        return homes;
    }
    for process in (entry.process_provider)() {
        for home in process_homes_from_env(user_home, entry, &process) {
            push_home_if_missing(&mut homes, home);
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
    static TEST_INSTALLED_PROCESSES: Mutex<Vec<ProcessData>> = Mutex::new(Vec::new());
    static TEST_PROCESS_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn process_env_homes_are_discovered_for_current_home() {
        let _guard = TEST_PROCESS_ENV_LOCK.lock().unwrap();
        let Some(current_home) = home::home_dir() else {
            return;
        };
        let static_home = current_home.join(".codex");
        let custom_home = current_home.join("custom-codex");

        set_test_processes(vec![
            process_with_home("SENTRA_TEST_DISCOVERY_HOME", &static_home),
            process_with_home("sentra_test_discovery_home", &static_home),
            process_with_home("SENTRA_TEST_DISCOVERY_HOME", &custom_home),
        ]);

        let entry = test_entry_with_env_vars(
            test_process_data,
            never_installed,
            &["SENTRA_TEST_DISCOVERY_HOME"],
        );
        let homes = custom_homes_from_entry(&current_home, &entry);
        set_test_processes(Vec::new());

        assert_eq!(homes.len(), 2);
        assert!(homes.contains(&static_home));
        assert!(homes.contains(&custom_home));
    }

    #[test]
    fn process_env_homes_are_not_scanned_for_external_home() {
        let _guard = TEST_PROCESS_ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let custom_home = dir.path().join("custom-codex");

        set_test_processes(vec![process_with_home(
            "SENTRA_TEST_DISCOVERY_HOME",
            &custom_home,
        )]);

        let entry = test_entry_with_env_vars(
            test_process_data,
            never_installed,
            &["SENTRA_TEST_DISCOVERY_HOME"],
        );
        let agents = discover_entry_agents(dir.path(), std::slice::from_ref(&entry));
        set_test_processes(Vec::new());

        assert!(agents.is_empty());
    }

    #[test]
    fn entry_discovery_skips_install_probe_for_external_home() {
        let dir = tempfile::tempdir().unwrap();
        let entry = test_entry(crate::agents::entries::empty_process_data, always_installed);
        let agents = discover_entry_agents(dir.path(), std::slice::from_ref(&entry));

        assert!(agents.is_empty());
    }

    #[test]
    fn entry_discovery_uses_install_probe_when_home_is_missing() {
        let Some(current_home) = home::home_dir() else {
            return;
        };
        let entry = test_entry(crate::agents::entries::empty_process_data, always_installed);
        let agents = discover_entry_agents(&current_home, std::slice::from_ref(&entry));
        let expected_home = current_home.join(".codex");

        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].home(), expected_home.as_path());
    }

    #[test]
    fn entry_discovery_can_skip_install_probe() {
        let Some(current_home) = home::home_dir() else {
            return;
        };
        let entry = test_entry(crate::agents::entries::empty_process_data, always_installed);
        let agents = discover_entry_agents_with_options(
            &current_home,
            std::slice::from_ref(&entry),
            AgentDiscoveryOptions {
                skip_install_probe: true,
            },
        );

        assert!(agents.is_empty());
    }

    #[test]
    fn process_provider_is_not_called_without_process_home_env_vars() {
        let _guard = TEST_PROCESS_ENV_LOCK.lock().unwrap();
        let Some(current_home) = home::home_dir() else {
            return;
        };
        set_test_processes(vec![process_with_home(
            "SENTRA_TEST_DISCOVERY_HOME",
            &current_home.join("custom-process-home"),
        )]);
        let entry = test_entry_with_env_vars(test_process_data, never_installed, &[]);

        let homes = custom_homes_from_entry(&current_home, &entry);
        set_test_processes(Vec::new());

        assert!(homes.is_empty());
    }

    #[test]
    fn entry_discovery_includes_process_env_homes_for_current_home() {
        let Some(current_home) = home::home_dir() else {
            return;
        };
        let custom_home = current_home.join("custom-process-home");

        set_test_installed_processes(vec![process_with_home(
            "SENTRA_TEST_DISCOVERY_HOME",
            &custom_home,
        )]);

        let entry = AgentEntry {
            name: "codex-cli",
            title: Some("Codex"),
            homes: &[&[".missing-installed-home"]],
            asset_for_type: test_assets,
            is_installed: installed_only_for_custom_home,
            process_provider: test_installed_process_data,
            process_home_env_vars: &["SENTRA_TEST_DISCOVERY_HOME"],
        };
        let agents = discover_entry_agents(&current_home, &[entry]);
        set_test_installed_processes(Vec::new());

        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].home(), custom_home.as_path());
    }

    #[test]
    fn entry_discovery_supports_home_and_installed_fallback_entries() {
        let Some(current_home) = home::home_dir() else {
            return;
        };

        let home_entry = AgentEntry {
            name: "sentra-test-home",
            title: Some("Sentra Test Home"),
            homes: &[&[]],
            asset_for_type: test_assets,
            is_installed: never_installed,
            process_provider: crate::agents::entries::empty_process_data,
            process_home_env_vars: &[],
        };
        let installed_entry = AgentEntry {
            name: "sentra-test-installed",
            title: Some("Sentra Test Installed"),
            homes: &[&[".sentra-test-installed-entry"]],
            asset_for_type: test_assets,
            is_installed: always_installed,
            process_provider: crate::agents::entries::empty_process_data,
            process_home_env_vars: &[],
        };

        let agents = discover_entry_agents(&current_home, &[home_entry, installed_entry]);
        let names = agents.iter().map(|agent| agent.name()).collect::<Vec<_>>();

        assert!(names.contains(&"sentra-test-home"));
        assert!(names.contains(&"sentra-test-installed"));
    }

    #[test]
    fn matching_discovery_only_visits_matching_entries() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        let sentra_home = dir.path().join(".sentra");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(&sentra_home).unwrap();

        let agents = discover_agents_matching(dir.path(), |name| name == "codex-cli");

        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name(), "codex-cli");
        assert_eq!(agents[0].home(), codex_home.as_path());
    }

    #[test]
    fn provider_asset_discovery_preserves_surface_specific_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        let general_home = dir.path().join(".agents");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(&general_home).unwrap();

        let agents = discover_agents_with_asset(dir.path(), AssetType::Provider);
        let names = agents.iter().map(|agent| agent.name()).collect::<Vec<_>>();

        assert!(names.contains(&"codex-cli"));
        assert!(names.contains(&"codex-app"));
        assert!(names.contains(&"codex-cli-ide"));
        assert!(!names.contains(&"agents"));
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
        test_entry_with_env_vars(process_provider, is_installed, &["CODEX_HOME"])
    }

    fn test_entry_with_env_vars(
        process_provider: crate::agents::entries::AgentProcessProvider,
        is_installed: crate::agents::entries::AgentInstallDetector,
        process_home_env_vars: &'static [&'static str],
    ) -> AgentEntry {
        AgentEntry {
            name: "codex-cli",
            title: Some("Codex"),
            homes: &[&[".codex"]],
            asset_for_type: test_assets,
            is_installed,
            process_provider,
            process_home_env_vars,
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

    fn test_installed_process_data() -> Vec<ProcessData> {
        TEST_INSTALLED_PROCESSES.lock().unwrap().clone()
    }

    fn set_test_processes(processes: Vec<ProcessData>) {
        *TEST_PROCESSES.lock().unwrap() = processes;
    }

    fn set_test_installed_processes(processes: Vec<ProcessData>) {
        *TEST_INSTALLED_PROCESSES.lock().unwrap() = processes;
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

    fn installed_only_for_custom_home(_agent_name: &str, agent_home: &Path) -> bool {
        agent_home
            .file_name()
            .is_some_and(|name| name == "custom-process-home")
    }

    fn always_installed(_agent_name: &str, _agent_home: &Path) -> bool {
        true
    }
}
