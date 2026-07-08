mod base;
mod discovery;
mod entries;
mod install;
mod object;

mod claude_app;
mod claude_cli;
mod codex;
mod general;
mod hermes;
mod openclaw;
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
        "opencode" => Ok(install::InstallableAgent::OpenCode),
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
