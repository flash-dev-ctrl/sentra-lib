use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::SentraResult;
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, ProcessData};
use crate::utils::sanitize_command_args;
use crate::utils::sanitize_env_value;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub(crate) type ProcessMatcher = fn(&ProcessInfo<'_>) -> bool;

#[derive(Debug)]
pub(crate) struct ProcessInfo<'a> {
    pub(crate) name: &'a str,
    pub(crate) cmdline: &'a [String],
    pub(crate) path: Option<&'a Path>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessAsset {
    core: AssetCore,
    matcher: ProcessMatcher,
}

impl ProcessAsset {
    pub(crate) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<PathBuf>,
        matcher: ProcessMatcher,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
            matcher,
        }
    }
}

impl_erased_asset!(ProcessAsset, AssetType::Process, Vec<ProcessData>);

impl Asset<Vec<ProcessData>> for ProcessAsset {
    fn get_data(&self) -> SentraResult<Vec<ProcessData>> {
        Ok(process_data(self.matcher))
    }
}

pub(crate) fn process_data(matcher: ProcessMatcher) -> Vec<ProcessData> {
    let mut results = process_snapshot()
        .iter()
        .filter(|process| {
            let info = ProcessInfo {
                name: &process.name,
                cmdline: &process.cmdline,
                path: process.path.as_deref(),
            };
            matcher(&info)
        })
        .map(|process| process.data.clone())
        .collect::<Vec<_>>();
    fill_process_environments(&mut results);
    results.sort_by_key(|process| process.pid);
    results
}

fn fill_process_environments(processes: &mut [ProcessData]) {
    if processes.is_empty() {
        return;
    }
    let pids = processes
        .iter()
        .map(|process| Pid::from_u32(process.pid))
        .collect::<Vec<_>>();
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::nothing()
            .with_environ(UpdateKind::Always)
            .without_tasks(),
    );
    for process in processes {
        let pid = Pid::from_u32(process.pid);
        if let Some(updated) = system.processes().get(&pid) {
            process.env = sanitized_env(updated.environ());
        }
    }
}

const PROCESS_SNAPSHOT_TTL: Duration = Duration::from_secs(2);

static PROCESS_SNAPSHOT_CACHE: OnceLock<Mutex<ProcessSnapshotCache>> = OnceLock::new();

#[derive(Debug, Default)]
struct ProcessSnapshotCache {
    collected_at: Option<Instant>,
    snapshots: Vec<ProcessSnapshot>,
}

impl ProcessSnapshotCache {
    fn snapshot(&mut self) -> Vec<ProcessSnapshot> {
        self.snapshot_with(Instant::now(), collect_process_snapshot)
    }

    fn snapshot_with(
        &mut self,
        now: Instant,
        collect: impl FnOnce() -> Vec<ProcessSnapshot>,
    ) -> Vec<ProcessSnapshot> {
        if self
            .collected_at
            .is_none_or(|collected_at| now.duration_since(collected_at) >= PROCESS_SNAPSHOT_TTL)
        {
            self.snapshots = collect();
            self.collected_at = Some(now);
        }
        self.snapshots.clone()
    }
}

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    name: String,
    cmdline: Vec<String>,
    path: Option<PathBuf>,
    data: ProcessData,
}

fn process_snapshot() -> Vec<ProcessSnapshot> {
    let cache = PROCESS_SNAPSHOT_CACHE.get_or_init(|| Mutex::new(ProcessSnapshotCache::default()));
    let mut cache = cache.lock().unwrap_or_else(|err| err.into_inner());
    cache.snapshot()
}

fn collect_process_snapshot() -> Vec<ProcessSnapshot> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always)
            .without_tasks(),
    );

    system
        .processes()
        .values()
        .map(process_record)
        .collect::<Vec<_>>()
}

fn process_record(process: &sysinfo::Process) -> ProcessSnapshot {
    let name = os_to_string(process.name());
    let cmdline = process
        .cmd()
        .iter()
        .map(|arg| os_to_string(arg))
        .collect::<Vec<_>>();
    let path = process.exe().map(Path::to_path_buf);
    let data = ProcessData {
        pid: process.pid().as_u32(),
        name: name.clone(),
        cmdline: sanitize_command_args(&cmdline),
        started_at: process.start_time(),
        run_time_seconds: process.run_time(),
        path: path.clone(),
        env: sanitized_env(process.environ()),
    };
    ProcessSnapshot {
        name,
        cmdline,
        path,
        data,
    }
}

pub(crate) fn matches_binary_names(process: &ProcessInfo<'_>, binary_names: &[&str]) -> bool {
    if is_binary_name(process.name, binary_names) {
        return true;
    }
    process
        .cmdline
        .first()
        .map(|command| is_binary_name(command_basename(command), binary_names))
        .unwrap_or(false)
}

pub(crate) fn is_binary_name(value: &str, binary_names: &[&str]) -> bool {
    let value = normalized_binary_name(value);
    binary_names
        .iter()
        .any(|binary| value == normalized_binary_name(binary))
}

pub(crate) fn path_has_component(path: &Path, components: &[&str]) -> bool {
    path.components().any(|component| {
        let Some(component) = component.as_os_str().to_str() else {
            return false;
        };
        let component = normalized_binary_name(component);
        components
            .iter()
            .any(|expected| component == normalized_binary_name(expected))
    })
}

pub(crate) fn process_has_ide_extension(process: &ProcessInfo<'_>, extension_id: &str) -> bool {
    process
        .path
        .is_some_and(|path| value_has_ide_extension(path.to_string_lossy().as_ref(), extension_id))
        || process
            .cmdline
            .iter()
            .any(|command| value_has_ide_extension(command, extension_id))
}

fn value_has_ide_extension(value: &str, extension_id: &str) -> bool {
    let extension_id = extension_id.to_ascii_lowercase();
    let components = value
        .trim_matches(['"', '\''])
        .split(['/', '\\'])
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    components.windows(2).any(|pair| {
        pair[0] == "extensions"
            && (pair[1] == extension_id
                || pair[1]
                    .strip_prefix(&extension_id)
                    .is_some_and(|suffix| suffix.starts_with('-')))
    })
}

pub(crate) fn cmdline_has_path_components(process: &ProcessInfo<'_>, components: &[&str]) -> bool {
    !components.is_empty()
        && process.cmdline.iter().any(|arg| {
            let path_components = arg
                .trim_matches(['"', '\''])
                .split(['/', '\\'])
                .map(normalized_binary_name)
                .collect::<Vec<_>>();
            let expected = components
                .iter()
                .map(|component| normalized_binary_name(component))
                .collect::<Vec<_>>();
            path_components
                .windows(expected.len())
                .any(|window| window == expected.as_slice())
        })
}

fn command_basename(command: &str) -> &str {
    command.rsplit(['/', '\\']).next().unwrap_or(command)
}

fn normalized_binary_name(value: &str) -> String {
    value
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

fn sanitized_env(entries: &[OsString]) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for entry in entries {
        let entry = entry.to_string_lossy();
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        let value = sanitize_env_value(key, value);
        env.insert(key.to_string(), value);
    }
    env
}

fn os_to_string(value: &OsStr) -> String {
    value.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_process_by_binary_name() {
        assert_matches_binary_names("codex", &[], None, &["codex", "codex.exe"]);
        assert_matches_binary_names("Codex.exe", &[], None, &["codex", "codex.exe"]);
    }

    #[test]
    fn matches_process_by_first_command() {
        assert_matches_binary_names(
            "node",
            &[r"C:\Users\me\AppData\Local\Programs\OpenAI\Codex\codex.exe"],
            None,
            &["codex", "codex.exe"],
        );
        assert_matches_binary_names(
            "node",
            &["/usr/local/bin/codex"],
            None,
            &["codex", "codex.exe"],
        );
    }

    #[test]
    fn does_not_match_binary_name_as_substring() {
        assert_not_matches_binary_names("my-codex-helper", &[], None, &["codex", "codex.exe"]);
        assert_not_matches_binary_names(
            "node",
            &["/usr/local/bin/my-codex-helper"],
            None,
            &["codex", "codex.exe"],
        );
    }

    #[test]
    fn matches_path_component_case_insensitively() {
        let chatgpt_path = Path::new("Users")
            .join("me")
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("OpenAI")
            .join("ChatGPT")
            .join("ChatGPT.exe");
        let temp_path = Path::new("temp").join("ChatGPT.exe");

        assert!(path_has_component(&chatgpt_path, &["openai", "chatgpt"]));
        assert!(!path_has_component(&temp_path, &["openai"]));
    }

    #[test]
    fn matches_versioned_ide_extension_paths_exactly() {
        let path = Path::new("Users")
            .join("me")
            .join(".devin")
            .join("extensions")
            .join("openai.chatgpt-1.2.3-win32-x64")
            .join("bin")
            .join("codex.exe");
        let cmdline = vec![
            r#"C:\Users\me\.vscode\extensions\Anthropic.claude-code-2.0\claude.exe"#.to_string(),
        ];
        let process = ProcessInfo {
            name: "codex.exe",
            cmdline: &cmdline,
            path: Some(&path),
        };

        assert!(process_has_ide_extension(&process, "openai.chatgpt"));
        assert!(process_has_ide_extension(&process, "anthropic.claude-code"));
        assert!(!process_has_ide_extension(&process, "openai.chat"));
    }

    #[test]
    fn matches_all_cmdline_path_components_exactly() {
        let cmdline = vec![
            "node.exe".to_string(),
            r#"C:\Users\me\node_modules\@scope\agent\cli.js"#.to_string(),
        ];
        let process = ProcessInfo {
            name: "node.exe",
            cmdline: &cmdline,
            path: None,
        };

        assert!(cmdline_has_path_components(&process, &["@scope", "agent"]));
        assert!(!cmdline_has_path_components(
            &process,
            &["@scope", "agent-helper"]
        ));
        assert!(!cmdline_has_path_components(
            &process,
            &["node_modules", "agent"]
        ));
    }

    #[test]
    fn sanitizes_sensitive_environment_values() {
        let env = sanitized_env(&[
            OsString::from("OPENAI_API_KEY=sk-1234567890"),
            OsString::from("PATH=/usr/bin"),
        ]);

        assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
        let api_key = env.get("OPENAI_API_KEY").unwrap();
        assert_eq!(api_key, "****");
    }

    #[test]
    fn process_snapshot_cache_refreshes_after_ttl() {
        let now = Instant::now();
        let mut cache = ProcessSnapshotCache::default();

        let first = cache.snapshot_with(now, || vec![test_snapshot(1)]);
        let second =
            cache.snapshot_with(now + Duration::from_millis(500), || vec![test_snapshot(2)]);
        let third = cache.snapshot_with(
            now + PROCESS_SNAPSHOT_TTL + Duration::from_millis(1),
            || vec![test_snapshot(3)],
        );

        assert_eq!(first[0].data.pid, 1);
        assert_eq!(second[0].data.pid, 1);
        assert_eq!(third[0].data.pid, 3);
    }

    fn assert_matches_binary_names(
        name: &str,
        cmdline: &[&str],
        path: Option<&Path>,
        binary_names: &[&str],
    ) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert!(matches_binary_names(&process, binary_names));
    }

    fn assert_not_matches_binary_names(
        name: &str,
        cmdline: &[&str],
        path: Option<&Path>,
        binary_names: &[&str],
    ) {
        let cmdline = cmdline
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let process = ProcessInfo {
            name,
            cmdline: &cmdline,
            path,
        };
        assert!(!matches_binary_names(&process, binary_names));
    }

    fn test_snapshot(pid: u32) -> ProcessSnapshot {
        ProcessSnapshot {
            name: "codex".to_string(),
            cmdline: vec!["codex".to_string()],
            path: None,
            data: ProcessData {
                pid,
                name: "codex".to_string(),
                cmdline: vec!["codex".to_string()],
                started_at: 0,
                run_time_seconds: 0,
                path: None,
                env: BTreeMap::new(),
            },
        }
    }
}
