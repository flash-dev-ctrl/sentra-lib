use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let Some(user_home) = probe.target_user_home.as_deref() else {
        return false;
    };
    common_cli_install_paths(
        binary_name,
        &CliPathContext {
            user_home,
            include_current_user_env: probe_targets_current_user(probe),
            platform,
            env,
        },
    )
    .iter()
    .any(|path| (probe.path_is_file)(path))
}

pub(crate) fn any_existing_file_with(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    paths.iter().any(|path| (probe.path_is_file)(path))
}

pub(crate) fn any_existing_dir_with(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    paths.iter().any(|path| (probe.path_is_dir)(path))
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
    let output = if cfg!(windows) {
        Command::new("where").arg(binary).output()
    } else {
        Command::new("sh")
            .args(["-c", "command -v \"$1\"", "sentra"])
            .arg(binary)
            .output()
    };
    let output = output.ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
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

pub(crate) fn binary_paths(dir: impl Into<PathBuf>, binary: &str) -> Vec<PathBuf> {
    binary_paths_for_platform(dir.into(), binary, HostPlatform::current())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostPlatform {
    Unix,
    Windows,
}

impl HostPlatform {
    fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
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

struct CliPathContext<'a> {
    user_home: &'a Path,
    include_current_user_env: bool,
    platform: HostPlatform,
    env: &'a CliPathEnv,
}

#[derive(Debug, Default)]
struct CliPathEnv {
    homebrew_prefix: Option<PathBuf>,
    pnpm_home: Option<PathBuf>,
    npm_config_prefix: Option<PathBuf>,
    volta_home: Option<PathBuf>,
    bun_install: Option<PathBuf>,
    cargo_home: Option<PathBuf>,
    program_files: Option<PathBuf>,
    program_files_x86: Option<PathBuf>,
    program_data: Option<PathBuf>,
    windows_dir: Option<PathBuf>,
}

impl CliPathEnv {
    fn current() -> Self {
        Self {
            homebrew_prefix: env_path("HOMEBREW_PREFIX"),
            pnpm_home: env_path("PNPM_HOME"),
            npm_config_prefix: env_path("NPM_CONFIG_PREFIX"),
            volta_home: env_path("VOLTA_HOME"),
            bun_install: env_path("BUN_INSTALL"),
            cargo_home: env_path("CARGO_HOME"),
            program_files: env_path("ProgramFiles"),
            program_files_x86: env_path("ProgramFiles(x86)"),
            program_data: env_path("ProgramData"),
            windows_dir: env_path("WINDIR"),
        }
    }

    fn current_user_binary_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(prefix) = &self.homebrew_prefix {
            dirs.push(prefix.join("bin"));
        }
        if let Some(home) = &self.pnpm_home {
            dirs.push(home.clone());
        }
        if let Some(prefix) = &self.npm_config_prefix {
            dirs.push(prefix.clone());
            dirs.push(prefix.join("bin"));
        }
        if let Some(home) = &self.volta_home {
            dirs.push(home.join("bin"));
        }
        if let Some(home) = &self.bun_install {
            dirs.push(home.join("bin"));
        }
        if let Some(home) = &self.cargo_home {
            dirs.push(home.join("bin"));
        }
        dirs
    }
}

fn common_cli_install_paths(binary: &str, context: &CliPathContext<'_>) -> Vec<PathBuf> {
    let user_home = context.user_home;
    let mut dirs = vec![
        user_home.join(".local").join("bin"),
        user_home.join("Library").join("pnpm"),
        user_home.join(".local").join("share").join("pnpm"),
        user_home.join(".bun").join("bin"),
        user_home.join(".cargo").join("bin"),
        user_home.join(".volta").join("bin"),
        user_home.join(".npm-global").join("bin"),
        user_home.join(".asdf").join("shims"),
        user_home
            .join(".local")
            .join("share")
            .join("mise")
            .join("shims"),
        user_home.join("AppData").join("Roaming").join("npm"),
        user_home.join("AppData").join("Local").join("pnpm"),
    ];
    dirs.extend(version_manager_binary_dirs(user_home));
    dirs.extend(global_binary_dirs(context.platform, context.env));
    if context.include_current_user_env {
        dirs.extend(context.env.current_user_binary_dirs());
    }
    dirs.sort();
    dirs.dedup();
    dirs.into_iter()
        .flat_map(|dir| binary_paths_for_platform(dir, binary, context.platform))
        .collect()
}

fn version_manager_binary_dirs(user_home: &Path) -> Vec<PathBuf> {
    let layouts = [
        (
            user_home.join(".nvm").join("versions").join("node"),
            vec!["bin"],
        ),
        (
            user_home.join(".fnm").join("node-versions"),
            vec!["installation", "bin"],
        ),
        (
            user_home.join(".asdf").join("installs").join("nodejs"),
            vec!["bin"],
        ),
        (
            user_home
                .join(".local")
                .join("share")
                .join("mise")
                .join("installs")
                .join("node"),
            vec!["bin"],
        ),
    ];
    let mut dirs = Vec::new();
    for (versions_dir, suffix) in layouts {
        for entry in fs::read_dir(versions_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let version_dir = entry.path();
            if !version_dir.is_dir() {
                continue;
            }
            dirs.push(
                suffix
                    .iter()
                    .fold(version_dir, |path, segment| path.join(segment)),
            );
        }
    }
    dirs
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
    fn codex_probe_accepts_apple_silicon_homebrew_path_without_path_lookup() {
        let probe = scoped_probe(
            "/Users/me",
            "/Users/me",
            command_path_never_resolves,
            only_homebrew_codex_path,
        );

        assert!(command_exists_with_context(
            "codex",
            &probe,
            HostPlatform::Unix,
            &CliPathEnv::default(),
        ));
    }

    #[test]
    fn common_cli_paths_cover_global_and_user_package_managers() {
        let env = CliPathEnv {
            homebrew_prefix: Some(PathBuf::from("/custom/homebrew")),
            pnpm_home: Some(PathBuf::from("/custom/pnpm")),
            npm_config_prefix: Some(PathBuf::from("/custom/npm")),
            volta_home: Some(PathBuf::from("/custom/volta")),
            bun_install: Some(PathBuf::from("/custom/bun")),
            cargo_home: Some(PathBuf::from("/custom/cargo")),
            ..CliPathEnv::default()
        };
        let paths = common_cli_install_paths(
            "codex",
            &CliPathContext {
                user_home: Path::new("/Users/me"),
                include_current_user_env: true,
                platform: HostPlatform::Unix,
                env: &env,
            },
        );

        for expected in [
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
            "/home/linuxbrew/.linuxbrew/bin/codex",
            "/Users/me/Library/pnpm/codex",
            "/Users/me/.local/share/pnpm/codex",
            "/Users/me/.bun/bin/codex",
            "/Users/me/.cargo/bin/codex",
            "/Users/me/.volta/bin/codex",
            "/custom/homebrew/bin/codex",
            "/custom/pnpm/codex",
            "/custom/npm/bin/codex",
            "/custom/volta/bin/codex",
            "/custom/bun/bin/codex",
            "/custom/cargo/bin/codex",
        ] {
            assert!(paths.contains(&PathBuf::from(expected)), "{expected}");
        }

        let other_user_paths = common_cli_install_paths(
            "codex",
            &CliPathContext {
                user_home: Path::new("/Users/other"),
                include_current_user_env: false,
                platform: HostPlatform::Unix,
                env: &env,
            },
        );
        assert!(!other_user_paths.contains(&PathBuf::from("/custom/pnpm/codex")));
        assert!(!other_user_paths.contains(&PathBuf::from("/custom/volta/bin/codex")));
    }

    #[test]
    fn common_cli_paths_cover_target_user_node_version_managers() {
        let user_home = tempfile::tempdir().unwrap();
        let binary_dirs = [
            user_home
                .path()
                .join(".nvm")
                .join("versions")
                .join("node")
                .join("v22.0.0")
                .join("bin"),
            user_home
                .path()
                .join(".fnm")
                .join("node-versions")
                .join("v22.0.0")
                .join("installation")
                .join("bin"),
            user_home
                .path()
                .join(".asdf")
                .join("installs")
                .join("nodejs")
                .join("22.0.0")
                .join("bin"),
            user_home
                .path()
                .join(".local")
                .join("share")
                .join("mise")
                .join("installs")
                .join("node")
                .join("22.0.0")
                .join("bin"),
        ];
        for dir in &binary_dirs {
            std::fs::create_dir_all(dir).unwrap();
        }

        let env = CliPathEnv::default();
        let paths = common_cli_install_paths(
            "codex",
            &CliPathContext {
                user_home: user_home.path(),
                include_current_user_env: false,
                platform: HostPlatform::Unix,
                env: &env,
            },
        );
        for dir in binary_dirs {
            assert!(paths.contains(&dir.join("codex")), "{}", dir.display());
        }
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
