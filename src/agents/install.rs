use std::process::Command;

use crate::interfaces::{
    AgentInstallAction, AgentInstallProgress, AgentInstallProgressStage, AgentInstallResult,
    AgentUninstallOptions,
};
use crate::{SentraError, SentraResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallableAgent {
    Codex,
    ClaudeCli,
    KimiCode,
    OpenCode,
    Pi,
}

impl InstallableAgent {
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCli => "claude-cli",
            Self::KimiCode => "kimi-code",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
        }
    }

    fn binary(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCli => "claude",
            Self::KimiCode => "kimi",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallCommandPlan {
    pub(crate) program: &'static str,
    pub(crate) args: Vec<String>,
    pub(crate) method: &'static str,
}

impl InstallCommandPlan {
    pub(crate) fn command_line(&self) -> String {
        std::iter::once(self.program.to_string())
            .chain(self.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentInstallSpec {
    pub(crate) npm_package: Option<&'static str>,
    pub(crate) npm_update_package: Option<&'static str>,
    pub(crate) npm_global_options: &'static [&'static str],
    pub(crate) pnpm_package: Option<&'static str>,
    pub(crate) pnpm_update_package: Option<&'static str>,
    pub(crate) pnpm_global_options: &'static [&'static str],
    pub(crate) curl_command: Option<&'static str>,
    pub(crate) powershell_command: Option<&'static str>,
    pub(crate) brew_package: Option<&'static str>,
    pub(crate) brew_uninstall_package: Option<&'static str>,
    pub(crate) unix_files: &'static [&'static str],
    pub(crate) unix_dirs: &'static [&'static str],
    pub(crate) unix_config_files: &'static [&'static str],
    pub(crate) unix_config_dirs: &'static [&'static str],
    pub(crate) windows_paths: &'static [&'static str],
    pub(crate) windows_config_paths: &'static [&'static str],
    pub(crate) powershell_error_action: &'static str,
}

impl AgentInstallSpec {
    pub(crate) fn plans_for_platform(
        self,
        platform: Platform,
        action: AgentInstallAction,
    ) -> Vec<InstallCommandPlan> {
        let mut plans = Vec::new();
        if let Some(package) = package_for_action(self.npm_package, self.npm_update_package, action)
        {
            plans.push(match platform {
                Platform::Unix => npm_plan(package, self.npm_global_options),
                Platform::Windows => windows_npm_plan(package, self.npm_global_options),
            });
        }
        if let Some(package) =
            package_for_action(self.pnpm_package, self.pnpm_update_package, action)
        {
            plans.push(match platform {
                Platform::Unix => pnpm_plan(package, self.pnpm_global_options),
                Platform::Windows => windows_pnpm_plan(package, self.pnpm_global_options),
            });
        }

        match platform {
            Platform::Unix => {
                if let Some(package) = self.brew_package {
                    plans.push(brew_plan(package, action));
                }
                if let Some(command) = self.curl_command {
                    plans.push(sh_plan(command, "standalone installer"));
                }
            }
            Platform::Windows => {
                if let Some(command) = self.powershell_command {
                    plans.push(powershell_plan(command, "PowerShell installer"));
                }
            }
        }
        plans
    }

    pub(crate) fn uninstall_plan_for_platform(
        self,
        platform: Platform,
        options: AgentUninstallOptions,
    ) -> InstallCommandPlan {
        match platform {
            Platform::Unix => sh_plan(
                self.unix_uninstall_script(options),
                "shell uninstall script",
            ),
            Platform::Windows => powershell_plan(
                self.windows_uninstall_script(options),
                "PowerShell uninstall script",
            ),
        }
    }

    fn unix_uninstall_script(self, options: AgentUninstallOptions) -> String {
        let mut commands = Vec::new();
        push_shell_remove(&mut commands, "rm -f", self.unix_files);
        push_shell_remove(&mut commands, "rm -rf", self.unix_dirs);
        if options.delete_config {
            push_shell_remove(&mut commands, "rm -f", self.unix_config_files);
            push_shell_remove(&mut commands, "rm -rf", self.unix_config_dirs);
        }
        if let Some(package) = self.npm_package {
            commands.push(format!(
                "if command -v npm >/dev/null 2>&1 && npm list -g {package} >/dev/null 2>&1; then npm uninstall -g {package}; fi"
            ));
        }
        if let Some(package) = self.pnpm_package {
            commands.push(format!(
                "if command -v pnpm >/dev/null 2>&1 && pnpm list -g {package} >/dev/null 2>&1; then pnpm remove -g {package}; fi"
            ));
        }
        if let Some(package) = self.brew_package {
            let uninstall_package = self.brew_uninstall_package.unwrap_or(package);
            commands.push(format!(
                "if command -v brew >/dev/null 2>&1 && brew list {package} >/dev/null 2>&1; then brew uninstall {uninstall_package}; fi"
            ));
        }
        commands.join("; ")
    }

    fn windows_uninstall_script(self, options: AgentUninstallOptions) -> String {
        let mut commands = vec![format!(
            "$ErrorActionPreference = '{}'",
            self.powershell_error_action
        )];
        push_powershell_remove(&mut commands, self.windows_paths);
        if options.delete_config {
            push_powershell_remove(&mut commands, self.windows_config_paths);
        }
        if let Some(package) = self.npm_package {
            push_windows_package_uninstall(&mut commands, "npm", "uninstall", package);
        }
        if let Some(package) = self.pnpm_package {
            push_windows_package_uninstall(&mut commands, "pnpm", "remove", package);
        }
        commands.push("exit 0".to_string());
        commands.join("; ")
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
    uninstall_agent_with_options(agent, AgentUninstallOptions::default(), progress)
}

pub(crate) fn uninstall_agent_with_options(
    agent: InstallableAgent,
    options: AgentUninstallOptions,
    progress: Option<&mut dyn FnMut(AgentInstallProgress)>,
) -> SentraResult<AgentInstallResult> {
    let action = AgentInstallAction::Uninstall;
    execute_agent_command(
        agent,
        action,
        uninstall_plans(agent, options),
        "uninstall",
        progress,
    )
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
        InstallableAgent::Codex => {
            crate::agents::codex::install_plans_for_platform(platform, action)
        }
        InstallableAgent::ClaudeCli => {
            crate::agents::claude_cli::install_plans_for_platform(platform, action)
        }
        InstallableAgent::KimiCode => {
            crate::agents::kimi_code::install_plans_for_platform(platform, action)
        }
        InstallableAgent::OpenCode => {
            crate::agents::opencode::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Pi => crate::agents::pi::install_plans_for_platform(platform, action),
    }
}

fn uninstall_plans(
    agent: InstallableAgent,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    vec![uninstall_plan_for_platform(agent, platform(), options)]
}

fn uninstall_plan_for_platform(
    agent: InstallableAgent,
    platform: Platform,
    options: AgentUninstallOptions,
) -> InstallCommandPlan {
    match agent {
        InstallableAgent::Codex => {
            crate::agents::codex::uninstall_plan_for_platform(platform, options)
        }
        InstallableAgent::ClaudeCli => {
            crate::agents::claude_cli::uninstall_plan_for_platform(platform, options)
        }
        InstallableAgent::KimiCode => {
            crate::agents::kimi_code::uninstall_plan_for_platform(platform, options)
        }
        InstallableAgent::OpenCode => {
            crate::agents::opencode::uninstall_plan_for_platform(platform, options)
        }
        InstallableAgent::Pi => crate::agents::pi::uninstall_plan_for_platform(platform, options),
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

pub(crate) fn powershell_plan(
    command: impl Into<String>,
    method: &'static str,
) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "powershell",
        args: vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-Command".to_string(),
            command.into(),
        ],
        method,
    }
}

pub(crate) fn sh_plan(command: impl Into<String>, method: &'static str) -> InstallCommandPlan {
    InstallCommandPlan {
        program: "sh",
        args: vec!["-c".to_string(), command.into()],
        method,
    }
}

pub(crate) fn npm_plan(
    package: &'static str,
    global_options: &'static [&'static str],
) -> InstallCommandPlan {
    let mut args = vec!["install".to_string(), "-g".to_string()];
    args.extend(global_options.iter().map(|arg| (*arg).to_string()));
    args.push(package.to_string());
    InstallCommandPlan {
        program: "npm",
        args,
        method: "npm",
    }
}

pub(crate) fn pnpm_plan(
    package: &'static str,
    global_options: &'static [&'static str],
) -> InstallCommandPlan {
    let mut args = vec!["add".to_string(), "-g".to_string()];
    args.extend(global_options.iter().map(|arg| (*arg).to_string()));
    args.push(package.to_string());
    InstallCommandPlan {
        program: "pnpm",
        args,
        method: "pnpm",
    }
}

pub(crate) fn windows_pnpm_plan(
    package: &'static str,
    global_options: &'static [&'static str],
) -> InstallCommandPlan {
    let mut args = vec![
        "/C".to_string(),
        "pnpm".to_string(),
        "add".to_string(),
        "-g".to_string(),
    ];
    args.extend(global_options.iter().map(|arg| (*arg).to_string()));
    args.push(package.to_string());
    InstallCommandPlan {
        program: "cmd",
        args,
        method: "pnpm",
    }
}

pub(crate) fn windows_npm_plan(
    package: &'static str,
    global_options: &'static [&'static str],
) -> InstallCommandPlan {
    let mut args = vec![
        "/C".to_string(),
        "npm".to_string(),
        "install".to_string(),
        "-g".to_string(),
    ];
    args.extend(global_options.iter().map(|arg| (*arg).to_string()));
    args.push(package.to_string());
    InstallCommandPlan {
        program: "cmd",
        args,
        method: "npm",
    }
}

fn brew_plan(package: &'static str, action: AgentInstallAction) -> InstallCommandPlan {
    let verb = match action {
        AgentInstallAction::Install => "install",
        AgentInstallAction::Update => "upgrade",
        AgentInstallAction::Uninstall => "uninstall",
    };
    InstallCommandPlan {
        program: "brew",
        args: vec![verb.to_string(), package.to_string()],
        method: "Homebrew",
    }
}

fn package_for_action(
    install_package: Option<&'static str>,
    update_package: Option<&'static str>,
    action: AgentInstallAction,
) -> Option<&'static str> {
    match action {
        AgentInstallAction::Install | AgentInstallAction::Uninstall => install_package,
        AgentInstallAction::Update => update_package.or(install_package),
    }
}

fn push_shell_remove(commands: &mut Vec<String>, command: &str, paths: &[&'static str]) {
    if !paths.is_empty() {
        commands.push(format!("{command} {}", paths.join(" ")));
    }
}

fn push_powershell_remove(commands: &mut Vec<String>, paths: &[&'static str]) {
    if paths.is_empty() {
        return;
    }

    let literal_paths = paths
        .iter()
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(",");
    commands.push(format!(
        "Remove-Item -LiteralPath {literal_paths} -Recurse -Force -ErrorAction SilentlyContinue"
    ));
}

fn push_windows_package_uninstall(
    commands: &mut Vec<String>,
    manager: &str,
    uninstall_verb: &str,
    package: &str,
) {
    commands.push(format!(
        "if (Get-Command {manager} -ErrorAction SilentlyContinue) {{ $previousErrorActionPreference = $ErrorActionPreference; $ErrorActionPreference = 'Continue'; cmd /C \"{manager} list -g {package} >NUL 2>NUL\"; $packageInstalled = $LASTEXITCODE -eq 0; if ($packageInstalled) {{ cmd /C \"{manager} {uninstall_verb} -g {package}\"; $packageExitCode = $LASTEXITCODE }} else {{ $packageExitCode = 0 }}; $ErrorActionPreference = $previousErrorActionPreference; if ($packageExitCode -ne 0) {{ exit $packageExitCode }} }}"
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
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
                    args: vec!["--version".to_string()],
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
                    args: vec!["--sentra-missing-test-flag".to_string()],
                    method: "failing test installer",
                },
                InstallCommandPlan {
                    program: "rustc",
                    args: vec!["--version".to_string()],
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
                    args: vec!["--version".to_string()],
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
                args: vec!["--version".to_string()],
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
    fn windows_uninstall_probe_uses_cmd_to_avoid_powershell_shim_errors() {
        let command = AgentInstallSpec {
            npm_package: Some("@scope/test-agent"),
            npm_update_package: Some("@scope/test-agent@latest"),
            npm_global_options: &[],
            pnpm_package: Some("@scope/test-agent"),
            pnpm_update_package: Some("@scope/test-agent@latest"),
            pnpm_global_options: &[],
            curl_command: None,
            powershell_command: None,
            brew_package: None,
            brew_uninstall_package: None,
            unix_files: &[],
            unix_dirs: &[],
            unix_config_files: &[],
            unix_config_dirs: &[],
            windows_paths: &[],
            windows_config_paths: &[],
            powershell_error_action: "Stop",
        }
        .uninstall_plan_for_platform(
            Platform::Windows,
            AgentUninstallOptions {
                delete_config: false,
            },
        )
        .command_line();

        assert!(command.contains("cmd /C \"npm list -g @scope/test-agent >NUL 2>NUL\""));
        assert!(command.contains("cmd /C \"pnpm list -g @scope/test-agent >NUL 2>NUL\""));
        assert!(command.contains("cmd /C \"npm uninstall -g @scope/test-agent\""));
        assert!(command.contains("cmd /C \"pnpm remove -g @scope/test-agent\""));
        assert!(command.contains("$ErrorActionPreference = 'Continue'"));
        assert!(!command.contains("pnpm list -g @scope/test-agent *> $null"));
    }
}
