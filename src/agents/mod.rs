mod base;
mod discovery;
mod entries;
mod install;
mod install_status;
mod object;
mod process;

mod antigravity;
mod claude;
mod codebuddy;
mod coder;
mod codex;
mod cursor;
mod general;
mod hermes;
mod kimi;
mod kiro;
mod lingcode;
mod marvis;
mod openclaw;
mod opencode;
mod pi;
mod qoder;
mod sentra;
mod trae;
mod vscode;

pub use base::Agent;
pub use discovery::discover_agents;

pub(crate) fn workspace_agents_dir(user_home: &std::path::Path) -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let current_home = home::home_dir();
    workspace_agents_dir_from(user_home, current_home.as_deref(), &cwd)
}

fn workspace_agents_dir_from(
    user_home: &std::path::Path,
    current_home: Option<&std::path::Path>,
    cwd: &std::path::Path,
) -> Option<std::path::PathBuf> {
    if same_location(cwd, user_home) || current_home.is_some_and(|home| same_location(cwd, home)) {
        return None;
    }
    Some(cwd.join(".agents"))
}

fn same_location(left: &std::path::Path, right: &std::path::Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

fn installable_agent(
    agent: &str,
    operation: &str,
) -> crate::SentraResult<install::InstallableAgent> {
    match agent {
        "antigravity" => Ok(install::InstallableAgent::Antigravity),
        "codebuddy" | "codebuddy-code" | "codebuddy-cli" => {
            Ok(install::InstallableAgent::CodeBuddy)
        }
        "coder" => Ok(install::InstallableAgent::Coder),
        "codex" | "codex-cli" => Ok(install::InstallableAgent::Codex),
        "claude" | "claude-cli" => Ok(install::InstallableAgent::ClaudeCli),
        "cursor" => Ok(install::InstallableAgent::Cursor),
        "kimi" | "kimi-code" | "kimi-cli" => Ok(install::InstallableAgent::KimiCode),
        "kiro" => Ok(install::InstallableAgent::Kiro),
        "lingcode" => Ok(install::InstallableAgent::LingCode),
        "marvis" => Ok(install::InstallableAgent::Marvis),
        "opencode" => Ok(install::InstallableAgent::OpenCode),
        "pi" => Ok(install::InstallableAgent::Pi),
        "qoder" | "qoder-cli" => Ok(install::InstallableAgent::Qoder),
        "qoderwork" | "qoder-work" => Ok(install::InstallableAgent::QoderWork),
        "trae" => Ok(install::InstallableAgent::Trae),
        "vscode" => Ok(install::InstallableAgent::VsCode),
        "workbuddy" => Ok(install::InstallableAgent::WorkBuddy),
        other => Err(crate::SentraError::Message(format!(
            "unsupported {operation} agent: {other}"
        ))),
    }
}

pub fn install_agent(agent: &str) -> crate::SentraResult<crate::interfaces::AgentInstallResult> {
    let agent = installable_agent(agent, "installable")?;
    install::install_agent(agent)
}

pub fn install_agent_with_progress<F>(
    agent: &str,
    mut progress: F,
) -> crate::SentraResult<crate::interfaces::AgentInstallResult>
where
    F: FnMut(crate::interfaces::AgentInstallProgress),
{
    let agent = installable_agent(agent, "installable")?;
    install::install_agent_with_progress(agent, Some(&mut progress))
}

pub fn uninstall_agent(agent: &str) -> crate::SentraResult<crate::interfaces::AgentInstallResult> {
    let agent = installable_agent(agent, "uninstallable")?;
    install::uninstall_agent(agent)
}

pub fn uninstall_agent_with_options(
    agent: &str,
    options: crate::interfaces::AgentUninstallOptions,
) -> crate::SentraResult<crate::interfaces::AgentInstallResult> {
    let agent = installable_agent(agent, "uninstallable")?;
    install::uninstall_agent_with_options(agent, options, None)
}

pub fn uninstall_agent_with_progress<F>(
    agent: &str,
    mut progress: F,
) -> crate::SentraResult<crate::interfaces::AgentInstallResult>
where
    F: FnMut(crate::interfaces::AgentInstallProgress),
{
    let agent = installable_agent(agent, "uninstallable")?;
    install::uninstall_agent_with_progress(agent, Some(&mut progress))
}

pub fn uninstall_agent_with_options_and_progress<F>(
    agent: &str,
    options: crate::interfaces::AgentUninstallOptions,
    mut progress: F,
) -> crate::SentraResult<crate::interfaces::AgentInstallResult>
where
    F: FnMut(crate::interfaces::AgentInstallProgress),
{
    let agent = installable_agent(agent, "uninstallable")?;
    install::uninstall_agent_with_options(agent, options, Some(&mut progress))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_cli_and_legacy_alias_are_installable() {
        for agent in ["codex-cli", "codex"] {
            assert_eq!(
                installable_agent(agent, "installable").unwrap(),
                install::InstallableAgent::Codex
            );
        }
    }

    #[test]
    fn kimi_cli_and_legacy_aliases_are_installable() {
        for agent in ["kimi-cli", "kimi-code", "kimi"] {
            assert_eq!(
                installable_agent(agent, "installable").unwrap(),
                install::InstallableAgent::KimiCode
            );
        }
    }

    #[test]
    fn pi_is_installable() {
        assert_eq!(
            installable_agent("pi", "installable").unwrap(),
            install::InstallableAgent::Pi
        );
    }

    #[test]
    fn winget_and_npm_targets_are_installable() {
        for agent in [
            "antigravity",
            "codebuddy",
            "codebuddy-code",
            "codebuddy-cli",
            "coder",
            "cursor",
            "kiro",
            "qoder-cli",
            "qoder-work",
            "trae",
            "vscode",
            "workbuddy",
        ] {
            assert!(installable_agent(agent, "install").is_ok(), "{agent}");
        }
    }

    #[test]
    fn qoder_install_legacy_aliases_are_supported() {
        for (agent, expected) in [
            ("qoder", install::InstallableAgent::Qoder),
            ("qoderwork", install::InstallableAgent::QoderWork),
        ] {
            assert_eq!(installable_agent(agent, "install").unwrap(), expected);
        }
    }

    #[test]
    fn codebuddy_install_legacy_aliases_are_supported() {
        for agent in ["codebuddy", "codebuddy-code"] {
            assert_eq!(
                installable_agent(agent, "install").unwrap(),
                install::InstallableAgent::CodeBuddy
            );
        }
    }

    #[test]
    fn unverified_sources_are_recognized_for_explicit_blocking() {
        assert_eq!(
            installable_agent("lingcode", "install").unwrap(),
            install::InstallableAgent::LingCode
        );
        assert_eq!(
            installable_agent("marvis", "install").unwrap(),
            install::InstallableAgent::Marvis
        );
    }

    #[test]
    fn workspace_agents_dir_excludes_user_homes() {
        let scanned_home = tempfile::tempdir().unwrap();
        let current_home = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();

        assert!(
            workspace_agents_dir_from(
                scanned_home.path(),
                Some(current_home.path()),
                scanned_home.path(),
            )
            .is_none()
        );
        assert!(
            workspace_agents_dir_from(
                scanned_home.path(),
                Some(current_home.path()),
                current_home.path(),
            )
            .is_none()
        );
        assert_eq!(
            workspace_agents_dir_from(
                scanned_home.path(),
                Some(current_home.path()),
                workspace.path(),
            ),
            Some(workspace.path().join(".agents"))
        );
    }
}
