mod base;
mod discovery;
mod entries;
mod install;
mod install_status;
mod object;
mod process;

mod claude_app;
mod claude_cli;
mod codex;
mod general;
mod hermes;
mod kimi_code;
mod openclaw;
mod opencode;
mod pi;
mod sentra;

pub use base::Agent;
pub use discovery::discover_agents;

fn installable_agent(
    agent: &str,
    operation: &str,
) -> crate::SentraResult<install::InstallableAgent> {
    match agent {
        "codex" => Ok(install::InstallableAgent::Codex),
        "claude" | "claude-cli" => Ok(install::InstallableAgent::ClaudeCli),
        "kimi-code" => Ok(install::InstallableAgent::KimiCode),
        "opencode" => Ok(install::InstallableAgent::OpenCode),
        "pi" => Ok(install::InstallableAgent::Pi),
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
}
