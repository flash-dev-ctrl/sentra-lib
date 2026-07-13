use std::process::Command;

use crate::interfaces::{
    AgentInstallAction, AgentInstallProgress, AgentInstallProgressStage, AgentInstallResult,
};
use crate::{SentraError, SentraResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallableAgent {
    Codex,
    ClaudeCli,
    OpenCode,
}

impl InstallableAgent {
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCli => "claude-cli",
            Self::OpenCode => "opencode",
        }
    }

    fn binary(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCli => "claude",
            Self::OpenCode => "opencode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallCommandPlan {
    pub(crate) program: &'static str,
    pub(crate) args: Vec<&'static str>,
    pub(crate) method: &'static str,
}

impl InstallCommandPlan {
    fn command_line(&self) -> String {
        std::iter::once(self.program)
            .chain(self.args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub(crate) fn install_agent(agent: InstallableAgent) -> SentraResult<AgentInstallResult> {
    install_agent_with_progress(agent, None)
}

pub(crate) fn install_agent_with_progress(
    agent: InstallableAgent,
    progress: Option<&mut dyn FnMut(AgentInstallProgress)>,
) -> SentraResult<AgentInstallResult> {
    let action = if command_exists(agent.binary())? {
        AgentInstallAction::Update
    } else {
        AgentInstallAction::Install
    };
    let plans = install_plans(agent, action);
    execute_agent_command(agent, action, plans, "install", progress)
}

pub(crate) fn uninstall_agent(agent: InstallableAgent) -> SentraResult<AgentInstallResult> {
    uninstall_agent_with_progress(agent, None)
}

pub(crate) fn uninstall_agent_with_progress(
    agent: InstallableAgent,
    progress: Option<&mut dyn FnMut(AgentInstallProgress)>,
) -> SentraResult<AgentInstallResult> {
    let action = AgentInstallAction::Uninstall;
    execute_agent_command(agent, action, uninstall_plans(agent), "uninstall", progress)
}

fn execute_agent_command(
    agent: InstallableAgent,
    action: AgentInstallAction,
    plans: Vec<InstallCommandPlan>,
    verb: &str,
    mut progress: Option<&mut dyn FnMut(AgentInstallProgress)>,
) -> SentraResult<AgentInstallResult> {
    let mut errors = Vec::new();
    let total = plans.len() + 1;
    for (index, plan) in plans.into_iter().enumerate() {
        if let Some(reporter) = progress.as_deref_mut() {
            reporter(AgentInstallProgress {
                agent: agent.name().to_string(),
                action,
                current: index + 1,
                total,
                method: plan.method.to_string(),
                stage: AgentInstallProgressStage::Trying,
            });
        }
        let output = match Command::new(plan.program).args(&plan.args).output() {
            Ok(output) => output,
            Err(err) => {
                errors.push(format!("`{}` could not start: {err}", plan.command_line()));
                continue;
            }
        };
        if output.status.success() {
            if let Some(reporter) = progress.as_deref_mut() {
                reporter(AgentInstallProgress {
                    agent: agent.name().to_string(),
                    action,
                    current: total,
                    total,
                    method: agent.name().to_string(),
                    stage: AgentInstallProgressStage::Verifying,
                });
            }
            return Ok(AgentInstallResult {
                agent: agent.name().to_string(),
                action,
                command: plan.command_line(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        errors.push(format!(
            "`{}` exited {:?}: {}",
            plan.command_line(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Err(SentraError::Message(format!(
        "failed to {verb} {}: {}",
        agent.name(),
        errors.join("; ")
    )))
}

fn install_plans(agent: InstallableAgent, action: AgentInstallAction) -> Vec<InstallCommandPlan> {
    install_plans_for_platform(agent, platform(), action)
}

fn install_plans_for_platform(
    agent: InstallableAgent,
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match agent {
        InstallableAgent::Codex => codex_install_plans_for_platform(platform, action),
        InstallableAgent::ClaudeCli => claude_install_plans_for_platform(platform, action),
        InstallableAgent::OpenCode => opencode_install_plans_for_platform(platform, action),
    }
}

fn codex_install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Unix => vec![
            npm_plan(npm_codex_package(action)),
            install_plan_for_platform(InstallableAgent::Codex, platform),
        ],
        Platform::Windows => vec![
            windows_npm_plan(npm_codex_package(action)),
            winget_codex_plan(action),
            install_plan_for_platform(InstallableAgent::Codex, platform),
        ],
    }
}

fn claude_install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Unix => vec![
            npm_plan(npm_claude_package(action)),
            install_plan_for_platform(InstallableAgent::ClaudeCli, platform),
        ],
        Platform::Windows => vec![
            windows_npm_plan(npm_claude_package(action)),
            winget_claude_plan(action),
            install_plan_for_platform(InstallableAgent::ClaudeCli, platform),
            claude_cmd_install_plan(),
        ],
    }
}

fn install_plan_for_platform(agent: InstallableAgent, platform: Platform) -> InstallCommandPlan {
    match (agent, platform) {
        (InstallableAgent::Codex, Platform::Windows) => powershell_plan(
            "Import-Module Microsoft.PowerShell.Utility; irm https://chatgpt.com/codex/install.ps1 | iex",
            "PowerShell installer",
        ),
        (InstallableAgent::Codex, Platform::Unix) => sh_plan(
            "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
            "standalone installer",
        ),
        (InstallableAgent::ClaudeCli, Platform::Windows) => powershell_plan(
            "Import-Module Microsoft.PowerShell.Utility; irm https://claude.ai/install.ps1 | iex",
            "PowerShell installer",
        ),
        (InstallableAgent::ClaudeCli, Platform::Unix) => sh_plan(
            "curl -fsSL https://claude.ai/install.sh | bash",
            "standalone installer",
        ),
        (InstallableAgent::OpenCode, Platform::Unix) => sh_plan(
            "curl -fsSL https://opencode.ai/install | bash",
            "standalone installer",
        ),
        (InstallableAgent::OpenCode, Platform::Windows) => {
            windows_npm_plan(opencode_npm_package(AgentInstallAction::Install))
        }
    }
}

fn uninstall_plans(agent: InstallableAgent) -> Vec<InstallCommandPlan> {
    vec![uninstall_plan_for_platform(agent, platform())]
}

fn uninstall_plan_for_platform(agent: InstallableAgent, platform: Platform) -> InstallCommandPlan {
    match (agent, platform) {
        (InstallableAgent::Codex, Platform::Windows) => powershell_plan(
            r#"$ErrorActionPreference = 'Stop'; Remove-Item -LiteralPath "$env:LOCALAPPDATA\Programs\OpenAI\Codex\bin","$env:USERPROFILE\.codex\packages\standalone" -Recurse -Force -ErrorAction SilentlyContinue; if (Get-Command npm -ErrorAction SilentlyContinue) { npm list -g @openai/codex *> $null; if ($LASTEXITCODE -eq 0) { npm uninstall -g @openai/codex; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } } }; exit 0"#,
            "PowerShell uninstall script",
        ),
        (InstallableAgent::Codex, Platform::Unix) => sh_plan(
            r#"rm -f "$HOME/.local/bin/codex" "${CODEX_INSTALL_DIR:-$HOME/.local/bin}/codex"; rm -rf "${CODEX_HOME:-$HOME/.codex}/packages/standalone"; if command -v npm >/dev/null 2>&1 && npm list -g @openai/codex >/dev/null 2>&1; then npm uninstall -g @openai/codex; fi"#,
            "shell uninstall script",
        ),
        (InstallableAgent::ClaudeCli, Platform::Windows) => powershell_plan(
            r#"$ErrorActionPreference = 'Stop'; Remove-Item -LiteralPath "$env:USERPROFILE\.local\bin\claude.exe","$env:USERPROFILE\.local\share\claude" -Recurse -Force -ErrorAction SilentlyContinue; if (Get-Command winget -ErrorAction SilentlyContinue) { winget list --id Anthropic.ClaudeCode --exact *> $null; if ($LASTEXITCODE -eq 0) { winget uninstall Anthropic.ClaudeCode; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } } }; if (Get-Command npm -ErrorAction SilentlyContinue) { npm list -g @anthropic-ai/claude-code *> $null; if ($LASTEXITCODE -eq 0) { npm uninstall -g @anthropic-ai/claude-code; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } } }; exit 0"#,
            "PowerShell uninstall script",
        ),
        (InstallableAgent::ClaudeCli, Platform::Unix) => sh_plan(
            r#"rm -f "$HOME/.local/bin/claude"; rm -rf "$HOME/.local/share/claude"; if command -v brew >/dev/null 2>&1 && brew list --cask claude-code >/dev/null 2>&1; then brew uninstall --cask claude-code; fi; if command -v npm >/dev/null 2>&1 && npm list -g @anthropic-ai/claude-code >/dev/null 2>&1; then npm uninstall -g @anthropic-ai/claude-code; fi"#,
            "shell uninstall script",
        ),
        (InstallableAgent::OpenCode, Platform::Windows) => powershell_plan(
            r#"$ErrorActionPreference = 'Continue'; if (Get-Command npm -ErrorAction SilentlyContinue) { npm list -g opencode-ai *> $null; if ($LASTEXITCODE -eq 0) { npm uninstall -g opencode-ai; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } } }; if (Get-Command bun -ErrorAction SilentlyContinue) { bun pm ls -g opencode-ai *> $null; if ($LASTEXITCODE -eq 0) { bun remove -g opencode-ai; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE } } }; exit 0"#,
            "PowerShell uninstall script",
        ),
        (InstallableAgent::OpenCode, Platform::Unix) => sh_plan(
            r#"if command -v npm >/dev/null 2>&1 && npm list -g opencode-ai >/dev/null 2>&1; then npm uninstall -g opencode-ai; fi; if command -v bun >/dev/null 2>&1; then bun remove -g opencode-ai >/dev/null 2>&1 || true; fi; if command -v brew >/dev/null 2>&1 && brew list anomalyco/tap/opencode >/dev/null 2>&1; then brew uninstall opencode; fi"#,
            "shell uninstall script",
        ),
    }
}

fn command_exists(binary: &str) -> SentraResult<bool> {
    let output = if cfg!(windows) {
        Command::new("where").arg(binary).output()
    } else {
        Command::new("sh")
            .args(["-c", &format!("command -v {binary}")])
            .output()
    }?;
    Ok(output.status.success())
}

fn powershell_plan(command: &'static str, method: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "powershell",
        args: vec![
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            command,
        ],
        method,
    }
}

fn sh_plan(command: &'static str, method: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "sh",
        args: vec!["-c", command],
        method,
    }
}

fn winget_claude_plan(action: AgentInstallAction) -> InstallCommandPlan {
    let verb = match action {
        AgentInstallAction::Install => "install",
        AgentInstallAction::Update => "upgrade",
        AgentInstallAction::Uninstall => "uninstall",
    };
    InstallCommandPlan {
        program: "winget",
        args: vec![
            verb,
            "Anthropic.ClaudeCode",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        method: "WinGet",
    }
}

fn winget_codex_plan(action: AgentInstallAction) -> InstallCommandPlan {
    let verb = match action {
        AgentInstallAction::Install => "install",
        AgentInstallAction::Update => "upgrade",
        AgentInstallAction::Uninstall => "uninstall",
    };
    InstallCommandPlan {
        program: "winget",
        args: vec![
            verb,
            "-e",
            "--id",
            "OpenAI.Codex",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        method: "WinGet",
    }
}

fn claude_cmd_install_plan() -> InstallCommandPlan {
    InstallCommandPlan {
        program: "cmd",
        args: vec![
            "/C",
            "curl -fsSL https://claude.ai/install.cmd -o install.cmd && install.cmd && del install.cmd",
        ],
        method: "CMD installer",
    }
}

fn opencode_install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Unix => vec![
            npm_plan(opencode_npm_package(action)),
            bun_plan(opencode_bun_package(action)),
            brew_opencode_plan(action),
            install_plan_for_platform(InstallableAgent::OpenCode, platform),
        ],
        Platform::Windows => vec![
            windows_npm_plan(opencode_npm_package(action)),
            windows_bun_plan(opencode_bun_package(action)),
        ],
    }
}

fn npm_codex_package(action: AgentInstallAction) -> &'static str {
    match action {
        AgentInstallAction::Install => "@openai/codex",
        AgentInstallAction::Update => "@openai/codex@latest",
        AgentInstallAction::Uninstall => "@openai/codex",
    }
}

fn opencode_npm_package(action: AgentInstallAction) -> &'static str {
    match action {
        AgentInstallAction::Install => "opencode-ai",
        AgentInstallAction::Update => "opencode-ai@latest",
        AgentInstallAction::Uninstall => "opencode-ai",
    }
}

fn opencode_bun_package(action: AgentInstallAction) -> &'static str {
    match action {
        AgentInstallAction::Install => "opencode-ai",
        AgentInstallAction::Update => "opencode-ai@latest",
        AgentInstallAction::Uninstall => "opencode-ai",
    }
}

fn npm_plan(package: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "npm",
        args: vec!["install", "-g", package],
        method: "npm",
    }
}

fn bun_plan(package: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "bun",
        args: vec!["add", "-g", package],
        method: "bun",
    }
}

fn windows_bun_plan(package: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "cmd",
        args: vec!["/C", "bun", "add", "-g", package],
        method: "bun",
    }
}

fn brew_opencode_plan(action: AgentInstallAction) -> InstallCommandPlan {
    let verb = match action {
        AgentInstallAction::Install => "install",
        AgentInstallAction::Update => "upgrade",
        AgentInstallAction::Uninstall => "uninstall",
    };
    InstallCommandPlan {
        program: "brew",
        args: vec![verb, "anomalyco/tap/opencode"],
        method: "Homebrew",
    }
}

fn windows_npm_plan(package: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "cmd",
        args: vec!["/C", "npm", "install", "-g", package],
        method: "npm",
    }
}

fn npm_claude_package(action: AgentInstallAction) -> &'static str {
    match action {
        AgentInstallAction::Install => "@anthropic-ai/claude-code",
        AgentInstallAction::Update => "@anthropic-ai/claude-code@latest",
        AgentInstallAction::Uninstall => "@anthropic-ai/claude-code",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Unix,
    Windows,
}

fn platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Unix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_text(plan: InstallCommandPlan) -> String {
        plan.command_line()
    }

    #[test]
    fn codex_plan_supports_unix_and_windows_installers() {
        assert_eq!(
            command_text(install_plan_for_platform(
                InstallableAgent::Codex,
                Platform::Unix
            )),
            "sh -c curl -fsSL https://chatgpt.com/codex/install.sh | sh"
        );
        assert!(command_text(install_plan_for_platform(
            InstallableAgent::Codex,
            Platform::Windows
        ))
        .contains("powershell -NoProfile -ExecutionPolicy Bypass -Command Import-Module Microsoft.PowerShell.Utility; irm https://chatgpt.com/codex/install.ps1 | iex"));
    }

    #[test]
    fn windows_powershell_plan_loads_file_hash_module() {
        let command = command_text(install_plan_for_platform(
            InstallableAgent::Codex,
            Platform::Windows,
        ));

        assert!(command.contains("Import-Module Microsoft.PowerShell.Utility"));
        assert!(command.contains("https://chatgpt.com/codex/install.ps1"));
    }

    #[test]
    fn codex_install_and_update_prefer_npm_then_fallbacks() {
        let unix_install_plans = install_plans_for_platform(
            InstallableAgent::Codex,
            Platform::Unix,
            AgentInstallAction::Install,
        );
        assert_eq!(
            unix_install_plans[0].command_line(),
            "npm install -g @openai/codex"
        );
        assert_eq!(
            unix_install_plans[1].command_line(),
            "sh -c curl -fsSL https://chatgpt.com/codex/install.sh | sh"
        );

        let unix_update_plans = install_plans_for_platform(
            InstallableAgent::Codex,
            Platform::Unix,
            AgentInstallAction::Update,
        );
        assert_eq!(
            unix_update_plans[0].command_line(),
            "npm install -g @openai/codex@latest"
        );

        let install_plans = install_plans_for_platform(
            InstallableAgent::Codex,
            Platform::Windows,
            AgentInstallAction::Install,
        );
        assert_eq!(
            install_plans[0].command_line(),
            "cmd /C npm install -g @openai/codex"
        );
        assert_eq!(
            install_plans[1].command_line(),
            "winget install -e --id OpenAI.Codex --accept-package-agreements --accept-source-agreements"
        );
        assert!(
            install_plans[2]
                .command_line()
                .contains("https://chatgpt.com/codex/install.ps1")
        );

        let update_plans = install_plans_for_platform(
            InstallableAgent::Codex,
            Platform::Windows,
            AgentInstallAction::Update,
        );
        assert_eq!(
            update_plans[0].command_line(),
            "cmd /C npm install -g @openai/codex@latest"
        );
        assert_eq!(
            update_plans[1].command_line(),
            "winget upgrade -e --id OpenAI.Codex --accept-package-agreements --accept-source-agreements"
        );
        assert!(
            update_plans[2]
                .command_line()
                .contains("https://chatgpt.com/codex/install.ps1")
        );
    }

    #[test]
    fn command_runner_continues_after_spawn_error() {
        let result = execute_agent_command(
            InstallableAgent::Codex,
            AgentInstallAction::Install,
            vec![
                InstallCommandPlan {
                    program: "sentra-missing-installer-command",
                    args: Vec::new(),
                    method: "missing test installer",
                },
                InstallCommandPlan {
                    program: "rustc",
                    args: vec!["--version"],
                    method: "rustc test installer",
                },
            ],
            "install",
            None,
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
    }

    #[test]
    fn command_runner_continues_after_command_failure() {
        let result = execute_agent_command(
            InstallableAgent::Codex,
            AgentInstallAction::Install,
            vec![
                InstallCommandPlan {
                    program: "rustc",
                    args: vec!["--sentra-missing-test-flag"],
                    method: "failing test installer",
                },
                InstallCommandPlan {
                    program: "rustc",
                    args: vec!["--version"],
                    method: "rustc test installer",
                },
            ],
            "install",
            None,
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
    }

    #[test]
    fn command_runner_reports_each_attempt() {
        let mut progress = Vec::new();
        let result = execute_agent_command(
            InstallableAgent::Codex,
            AgentInstallAction::Install,
            vec![
                InstallCommandPlan {
                    program: "sentra-missing-installer-command",
                    args: Vec::new(),
                    method: "missing test installer",
                },
                InstallCommandPlan {
                    program: "rustc",
                    args: vec!["--version"],
                    method: "rustc test installer",
                },
            ],
            "install",
            Some(&mut |event| progress.push(event)),
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
        assert_eq!(progress.len(), 3);
        assert_eq!(progress[0].current, 1);
        assert_eq!(progress[0].total, 3);
        assert_eq!(progress[0].method, "missing test installer");
        assert_eq!(progress[0].stage, AgentInstallProgressStage::Trying);
        assert_eq!(progress[1].current, 2);
        assert_eq!(progress[1].total, 3);
        assert_eq!(progress[1].method, "rustc test installer");
        assert_eq!(progress[1].stage, AgentInstallProgressStage::Trying);
        assert_eq!(progress[2].current, 3);
        assert_eq!(progress[2].total, 3);
        assert_eq!(progress[2].method, "codex");
        assert_eq!(progress[2].stage, AgentInstallProgressStage::Verifying);
    }

    #[test]
    fn command_runner_reports_success_verification() {
        let mut progress = Vec::new();
        let result = execute_agent_command(
            InstallableAgent::OpenCode,
            AgentInstallAction::Update,
            vec![InstallCommandPlan {
                program: "rustc",
                args: vec!["--version"],
                method: "rustc test installer",
            }],
            "install",
            Some(&mut |event| progress.push(event)),
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
        assert_eq!(progress.len(), 2);
        assert_eq!(progress[0].current, 1);
        assert_eq!(progress[0].total, 2);
        assert_eq!(progress[0].method, "rustc test installer");
        assert_eq!(progress[0].stage, AgentInstallProgressStage::Trying);
        assert_eq!(progress[1].current, 2);
        assert_eq!(progress[1].total, 2);
        assert_eq!(progress[1].method, "opencode");
        assert_eq!(progress[1].stage, AgentInstallProgressStage::Verifying);
    }

    #[test]
    fn opencode_plan_supports_official_installers() {
        let unix = install_plans_for_platform(
            InstallableAgent::OpenCode,
            Platform::Unix,
            AgentInstallAction::Install,
        );
        assert_eq!(unix[0].command_line(), "npm install -g opencode-ai");
        assert_eq!(unix[1].command_line(), "bun add -g opencode-ai");
        assert_eq!(
            unix[2].command_line(),
            "brew install anomalyco/tap/opencode"
        );
        assert_eq!(
            unix[3].command_line(),
            "sh -c curl -fsSL https://opencode.ai/install | bash"
        );

        let windows = install_plans_for_platform(
            InstallableAgent::OpenCode,
            Platform::Windows,
            AgentInstallAction::Install,
        );
        assert_eq!(
            windows[0].command_line(),
            "cmd /C npm install -g opencode-ai"
        );
        assert_eq!(windows[1].command_line(), "cmd /C bun add -g opencode-ai");
    }

    #[test]
    fn opencode_uninstall_plan_preserves_user_data() {
        let unix = command_text(uninstall_plan_for_platform(
            InstallableAgent::OpenCode,
            Platform::Unix,
        ));
        assert!(unix.contains("npm uninstall -g opencode-ai"));
        assert!(unix.contains("bun remove -g opencode-ai"));
        assert!(unix.contains("brew uninstall opencode"));
        assert!(!unix.contains(".opencode"));

        let windows = command_text(uninstall_plan_for_platform(
            InstallableAgent::OpenCode,
            Platform::Windows,
        ));
        assert!(windows.contains("npm uninstall -g opencode-ai"));
        assert!(windows.contains("bun remove -g opencode-ai"));
        assert!(windows.contains("$ErrorActionPreference = 'Continue'"));
        assert!(windows.ends_with("exit 0"));
        assert!(!windows.contains(".opencode"));
    }

    #[test]
    fn windows_claude_install_prefers_package_managers_then_installer_fallbacks() {
        let plans = install_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows,
            AgentInstallAction::Install,
        );

        assert_eq!(plans.len(), 4);
        assert_eq!(
            plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code"
        );
        assert_eq!(
            plans[1].command_line(),
            "winget install Anthropic.ClaudeCode --accept-package-agreements --accept-source-agreements"
        );
        assert!(
            plans[2]
                .command_line()
                .contains("https://claude.ai/install.ps1")
        );
        assert!(
            plans[3]
                .command_line()
                .contains("https://claude.ai/install.cmd")
        );
    }

    #[test]
    fn windows_claude_update_has_winget_fallback() {
        let plans = install_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows,
            AgentInstallAction::Update,
        );

        assert_eq!(
            plans[1].command_line(),
            "winget upgrade Anthropic.ClaudeCode --accept-package-agreements --accept-source-agreements"
        );
    }

    #[test]
    fn claude_install_and_update_prefer_npm_then_fallbacks() {
        let unix_install_plans = install_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Unix,
            AgentInstallAction::Install,
        );
        assert_eq!(
            unix_install_plans[0].command_line(),
            "npm install -g @anthropic-ai/claude-code"
        );
        assert_eq!(
            unix_install_plans[1].command_line(),
            "sh -c curl -fsSL https://claude.ai/install.sh | bash"
        );

        let unix_update_plans = install_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Unix,
            AgentInstallAction::Update,
        );
        assert_eq!(
            unix_update_plans[0].command_line(),
            "npm install -g @anthropic-ai/claude-code@latest"
        );

        let install_plans = install_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows,
            AgentInstallAction::Install,
        );
        assert_eq!(
            install_plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code"
        );

        let update_plans = install_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows,
            AgentInstallAction::Update,
        );
        assert_eq!(
            update_plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code@latest"
        );
    }

    #[test]
    fn claude_plan_supports_unix_and_windows_installers() {
        assert_eq!(
            command_text(install_plan_for_platform(
                InstallableAgent::ClaudeCli,
                Platform::Unix
            )),
            "sh -c curl -fsSL https://claude.ai/install.sh | bash"
        );
        assert!(command_text(install_plan_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows
        ))
        .contains("powershell -NoProfile -ExecutionPolicy Bypass -Command Import-Module Microsoft.PowerShell.Utility; irm https://claude.ai/install.ps1 | iex"));
    }

    #[test]
    fn codex_uninstall_plan_preserves_user_data() {
        let unix = command_text(uninstall_plan_for_platform(
            InstallableAgent::Codex,
            Platform::Unix,
        ));
        assert!(unix.contains(".local/bin/codex"));
        assert!(unix.contains("packages/standalone"));
        assert!(unix.contains("@openai/codex"));
        assert!(!unix.contains("rm -rf \"$HOME/.codex\""));

        let windows = command_text(uninstall_plan_for_platform(
            InstallableAgent::Codex,
            Platform::Windows,
        ));
        assert!(windows.contains("LOCALAPPDATA"));
        assert!(windows.contains(".codex\\packages\\standalone"));
        assert!(windows.contains("@openai/codex"));
        assert!(!windows.contains("Remove-Item -Path \"$env:USERPROFILE\\.codex\""));
    }

    #[test]
    fn claude_uninstall_plan_supports_official_methods_without_user_data() {
        let unix = command_text(uninstall_plan_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Unix,
        ));
        assert!(unix.contains(".local/bin/claude"));
        assert!(unix.contains(".local/share/claude"));
        assert!(unix.contains("brew uninstall --cask claude-code"));
        assert!(unix.contains("npm uninstall -g @anthropic-ai/claude-code"));
        assert!(!unix.contains("rm -rf \"$HOME/.claude\""));

        let windows = command_text(uninstall_plan_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows,
        ));
        assert!(windows.contains(".local\\bin\\claude.exe"));
        assert!(windows.contains(".local\\share\\claude"));
        assert!(windows.contains("winget uninstall Anthropic.ClaudeCode"));
        assert!(windows.contains("npm uninstall -g @anthropic-ai/claude-code"));
        assert!(windows.ends_with("exit 0"));
        assert!(!windows.contains("Remove-Item -Path \"$env:USERPROFILE\\.claude\""));
    }
}
