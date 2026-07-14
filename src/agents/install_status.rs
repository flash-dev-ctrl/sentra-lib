use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentInstallProbe {
    Codex,
    ClaudeCli,
    ClaudeApp,
    Hermes,
    OpenClaw,
    OpenCode,
}

pub(crate) fn is_agent_installed(agent: AgentInstallProbe, agent_home: &Path) -> bool {
    let probe = InstallStatusProbe {
        command_exists,
        path_is_file,
        path_is_dir,
    };
    is_agent_installed_with(agent, agent_home, &probe)
}

struct InstallStatusProbe {
    command_exists: fn(&str) -> bool,
    path_is_file: fn(&Path) -> bool,
    path_is_dir: fn(&Path) -> bool,
}

fn is_agent_installed_with(
    agent: AgentInstallProbe,
    agent_home: &Path,
    probe: &InstallStatusProbe,
) -> bool {
    match agent {
        AgentInstallProbe::Codex => {
            (probe.command_exists)("codex")
                || any_existing_file(codex_install_paths(agent_home), probe)
        }
        AgentInstallProbe::ClaudeCli => {
            (probe.command_exists)("claude")
                || any_existing_file(claude_cli_install_paths(agent_home), probe)
        }
        AgentInstallProbe::ClaudeApp => {
            any_existing_file(claude_app_install_paths(agent_home), probe)
                || any_existing_dir(claude_app_bundle_paths(agent_home), probe)
        }
        AgentInstallProbe::Hermes => {
            (probe.command_exists)("hermes")
                || (probe.command_exists)("hermes-agent")
                || any_existing_file(hermes_install_paths(agent_home), probe)
        }
        AgentInstallProbe::OpenClaw => {
            (probe.command_exists)("openclaw")
                || (probe.command_exists)("openclawcli")
                || any_existing_file(openclaw_install_paths(agent_home), probe)
        }
        AgentInstallProbe::OpenCode => {
            (probe.command_exists)("opencode")
                || any_existing_file(opencode_install_paths(agent_home), probe)
        }
    }
}

fn command_exists(binary: &str) -> bool {
    let output = if cfg!(windows) {
        Command::new("where").arg(binary).output()
    } else {
        Command::new("sh")
            .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "sentra"])
            .arg(binary)
            .output()
    };
    output
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn path_is_file(path: &Path) -> bool {
    path.is_file()
}

fn path_is_dir(path: &Path) -> bool {
    path.is_dir()
}

fn codex_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "codex");
    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        paths.extend(binary_paths(
            local_app_data
                .join("Programs")
                .join("OpenAI")
                .join("Codex")
                .join("bin"),
            "codex",
        ));
    }
    if let Some(install_dir) = env_path("CODEX_INSTALL_DIR") {
        paths.extend(binary_paths(install_dir, "codex"));
    }
    paths
}

fn claude_cli_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "claude");
    paths.extend(binary_paths(
        user_home.join(".local").join("share").join("claude"),
        "claude",
    ));
    paths
}

fn claude_app_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = claude_app_user_home(agent_home);
    let mut paths = binary_paths(agent_home, "Claude");
    paths.extend(binary_paths(agent_home.join("app"), "Claude"));
    paths.extend(binary_paths(
        user_home
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("Claude"),
        "Claude",
    ));
    paths
}

fn claude_app_bundle_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = claude_app_user_home(agent_home);
    vec![
        user_home.join("Applications").join("Claude.app"),
        PathBuf::from("/Applications/Claude.app"),
    ]
}

fn hermes_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "hermes");
    paths.extend(binary_paths(
        user_home.join(".local").join("bin"),
        "hermes-agent",
    ));
    paths.extend(binary_paths(agent_home.join("bin"), "hermes"));
    paths.extend(binary_paths(agent_home.join("bin"), "hermes-agent"));
    paths.extend(binary_paths("/usr/local/bin", "hermes"));
    paths.extend(binary_paths("/usr/local/bin", "hermes-agent"));
    paths
}

fn openclaw_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = hidden_home_parent(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "openclaw");
    paths.extend(binary_paths(
        user_home.join(".local").join("bin"),
        "openclawcli",
    ));
    paths.extend(binary_paths(agent_home.join("bin"), "openclaw"));
    paths.extend(binary_paths(agent_home.join("bin"), "openclawcli"));
    paths
}

fn opencode_install_paths(agent_home: &Path) -> Vec<PathBuf> {
    let user_home = opencode_user_home(agent_home);
    let mut paths = binary_paths(user_home.join(".local").join("bin"), "opencode");
    paths.extend(binary_paths(agent_home.join("bin"), "opencode"));
    paths
}

fn binary_paths(dir: impl Into<PathBuf>, binary: &str) -> Vec<PathBuf> {
    let dir = dir.into();
    if cfg!(windows) {
        ["exe", "cmd", "bat"]
            .into_iter()
            .map(|ext| dir.join(format!("{binary}.{ext}")))
            .collect()
    } else {
        vec![dir.join(binary)]
    }
}

fn any_existing_file(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    paths.iter().any(|path| (probe.path_is_file)(path))
}

fn any_existing_dir(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    paths.iter().any(|path| (probe.path_is_dir)(path))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn hidden_home_parent(agent_home: &Path) -> PathBuf {
    agent_home
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| agent_home.to_path_buf())
}

fn opencode_user_home(agent_home: &Path) -> PathBuf {
    if agent_home.file_name().and_then(|name| name.to_str()) == Some("opencode")
        && agent_home
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(".config")
    {
        return agent_home
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| agent_home.to_path_buf());
    }
    hidden_home_parent(agent_home)
}

fn claude_app_user_home(agent_home: &Path) -> PathBuf {
    let parts = agent_home
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for suffix in [
        &["AppData", "Local", "Claude"][..],
        &["AppData", "Local", "Claude-3p"][..],
        &["Library", "Application Support", "Claude"][..],
        &["Library", "Application Support", "Claude-3p"][..],
    ] {
        if path_parts_end_with(&parts, suffix) {
            let ancestor_count = suffix.len();
            let mut home = agent_home;
            for _ in 0..ancestor_count {
                home = home.parent().unwrap_or(home);
            }
            return home.to_path_buf();
        }
    }
    hidden_home_parent(agent_home)
}

fn path_parts_end_with(parts: &[String], suffix: &[&str]) -> bool {
    parts.len() >= suffix.len()
        && parts[parts.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_probe_requires_binary_or_install_path_not_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let opencode_home = dir.path().join(".config").join("opencode");
        std::fs::create_dir_all(&opencode_home).unwrap();
        let probe = test_probe(command_never_exists, path_never_exists, path_never_exists);

        assert!(!is_agent_installed_with(
            AgentInstallProbe::OpenCode,
            &opencode_home,
            &probe
        ));
    }

    #[test]
    fn cli_probe_accepts_command_presence() {
        let dir = tempfile::tempdir().unwrap();
        let codex_home = dir.path().join(".codex");
        let probe = test_probe(
            only_codex_command_exists,
            path_never_exists,
            path_never_exists,
        );

        assert!(is_agent_installed_with(
            AgentInstallProbe::Codex,
            &codex_home,
            &probe
        ));
    }

    #[test]
    fn cli_probe_accepts_known_user_install_path() {
        let dir = tempfile::tempdir().unwrap();
        let opencode_home = dir.path().join(".config").join("opencode");
        let bin_dir = dir.path().join(".local").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(
            bin_dir.join(if cfg!(windows) {
                "opencode.exe"
            } else {
                "opencode"
            }),
            "",
        )
        .unwrap();
        let probe = test_probe(command_never_exists, path_is_file, path_never_exists);

        assert!(is_agent_installed_with(
            AgentInstallProbe::OpenCode,
            &opencode_home,
            &probe
        ));
    }

    #[test]
    fn claude_app_probe_requires_app_binary_or_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let app_home = dir.path().join("AppData").join("Local").join("Claude");
        std::fs::create_dir_all(&app_home).unwrap();
        let probe = test_probe(command_never_exists, path_never_exists, path_never_exists);

        assert!(!is_agent_installed_with(
            AgentInstallProbe::ClaudeApp,
            &app_home,
            &probe
        ));

        let app_dir = dir
            .path()
            .join("AppData")
            .join("Local")
            .join("Programs")
            .join("Claude");
        std::fs::create_dir_all(&app_dir).unwrap();
        let app_binary = app_dir.join(if cfg!(windows) {
            "Claude.exe"
        } else {
            "Claude"
        });
        std::fs::write(&app_binary, "").unwrap();
        let probe = test_probe(command_never_exists, path_is_file, path_never_exists);

        assert!(is_agent_installed_with(
            AgentInstallProbe::ClaudeApp,
            &app_home,
            &probe
        ));
    }

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn only_codex_command_exists(binary: &str) -> bool {
        binary == "codex"
    }

    fn path_never_exists(_: &Path) -> bool {
        false
    }

    fn test_probe(
        command_exists: fn(&str) -> bool,
        path_is_file: fn(&Path) -> bool,
        path_is_dir: fn(&Path) -> bool,
    ) -> InstallStatusProbe {
        InstallStatusProbe {
            command_exists,
            path_is_file,
            path_is_dir,
        }
    }
}
