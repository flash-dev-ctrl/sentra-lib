use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy)]
pub(crate) struct InstallStatusProbe {
    command_exists: fn(&str) -> bool,
    path_is_file: fn(&Path) -> bool,
    path_is_dir: fn(&Path) -> bool,
    windows_product_installed: fn(&[&str], &[&str]) -> bool,
}

impl InstallStatusProbe {
    pub(crate) fn real() -> Self {
        Self {
            command_exists,
            path_is_file,
            path_is_dir,
            windows_product_installed,
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
            windows_product_installed: |_, _| false,
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
