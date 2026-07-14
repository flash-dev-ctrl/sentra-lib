use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy)]
pub(crate) struct InstallStatusProbe {
    command_exists: fn(&str) -> bool,
    path_is_file: fn(&Path) -> bool,
    path_is_dir: fn(&Path) -> bool,
}

impl InstallStatusProbe {
    pub(crate) fn real() -> Self {
        Self {
            command_exists,
            path_is_file,
            path_is_dir,
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
            path_is_file,
            path_is_dir,
        }
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

pub(crate) fn any_command_exists_with(binary_names: &[&str], probe: &InstallStatusProbe) -> bool {
    binary_names
        .iter()
        .any(|binary_name| (probe.command_exists)(binary_name))
}

pub(crate) fn any_existing_file_with(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    paths.iter().any(|path| (probe.path_is_file)(path))
}

pub(crate) fn any_existing_dir_with(paths: Vec<PathBuf>, probe: &InstallStatusProbe) -> bool {
    paths.iter().any(|path| (probe.path_is_dir)(path))
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

pub(crate) fn binary_paths(dir: impl Into<PathBuf>, binary: &str) -> Vec<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    fn command_never_exists(_: &str) -> bool {
        false
    }

    fn only_devin_command_exists(binary: &str) -> bool {
        binary == "devin"
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
