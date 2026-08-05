use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct InstallStatusProbe {
    command_exists: fn(&str) -> bool,
    command_path: fn(&str) -> Option<PathBuf>,
    path_is_file: fn(&Path) -> bool,
    path_is_dir: fn(&Path) -> bool,
    windows_product_installed: fn(&[&str], &[&str]) -> bool,
    target_user_home: Option<PathBuf>,
    current_user_home: Option<PathBuf>,
}

impl InstallStatusProbe {
    pub(crate) fn real(user_home: impl Into<PathBuf>) -> Self {
        Self {
            command_exists: command_never_exists,
            command_path,
            path_is_file,
            path_is_dir,
            windows_product_installed,
            target_user_home: Some(user_home.into()),
            current_user_home: home::home_dir(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(
        command_exists: fn(&str) -> bool,
        path_is_file: fn(&Path) -> bool,
        path_is_dir: fn(&Path) -> bool,
    ) -> Self {
        Self {
            command_exists,
            command_path: command_path_never_resolves,
            path_is_file,
            path_is_dir,
            windows_product_installed: |_, _| false,
            target_user_home: None,
            current_user_home: None,
        }
    }

    pub(crate) fn product_installed(&self, display_names: &[&str], publishers: &[&str]) -> bool {
        (self.windows_product_installed)(display_names, publishers)
    }
}

pub(crate) fn is_named_cli_agent_installed_with(
    agent_name: &str,
    agent_home: &Path,
    probe: &InstallStatusProbe,
) -> bool {
    any_command_exists_with(&[agent_name], probe)
        || any_existing_file_with(named_cli_install_paths(agent_name, agent_home), probe)
}

pub(crate) fn is_ide_extension_installed(agent_home: &Path, extension_id: &str) -> bool {
    // ponytail: default one-level indexes cover VS Code forks; add explicit roots when
    // custom --extensions-dir support is required.
    fs::read_dir(hidden_home_parent(agent_home))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("extensions").join("extensions.json"))
        .any(|path| extension_index_contains(&path, extension_id))
}

fn extension_index_contains(path: &Path, extension_id: &str) -> bool {
    let Ok(Some(index)) = crate::utils::read_json_file(path) else {
        return false;
    };
    index.as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            entry
                .pointer("/identifier/id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| id.eq_ignore_ascii_case(extension_id))
        })
    })
}

pub(crate) fn any_command_exists_with(binary_names: &[&str], probe: &InstallStatusProbe) -> bool {
    let env = CliPathEnv::current();
    binary_names.iter().any(|binary_name| {
        command_exists_with_context(binary_name, probe, HostPlatform::current(), &env)
    })
}

fn command_exists_with_context(
    binary_name: &str,
    probe: &InstallStatusProbe,
    platform: HostPlatform,
    env: &CliPathEnv,
) -> bool {
    if (probe.command_exists)(binary_name) {
        return true;
    }
    if let Some(path) = (probe.command_path)(binary_name)
        && resolved_command_is_in_scope(&path, probe, platform, env)
        && (probe.path_is_file)(&path)
    {
        return true;
    }
    false
}

pub(crate) fn any_existing_file_with(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    let env = CliPathEnv::current();
    let platform = HostPlatform::current();
    paths.iter().any(|path| {
        known_install_path_is_in_scope(path, probe, platform, &env) && (probe.path_is_file)(path)
    })
}

pub(crate) fn any_existing_dir_with(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    let env = CliPathEnv::current();
    let platform = HostPlatform::current();
    paths.iter().any(|path| {
        known_install_path_is_in_scope(path, probe, platform, &env) && (probe.path_is_dir)(path)
    })
}

pub(crate) fn windows_product_installed(display_names: &[&str], publishers: &[&str]) -> bool {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        };

        const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
        for (hive, view) in [
            (HKEY_CURRENT_USER, KEY_WOW64_64KEY),
            (HKEY_CURRENT_USER, KEY_WOW64_32KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        ] {
            let Ok(uninstall) =
                RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, KEY_READ | view)
            else {
                continue;
            };
            for key_name in uninstall.enum_keys().filter_map(Result::ok) {
                let Ok(product) = uninstall.open_subkey_with_flags(key_name, KEY_READ | view)
                else {
                    continue;
                };
                let Ok(display_name) = product.get_value::<String, _>("DisplayName") else {
                    continue;
                };
                let Ok(publisher) = product.get_value::<String, _>("Publisher") else {
                    continue;
                };
                if windows_product_matches(&display_name, &publisher, display_names, publishers) {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = (display_names, publishers);
        false
    }
}

fn windows_product_matches(
    display_name: &str,
    publisher: &str,
    display_names: &[&str],
    publishers: &[&str],
) -> bool {
    display_names
        .iter()
        .any(|expected| product_name_matches(display_name, expected))
        && publishers.iter().any(|expected| {
            publisher
                .to_ascii_lowercase()
                .contains(&expected.to_ascii_lowercase())
        })
}

fn product_name_matches(actual: &str, expected: &str) -> bool {
    let actual = actual.trim().to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    if actual == expected {
        return true;
    }
    let Some(suffix) = actual.strip_prefix(&expected) else {
        return false;
    };
    let suffix = suffix.trim_start();
    suffix.starts_with('(')
        || (!suffix.is_empty()
            && suffix
                .chars()
                .all(|char| char.is_ascii_digit() || matches!(char, '.' | '-' | ' ')))
}

fn command_path(binary: &str) -> Option<PathBuf> {
    path_command_candidates(binary)
        .into_iter()
        .find(|path| path.is_file())
}

fn command_never_exists(_: &str) -> bool {
    false
}

#[cfg(test)]
fn command_path_never_resolves(_: &str) -> Option<PathBuf> {
    None
}

fn path_is_file(path: &Path) -> bool {
    path.is_file()
}

fn path_is_dir(path: &Path) -> bool {
    path.is_dir()
}

fn path_command_candidates(binary: &str) -> Vec<PathBuf> {
    path_command_candidates_from_values(
        binary,
        effective_path_values(HostPlatform::current()),
        HostPlatform::current(),
    )
}

fn path_command_candidates_from_values(
    binary: &str,
    path_values: Vec<String>,
    platform: HostPlatform,
) -> Vec<PathBuf> {
    if binary.trim().is_empty()
        || binary.contains(std::path::MAIN_SEPARATOR)
        || binary.contains('/')
        || binary.contains('\\')
    {
        return Vec::new();
    }
    path_values
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .flat_map(|dir| platform_command_candidates(&dir, binary, platform))
        .collect()
}

fn effective_path_values(platform: HostPlatform) -> Vec<String> {
    let process_path = env_string("PATH").or_else(|| env_string("Path"));
    effective_path_values_from(
        process_path,
        declared_path_values(platform),
        shell_initialized_path_values(platform),
        platform,
    )
}

fn effective_path_values_from(
    process_path: Option<String>,
    declared_paths: Vec<String>,
    shell_paths: Vec<String>,
    platform: HostPlatform,
) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(path) = process_path {
        push_path_value_if_missing(&mut values, path);
    }
    for path in declared_paths {
        push_path_value_if_missing(&mut values, path);
    }
    for path in shell_paths {
        push_path_value_if_missing(&mut values, path);
    }
    let _ = platform;
    values
}

fn declared_path_values(platform: HostPlatform) -> Vec<String> {
    match platform {
        HostPlatform::Windows => windows_registry_path_values(),
        HostPlatform::MacOS => macos_declared_path_values(),
        HostPlatform::Unix => linux_declared_path_values(),
    }
}

fn push_path_value_if_missing(values: &mut Vec<String>, value: String) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if !values
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(value))
    {
        values.push(value.to_string());
    }
}

fn env_string(name: &str) -> Option<String> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string_lossy().to_string())
}

static SHELL_ENV_CACHE: OnceLock<Vec<(String, String)>> = OnceLock::new();

fn shell_initialized_path_values(platform: HostPlatform) -> Vec<String> {
    let env = SHELL_ENV_CACHE.get_or_init(|| collect_shell_initialized_env(platform));
    ["PATH", "Path"]
        .into_iter()
        .filter_map(|key| shell_env_value(env, key))
        .collect()
}

fn shell_env_value(env: &[(String, String)], key: &str) -> Option<String> {
    env.iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
        .filter(|value| !value.trim().is_empty())
}

fn collect_shell_initialized_env(platform: HostPlatform) -> Vec<(String, String)> {
    shell_env_commands(platform)
        .into_iter()
        .find_map(run_env_command)
        .unwrap_or_default()
}

fn shell_env_commands(platform: HostPlatform) -> Vec<EnvCommand> {
    match platform {
        HostPlatform::Windows => {
            let cmd = std::env::var_os("ComSpec")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("cmd.exe"));
            vec![EnvCommand {
                program: cmd,
                args: vec!["/c".to_string(), "set".to_string()],
            }]
        }
        HostPlatform::MacOS | HostPlatform::Unix => unix_shell_env_commands(),
    }
}

fn unix_shell_env_commands() -> Vec<EnvCommand> {
    let mut commands = Vec::new();
    if let Some(shell) = std::env::var_os("SHELL").filter(|value| !value.is_empty()) {
        commands.push(unix_shell_env_command(PathBuf::from(shell)));
    }
    for shell in ["bash", "zsh", "sh"] {
        commands.push(unix_shell_env_command(PathBuf::from(shell)));
    }
    commands
}

fn unix_shell_env_command(program: PathBuf) -> EnvCommand {
    let shell_name = program
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let flag = if shell_name.contains("bash") || shell_name.contains("zsh") {
        "-ic"
    } else {
        "-c"
    };
    EnvCommand {
        program,
        args: vec![flag.to_string(), "env".to_string()],
    }
}

struct EnvCommand {
    program: PathBuf,
    args: Vec<String>,
}

const SHELL_ENV_TIMEOUT: Duration = Duration::from_millis(1200);

fn run_env_command(command: EnvCommand) -> Option<Vec<(String, String)>> {
    let mut child = Command::new(command.program)
        .args(command.args)
        .env("SENTRA_ENV_SNAPSHOT", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started_at = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return None,
            Ok(None) if started_at.elapsed() >= SHELL_ENV_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let env = parse_env_output(&stdout);
    (!env.is_empty()).then_some(env)
}

fn parse_env_output(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim().is_empty() {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

#[cfg(windows)]
fn windows_registry_path_values() -> Vec<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    const USER_ENV: &str = "Environment";
    const SYSTEM_ENV: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

    [
        (HKEY_CURRENT_USER, USER_ENV),
        (HKEY_LOCAL_MACHINE, SYSTEM_ENV),
    ]
    .into_iter()
    .filter_map(|(hive, key)| {
        RegKey::predef(hive)
            .open_subkey_with_flags(key, KEY_READ)
            .ok()
            .and_then(|env| env.get_value::<String, _>("Path").ok())
    })
    .map(|value| expand_windows_env_vars(&value))
    .filter(|value| !value.trim().is_empty())
    .collect()
}

#[cfg(not(windows))]
fn windows_registry_path_values() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn macos_declared_path_values() -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(content) = fs::read_to_string("/etc/paths") {
        paths.extend(
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_string),
        );
    }
    if let Ok(entries) = fs::read_dir("/etc/paths.d") {
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        files.sort();
        for file in files {
            if let Ok(content) = fs::read_to_string(file) {
                paths.extend(
                    content
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .map(str::to_string),
                );
            }
        }
    }
    if paths.is_empty() {
        return Vec::new();
    }
    vec![std::env::join_paths(paths).map_or_else(
        |_| String::new(),
        |value| value.to_string_lossy().to_string(),
    )]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .collect()
}

#[cfg(not(target_os = "macos"))]
fn macos_declared_path_values() -> Vec<String> {
    Vec::new()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_declared_path_values() -> Vec<String> {
    fs::read_to_string("/etc/environment")
        .ok()
        .and_then(|content| parse_environment_assignment(&content, "PATH"))
        .into_iter()
        .collect()
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn linux_declared_path_values() -> Vec<String> {
    Vec::new()
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn parse_environment_assignment(content: &str, key: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| unquote_environment_value(value.trim()))
    })
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn unquote_environment_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        return value[1..value.len() - 1].to_string();
    }
    value.to_string()
}

fn expand_windows_env_vars(value: &str) -> String {
    expand_windows_env_vars_with(value, windows_env_value)
}

fn windows_env_value(name: &str) -> Option<String> {
    env_string(name)
        .or_else(|| windows_registry_env_value(name))
        .or_else(|| windows_known_env_value(name))
}

#[cfg(windows)]
fn windows_registry_env_value(name: &str) -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    const USER_ENV: &str = "Environment";
    const SYSTEM_ENV: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

    [
        (HKEY_CURRENT_USER, USER_ENV),
        (HKEY_LOCAL_MACHINE, SYSTEM_ENV),
    ]
    .into_iter()
    .filter_map(|(hive, key)| {
        RegKey::predef(hive)
            .open_subkey_with_flags(key, KEY_READ)
            .ok()
            .and_then(|env| env.get_value::<String, _>(name).ok())
    })
    .find(|value| !value.trim().is_empty())
}

#[cfg(not(windows))]
fn windows_registry_env_value(_: &str) -> Option<String> {
    None
}

fn windows_known_env_value(name: &str) -> Option<String> {
    if name.eq_ignore_ascii_case("USERPROFILE") {
        return home::home_dir().map(|path| path.to_string_lossy().to_string());
    }
    let user_home = home::home_dir()?;
    if name.eq_ignore_ascii_case("LOCALAPPDATA") {
        return Some(
            user_home
                .join("AppData")
                .join("Local")
                .to_string_lossy()
                .to_string(),
        );
    }
    if name.eq_ignore_ascii_case("APPDATA") {
        return Some(
            user_home
                .join("AppData")
                .join("Roaming")
                .to_string_lossy()
                .to_string(),
        );
    }
    None
}

fn expand_windows_env_vars_with(
    value: &str,
    mut resolve: impl FnMut(&str) -> Option<String>,
) -> String {
    let mut expanded = String::new();
    let mut rest = value;
    loop {
        let Some(start) = rest.find('%') else {
            expanded.push_str(rest);
            break;
        };
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            expanded.push('%');
            expanded.push_str(after_start);
            break;
        };
        let name = &after_start[..end];
        if name.is_empty() {
            expanded.push_str("%%");
        } else if let Some(value) = resolve(name) {
            expanded.push_str(&value);
        } else {
            expanded.push('%');
            expanded.push_str(name);
            expanded.push('%');
        }
        rest = &after_start[end + 1..];
    }
    expanded
}

fn platform_command_candidates(dir: &Path, binary: &str, platform: HostPlatform) -> Vec<PathBuf> {
    if platform != HostPlatform::Windows || Path::new(binary).extension().is_some() {
        return vec![dir.join(binary)];
    }
    windows_path_extensions()
        .into_iter()
        .map(|extension| dir.join(format!("{binary}{extension}")))
        .collect()
}

fn windows_path_extensions() -> Vec<String> {
    std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| {
                    if value.starts_with('.') {
                        value.to_string()
                    } else {
                        format!(".{value}")
                    }
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            [".COM", ".EXE", ".BAT", ".CMD"]
                .into_iter()
                .map(str::to_string)
                .collect()
        })
}

pub(crate) fn binary_paths(dir: impl Into<PathBuf>, binary: &str) -> Vec<PathBuf> {
    binary_paths_for_platform(dir.into(), binary, HostPlatform::current())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostPlatform {
    Unix,
    MacOS,
    Windows,
}

impl HostPlatform {
    fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else {
            Self::Unix
        }
    }
}

fn binary_paths_for_platform(dir: PathBuf, binary: &str, platform: HostPlatform) -> Vec<PathBuf> {
    if platform == HostPlatform::Windows {
        ["exe", "cmd", "bat"]
            .into_iter()
            .map(|ext| dir.join(format!("{binary}.{ext}")))
            .collect()
    } else {
        vec![dir.join(binary)]
    }
}

#[derive(Debug, Default)]
struct CliPathEnv {
    program_files: Option<PathBuf>,
    program_files_x86: Option<PathBuf>,
    program_data: Option<PathBuf>,
    windows_dir: Option<PathBuf>,
}

impl CliPathEnv {
    fn current() -> Self {
        Self {
            program_files: env_path("ProgramFiles"),
            program_files_x86: env_path("ProgramFiles(x86)"),
            program_data: env_path("ProgramData"),
            windows_dir: env_path("WINDIR"),
        }
    }
}

fn global_binary_dirs(platform: HostPlatform, env: &CliPathEnv) -> Vec<PathBuf> {
    if platform == HostPlatform::Windows {
        let mut dirs = Vec::new();
        for root in [
            env.program_files.as_ref(),
            env.program_files_x86.as_ref(),
            env.program_data.as_ref(),
            env.windows_dir.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            dirs.push(root.clone());
            dirs.push(root.join("bin"));
        }
        return dirs;
    }
    unix_global_binary_dirs()
}

fn unix_global_binary_dirs() -> Vec<PathBuf> {
    [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/home/linuxbrew/.linuxbrew/bin",
        "/opt/local/bin",
        "/usr/bin",
        "/bin",
        "/snap/bin",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn probe_targets_current_user(probe: &InstallStatusProbe) -> bool {
    match (
        probe.target_user_home.as_deref(),
        probe.current_user_home.as_deref(),
    ) {
        (Some(target), Some(current)) => same_location(target, current),
        _ => false,
    }
}

fn resolved_command_is_in_scope(
    command_path: &Path,
    probe: &InstallStatusProbe,
    platform: HostPlatform,
    env: &CliPathEnv,
) -> bool {
    let Some(target_home) = probe.target_user_home.as_deref() else {
        return true;
    };
    if probe_targets_current_user(probe) || path_is_within(command_path, target_home) {
        return true;
    }
    global_binary_dirs(platform, env)
        .iter()
        .any(|dir| path_is_within(command_path, dir))
}

fn known_install_path_is_in_scope(
    path: &Path,
    probe: &InstallStatusProbe,
    platform: HostPlatform,
    env: &CliPathEnv,
) -> bool {
    let Some(target_home) = probe.target_user_home.as_deref() else {
        return true;
    };
    path_is_within(path, target_home)
        || global_binary_dirs(platform, env)
            .iter()
            .any(|dir| path_is_within(path, dir))
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    match (path.canonicalize(), root.canonicalize()) {
        (Ok(path), Ok(root)) => path.starts_with(root),
        _ => path.starts_with(root),
    }
}

fn same_location(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn named_cli_install_paths(agent_name: &str, agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(agent_home.join("bin"), agent_name);
    paths.extend(binary_paths(
        user_home.join(".local").join("bin"),
        agent_name,
    ));
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        paths.extend(binary_paths(
            local_app_data.join(agent_name).join("cli").join("bin"),
            agent_name,
        ));
    }
    paths
}

pub(crate) fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn hidden_home_parent(agent_home: &Path) -> PathBuf {
    agent_home
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| agent_home.to_path_buf())
}

pub(crate) fn user_home_for_agent_home(
    agent_home: &Path,
    default_home_segments: &[&str],
) -> PathBuf {
    user_home_for_agent_home_with_current(
        agent_home,
        default_home_segments,
        home::home_dir().as_deref(),
    )
}

fn user_home_for_agent_home_with_current(
    agent_home: &Path,
    default_home_segments: &[&str],
    current_user_home: Option<&Path>,
) -> PathBuf {
    if let Some(current_user_home) = current_user_home
        && path_is_within(agent_home, current_user_home)
    {
        return current_user_home.to_path_buf();
    }
    let mut user_home = agent_home;
    for expected in default_home_segments.iter().rev() {
        let matches = user_home
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(expected));
        if !matches {
            return current_user_home
                .map(Path::to_path_buf)
                .unwrap_or_else(|| hidden_home_parent(agent_home));
        }
        let Some(parent) = user_home.parent() else {
            return current_user_home
                .map(Path::to_path_buf)
                .unwrap_or_else(|| hidden_home_parent(agent_home));
        };
        user_home = parent;
    }
    user_home.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_probe_requires_path_resolution_even_for_common_install_dirs() {
        let probe = scoped_probe(
            "/Users/me",
            "/Users/me",
            command_path_never_resolves,
            only_homebrew_codex_path,
        );

        assert!(!command_exists_with_context(
            "codex",
            &probe,
            HostPlatform::Unix,
            &CliPathEnv::default(),
        ));
    }

    #[test]
    fn resolved_command_path_is_scoped_to_target_user_or_global_prefix() {
        let current_user_probe = scoped_probe(
            "/Users/other",
            "/Users/current",
            current_user_codex_path,
            only_current_user_codex_path,
        );
        assert!(!command_exists_with_context(
            "codex",
            &current_user_probe,
            HostPlatform::Unix,
            &CliPathEnv::default(),
        ));

        let target_user_probe = scoped_probe(
            "/Users/other",
            "/Users/current",
            target_user_codex_path,
            only_target_user_codex_path,
        );
        assert!(command_exists_with_context(
            "codex",
            &target_user_probe,
            HostPlatform::Unix,
            &CliPathEnv::default(),
        ));

        let global_probe = scoped_probe(
            "/Users/other",
            "/Users/current",
            homebrew_codex_path,
            only_homebrew_codex_path,
        );
        assert!(command_exists_with_context(
            "codex",
            &global_probe,
            HostPlatform::Unix,
            &CliPathEnv::default(),
        ));
    }

    #[test]
    fn known_install_paths_are_scoped_to_target_user_or_global_prefix() {
        let probe = scoped_probe(
            "/Users/fixture",
            "/Users/fixture",
            command_path_never_resolves,
            only_current_user_codex_path,
        );

        assert!(!any_existing_file_with(
            vec![PathBuf::from("/Users/current/.local/bin/codex")],
            &probe,
        ));
        assert!(any_existing_file_with(
            vec![PathBuf::from("/Users/fixture/.local/bin/codex")],
            &scoped_probe(
                "/Users/fixture",
                "/Users/fixture",
                command_path_never_resolves,
                only_fixture_user_codex_path,
            ),
        ));
        let global_probe = scoped_probe(
            "/Users/fixture",
            "/Users/fixture",
            command_path_never_resolves,
            only_homebrew_codex_path,
        );
        assert!(known_install_path_is_in_scope(
            Path::new("/opt/homebrew/bin/codex"),
            &global_probe,
            HostPlatform::Unix,
            &CliPathEnv::default(),
        ));
    }

    #[test]
    fn user_home_resolution_handles_default_and_custom_agent_homes() {
        let current_home = Path::new("/Users/current");

        assert_eq!(
            user_home_for_agent_home_with_current(
                Path::new("/Users/target/.codex"),
                &[".codex"],
                Some(current_home),
            ),
            PathBuf::from("/Users/target")
        );
        assert_eq!(
            user_home_for_agent_home_with_current(
                Path::new("/Users/target/.config/coderv2"),
                &[".config", "coderv2"],
                Some(current_home),
            ),
            PathBuf::from("/Users/target")
        );
        assert_eq!(
            user_home_for_agent_home_with_current(
                Path::new("/Volumes/config/codex"),
                &[".codex"],
                Some(current_home),
            ),
            current_home
        );
        assert_eq!(
            user_home_for_agent_home_with_current(
                Path::new("/Users/current/project/.codex"),
                &[".codex"],
                Some(current_home),
            ),
            current_home
        );
    }

    #[test]
    fn windows_path_candidates_use_supplied_path_values() {
        let candidates = path_command_candidates_from_values(
            "codex",
            vec!["/tools/bin".to_string()],
            HostPlatform::Windows,
        );
        let candidates = candidates
            .iter()
            .map(|path| {
                path.to_string_lossy()
                    .replace('\\', "/")
                    .to_ascii_lowercase()
            })
            .collect::<Vec<_>>();

        assert!(
            candidates
                .iter()
                .any(|path| path.ends_with("/tools/bin/codex.cmd"))
        );
        assert!(
            candidates
                .iter()
                .any(|path| path.ends_with("/tools/bin/codex.exe"))
        );
    }

    #[test]
    fn windows_effective_path_merges_process_and_registry_paths() {
        let values = effective_path_values_from(
            Some(r"C:\Windows\System32".to_string()),
            vec![
                r"C:\Users\me\AppData\Roaming\npm".to_string(),
                r"c:\windows\system32".to_string(),
            ],
            Vec::new(),
            HostPlatform::Windows,
        );

        assert_eq!(
            values,
            vec![
                r"C:\Windows\System32".to_string(),
                r"C:\Users\me\AppData\Roaming\npm".to_string(),
            ]
        );
    }

    #[test]
    fn unix_effective_path_merges_process_and_declared_paths() {
        let values = effective_path_values_from(
            Some("/usr/bin:/bin".to_string()),
            vec!["/opt/homebrew/bin:/usr/local/bin".to_string()],
            Vec::new(),
            HostPlatform::Unix,
        );

        assert_eq!(
            values,
            vec![
                "/usr/bin:/bin".to_string(),
                "/opt/homebrew/bin:/usr/local/bin".to_string(),
            ]
        );
    }

    #[test]
    fn effective_path_merges_shell_initialized_path_values() {
        let values = effective_path_values_from(
            Some("/usr/bin:/bin".to_string()),
            vec!["/opt/homebrew/bin:/usr/local/bin".to_string()],
            vec![
                "/home/me/.local/bin:/usr/bin".to_string(),
                "/usr/bin:/bin".to_string(),
            ],
            HostPlatform::Unix,
        );

        assert_eq!(
            values,
            vec![
                "/usr/bin:/bin".to_string(),
                "/opt/homebrew/bin:/usr/local/bin".to_string(),
                "/home/me/.local/bin:/usr/bin".to_string(),
            ]
        );
    }

    #[test]
    fn env_output_parser_reads_key_value_lines() {
        let env = parse_env_output(
            r#"
IGNORED
PATH=/home/me/.local/bin:/usr/bin
SHELL=/bin/bash
"#,
        );

        assert_eq!(
            shell_env_value(&env, "path").as_deref(),
            Some("/home/me/.local/bin:/usr/bin")
        );
        assert_eq!(shell_env_value(&env, "SHELL").as_deref(), Some("/bin/bash"));
    }

    #[test]
    fn environment_assignment_parser_reads_quoted_path() {
        let path = parse_environment_assignment(
            r#"
# comment
LANG=en_US.UTF-8
PATH="/usr/local/bin:/usr/bin:/bin"
"#,
            "PATH",
        );

        assert_eq!(path.as_deref(), Some("/usr/local/bin:/usr/bin:/bin"));
    }

    #[test]
    fn windows_env_expansion_preserves_unknown_variables() {
        let expanded = expand_windows_env_vars_with(
            r"%USERPROFILE%\AppData\Roaming\npm;%UNKNOWN%\bin",
            |name| {
                if name.eq_ignore_ascii_case("USERPROFILE") {
                    Some(r"C:\Users\me".to_string())
                } else {
                    None
                }
            },
        );

        assert_eq!(expanded, r"C:\Users\me\AppData\Roaming\npm;%UNKNOWN%\bin");
    }

    #[test]
    fn named_cli_probe_requires_binary_or_install_path_not_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".devin");
        std::fs::create_dir_all(&agent_home).unwrap();
        let probe =
            InstallStatusProbe::test(command_never_exists, path_never_exists, path_never_exists);

        assert!(!is_named_cli_agent_installed_with(
            "devin",
            &agent_home,
            &probe
        ));
    }

    #[test]
    fn named_cli_probe_accepts_command_presence() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".devin");
        let probe = InstallStatusProbe::test(
            only_devin_command_exists,
            path_never_exists,
            path_never_exists,
        );

        assert!(is_named_cli_agent_installed_with(
            "devin",
            &agent_home,
            &probe
        ));
    }

    #[test]
    fn named_cli_probe_accepts_known_user_install_path() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".devin");
        let bin_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join(test_binary_name("devin")), "").unwrap();
        let probe = InstallStatusProbe::test(command_never_exists, path_is_file, path_never_exists);

        assert!(is_named_cli_agent_installed_with(
            "devin",
            &agent_home,
            &probe
        ));
    }

    #[test]
    fn ide_extension_probe_scans_any_vscode_family_index() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".codex");
        for (ide, extension_id) in [
            (".vscode", "openai.chatgpt-helper"),
            (".devin", "OPENAI.CHATGPT"),
            (".cursor", "openai.chatgpt"),
        ] {
            let extension_dir = dir.path().join(ide).join("extensions");
            std::fs::create_dir_all(&extension_dir).unwrap();
            std::fs::write(
                extension_dir.join("extensions.json"),
                serde_json::to_vec(&serde_json::json!([{
                    "identifier": { "id": extension_id }
                }]))
                .unwrap(),
            )
            .unwrap();
        }

        assert!(is_ide_extension_installed(&agent_home, "openai.chatgpt"));
        assert!(!is_ide_extension_installed(&agent_home, "openai.chat"));
    }

    #[test]
    fn ide_extension_probe_ignores_missing_and_malformed_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir.path().join(".claude");

        assert!(!is_ide_extension_installed(
            &agent_home,
            "anthropic.claude-code"
        ));

        let extension_dir = dir.path().join(".trae").join("extensions");
        std::fs::create_dir_all(&extension_dir).unwrap();
        std::fs::write(extension_dir.join("extensions.json"), "not-json").unwrap();

        assert!(!is_ide_extension_installed(
            &agent_home,
            "anthropic.claude-code"
        ));
    }

    #[test]
    fn windows_product_match_requires_product_and_publisher() {
        assert!(windows_product_matches(
            "Claude Code",
            "Anthropic PBC",
            &["Claude Code"],
            &["Anthropic"]
        ));
        assert!(windows_product_matches(
            "Antigravity CLI",
            "Google",
            &["Antigravity CLI"],
            &["Google"]
        ));
        assert!(windows_product_matches(
            "WorkBuddy 5.2.6",
            "Tencent Technology (Shenzhen) Company Limited",
            &["WorkBuddy"],
            &["Tencent Technology"]
        ));
        assert!(windows_product_matches(
            "Trae (User)",
            "SPRING (SG) PTE. LTD",
            &["Trae"],
            &["SPRING (SG)"]
        ));
        assert!(windows_product_matches(
            "Kiro",
            "Amazon Web Services",
            &["Kiro"],
            &["Amazon Web Services"]
        ));
        assert!(windows_product_matches(
            "Cursor (User)",
            "Anysphere, Inc.",
            &["Cursor"],
            &["Anysphere"]
        ));
        assert!(windows_product_matches(
            "Qoder (User)",
            "Alibaba Cloud",
            &["Qoder"],
            &["Alibaba"]
        ));
        assert!(!windows_product_matches(
            "TRAE SOLO (User)",
            "SPRING (SG) PTE. LTD",
            &["Trae"],
            &["SPRING (SG)"]
        ));
        assert!(!windows_product_matches(
            "WorkBuddy 5.2.6",
            "Unrelated Publisher",
            &["WorkBuddy"],
            &["Tencent Technology"]
        ));
    }

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn only_devin_command_exists(binary: &str) -> bool {
        binary == "devin"
    }

    fn scoped_probe(
        target_user_home: &str,
        current_user_home: &str,
        command_path: fn(&str) -> Option<PathBuf>,
        path_is_file: fn(&Path) -> bool,
    ) -> InstallStatusProbe {
        InstallStatusProbe {
            command_exists: command_never_exists,
            command_path,
            path_is_file,
            path_is_dir: path_never_exists,
            windows_product_installed: |_, _| false,
            target_user_home: Some(PathBuf::from(target_user_home)),
            current_user_home: Some(PathBuf::from(current_user_home)),
        }
    }

    fn current_user_codex_path(_: &str) -> Option<PathBuf> {
        Some(PathBuf::from("/Users/current/.volta/bin/codex"))
    }

    fn target_user_codex_path(_: &str) -> Option<PathBuf> {
        Some(PathBuf::from("/Users/other/.volta/bin/codex"))
    }

    fn homebrew_codex_path(_: &str) -> Option<PathBuf> {
        Some(PathBuf::from("/opt/homebrew/bin/codex"))
    }

    fn only_current_user_codex_path(path: &Path) -> bool {
        path == Path::new("/Users/current/.volta/bin/codex")
    }

    fn only_target_user_codex_path(path: &Path) -> bool {
        path == Path::new("/Users/other/.volta/bin/codex")
    }

    fn only_fixture_user_codex_path(path: &Path) -> bool {
        path == Path::new("/Users/fixture/.local/bin/codex")
    }

    fn only_homebrew_codex_path(path: &Path) -> bool {
        path == Path::new("/opt/homebrew/bin/codex")
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }

    fn test_binary_name(binary: &str) -> String {
        if cfg!(windows) {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        }
    }
}
