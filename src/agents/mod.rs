mod base;
mod discovery;
mod entries;
mod install;
mod install_status;
mod object;
mod process;

mod antigravity;
mod claude_app;
mod claude_cli;
mod codebuddy;
mod coder;
mod codex;
mod cursor;
mod general;
mod hermes;
mod kimi_code;
mod kiro;
mod lingcode;
mod marvis;
mod openclaw;
mod opencode;
mod pi;
mod qoder;
mod qoderwork;
mod sentra;
mod trae;
mod vscode;
mod workbuddy;

pub use base::Agent;
pub use discovery::discover_agents;

fn installable_agent(
    agent: &str,
    operation: &str,
) -> crate::SentraResult<install::InstallableAgent> {
    match agent {
        "antigravity" => Ok(install::InstallableAgent::Antigravity),
        "codebuddy" => Ok(install::InstallableAgent::CodeBuddy),
        "coder" => Ok(install::InstallableAgent::Coder),
        "codex" => Ok(install::InstallableAgent::Codex),
        "claude" | "claude-cli" => Ok(install::InstallableAgent::ClaudeCli),
        "cursor" => Ok(install::InstallableAgent::Cursor),
        "kimi-code" => Ok(install::InstallableAgent::KimiCode),
        "kiro" => Ok(install::InstallableAgent::Kiro),
        "lingcode" => Ok(install::InstallableAgent::LingCode),
        "marvis" => Ok(install::InstallableAgent::Marvis),
        "opencode" => Ok(install::InstallableAgent::OpenCode),
        "pi" => Ok(install::InstallableAgent::Pi),
        "qoder" => Ok(install::InstallableAgent::Qoder),
        "qoderwork" => Ok(install::InstallableAgent::QoderWork),
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
    fn kimi_code_is_installable() {
        assert_eq!(
            installable_agent("kimi-code", "installable").unwrap(),
            install::InstallableAgent::KimiCode
        );
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
            "coder",
            "cursor",
            "kiro",
            "qoder",
            "qoderwork",
            "trae",
            "vscode",
            "workbuddy",
        ] {
            assert!(installable_agent(agent, "install").is_ok(), "{agent}");
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
}
