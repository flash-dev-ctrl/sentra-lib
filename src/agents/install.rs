use std::process::{Command, Stdio};

use crate::interfaces::{
    AgentInstallAction, AgentInstallProgress, AgentInstallProgressStage, AgentInstallResult,
    AgentUninstallOptions,
};
use crate::{SentraError, SentraResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallableAgent {
    Antigravity,
    CodeBuddy,
    Coder,
    Codex,
    ClaudeCli,
    Cursor,
    KimiCode,
    Kiro,
    LingCode,
    Marvis,
    OpenCode,
    Pi,
    Qoder,
    QoderWork,
    Trae,
    VsCode,
    WorkBuddy,
}

impl InstallableAgent {
    fn name(self) -> &'static str {
        match self {
            Self::Antigravity => "antigravity",
            Self::CodeBuddy => "codebuddy",
            Self::Coder => "coder",
            Self::Codex => "codex",
            Self::ClaudeCli => "claude-cli",
            Self::Cursor => "cursor",
            Self::KimiCode => "kimi-code",
            Self::Kiro => "kiro",
            Self::LingCode => "lingcode",
            Self::Marvis => "marvis",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Qoder => "qoder",
            Self::QoderWork => "qoderwork",
            Self::Trae => "trae",
            Self::VsCode => "vscode",
            Self::WorkBuddy => "workbuddy",
        }
    }

    fn entry(self) -> &'static crate::agents::entries::AgentEntry {
        use crate::agents::entries::*;

        match self {
            Self::Antigravity => &ANTIGRAVITY_AGENT_ENTRY,
            Self::CodeBuddy => &CODEBUDDY_AGENT_ENTRY,
            Self::Coder => &CODER_AGENT_ENTRY,
            Self::Codex => &CODEX_AGENT_ENTRY,
            Self::ClaudeCli => &CLAUDE_CLI_AGENT_ENTRY,
            Self::Cursor => &CURSOR_AGENT_ENTRY,
            Self::KimiCode => &KIMI_CODE_AGENT_ENTRY,
            Self::Kiro => &KIRO_AGENT_ENTRY,
            Self::LingCode => &LINGCODE_AGENT_ENTRY,
            Self::Marvis => &MARVIS_AGENT_ENTRY,
            Self::OpenCode => &OPENCODE_AGENT_ENTRY,
            Self::Pi => &PI_AGENT_ENTRY,
            Self::Qoder => &QODER_AGENT_ENTRY,
            Self::QoderWork => &QODERWORK_AGENT_ENTRY,
            Self::Trae => &TRAE_AGENT_ENTRY,
            Self::VsCode => &VSCODE_AGENT_ENTRY,
            Self::WorkBuddy => &WORKBUDDY_AGENT_ENTRY,
        }
    }

    fn is_installed(self) -> SentraResult<bool> {
        let user_home = home::home_dir().ok_or_else(|| {
            SentraError::Message("could not determine current user home".to_string())
        })?;
        let entry = self.entry();
        let homes = entry
            .homes
            .iter()
            .map(|segments| {
                segments
                    .iter()
                    .fold(user_home.clone(), |home, segment| home.join(segment))
            })
            .collect::<Vec<_>>();
        let installed_at = |agent_home: &std::path::Path| match self {
            Self::Antigravity => {
                crate::agents::antigravity::is_install_target_installed(agent_home)
            }
            Self::Coder => crate::agents::coder::is_install_target_installed(agent_home),
            Self::Qoder => crate::agents::qoder::is_install_target_installed(agent_home),
            Self::VsCode => crate::agents::vscode::is_install_target_installed(agent_home),
            _ => (entry.is_installed)(entry.name, agent_home),
        };
        Ok(if homes.is_empty() {
            installed_at(&user_home)
        } else {
            homes.iter().any(|agent_home| installed_at(agent_home))
        })
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
    pub(crate) brew_package: Option<&'static str>,
    pub(crate) brew_uninstall_package: Option<&'static str>,
    pub(crate) winget_id: Option<&'static str>,
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
                Platform::MacOS | Platform::Linux => npm_plan(package, self.npm_global_options),
                Platform::Windows => windows_npm_plan(package, self.npm_global_options),
            });
        }
        if let Some(package) =
            package_for_action(self.pnpm_package, self.pnpm_update_package, action)
        {
            plans.push(match platform {
                Platform::MacOS | Platform::Linux => pnpm_plan(package, self.pnpm_global_options),
                Platform::Windows => windows_pnpm_plan(package, self.pnpm_global_options),
            });
        }

        match platform {
            Platform::MacOS | Platform::Linux => {
                if let Some(package) = self.brew_package {
                    plans.push(brew_plan(package, action));
                }
            }
            Platform::Windows => {
                if let Some(id) = self.winget_id {
                    plans.push(winget_plan(id, action));
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
            Platform::MacOS | Platform::Linux => sh_plan(
                self.unix_uninstall_script(options),
                "shell uninstall script",
            ),
            Platform::Windows => powershell_plan(
                self.windows_uninstall_script(options),
                if self.winget_id.is_some() {
                    "WinGet"
                } else {
                    "PowerShell uninstall script"
                },
            ),
        }
    }

    fn unix_uninstall_script(self, options: AgentUninstallOptions) -> String {
        let mut commands = vec!["set -e".to_string()];
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
        if let Some(id) = self.winget_id {
            commands.push(format!(
                "if (Get-Command winget -ErrorAction SilentlyContinue) {{ winget list --id {id} --exact --source winget --disable-interactivity *> $null; if ($LASTEXITCODE -eq 0) {{ winget uninstall --id {id} --exact --source winget --disable-interactivity; if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }} }} }}"
            ));
        }
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
    let mut verifier = || agent.is_installed();
    let action = install_action(&mut verifier)?;
    let plans = install_plans(agent, action)?;
    execute_agent_command(agent, action, plans, "install", &mut verifier, progress)
}

fn install_action(
    verifier: &mut dyn FnMut() -> SentraResult<bool>,
) -> SentraResult<AgentInstallAction> {
    Ok(if verifier()? {
        AgentInstallAction::Update
    } else {
        AgentInstallAction::Install
    })
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
    let mut verifier = || agent.is_installed();
    execute_agent_command(
        agent,
        action,
        uninstall_plans(agent, options)?,
        "uninstall",
        &mut verifier,
        progress,
    )
}

fn execute_agent_command(
    agent: InstallableAgent,
    action: AgentInstallAction,
    plans: Vec<InstallCommandPlan>,
    verb: &str,
    verifier: &mut dyn FnMut() -> SentraResult<bool>,
    mut progress: Option<&mut dyn FnMut(AgentInstallProgress)>,
) -> SentraResult<AgentInstallResult> {
    let mut errors = Vec::new();
    let total = plans.len();
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
        let output = match Command::new(plan.program)
            .args(&plan.args)
            .stdin(Stdio::inherit())
            .output()
        {
            Ok(output) => output,
            Err(err) => {
                errors.push(format!("`{}` could not start: {err}", plan.command_line()));
                continue;
            }
        };
        if output.status.success()
            || winget_update_not_applicable(action, &plan, output.status.code())
        {
            if let Some(reporter) = progress.as_deref_mut() {
                reporter(AgentInstallProgress {
                    agent: agent.name().to_string(),
                    action,
                    current: index + 1,
                    total,
                    method: agent.name().to_string(),
                    stage: AgentInstallProgressStage::Verifying,
                });
            }
            match verifier() {
                Ok(installed) if installed == (action != AgentInstallAction::Uninstall) => {
                    return Ok(AgentInstallResult {
                        agent: agent.name().to_string(),
                        action,
                        command: plan.command_line(),
                        exit_code: output.status.code(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    });
                }
                Ok(_) => errors.push(format!(
                    "`{}` exited successfully but verification still reports {} as {}",
                    plan.command_line(),
                    agent.name(),
                    if action == AgentInstallAction::Uninstall {
                        "installed"
                    } else {
                        "not installed"
                    }
                )),
                Err(err) => errors.push(format!(
                    "`{}` exited successfully but verification failed: {err}",
                    plan.command_line()
                )),
            }
            continue;
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

fn install_plans(
    agent: InstallableAgent,
    action: AgentInstallAction,
) -> SentraResult<Vec<InstallCommandPlan>> {
    let platform = platform()?;
    plans_or_unsupported(
        agent,
        platform,
        "install or update",
        install_plans_for_platform(agent, platform, action),
    )
}

fn install_plans_for_platform(
    agent: InstallableAgent,
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match agent {
        InstallableAgent::Antigravity => {
            crate::agents::antigravity::install_plans_for_platform(platform, action)
        }
        InstallableAgent::CodeBuddy => {
            crate::agents::codebuddy::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Coder => {
            crate::agents::coder::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Codex => {
            crate::agents::codex::install_plans_for_platform(platform, action)
        }
        InstallableAgent::ClaudeCli => {
            crate::agents::claude_cli::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Cursor => {
            crate::agents::cursor::install_plans_for_platform(platform, action)
        }
        InstallableAgent::KimiCode => {
            crate::agents::kimi_code::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Kiro => crate::agents::kiro::install_plans_for_platform(platform, action),
        InstallableAgent::LingCode | InstallableAgent::Marvis => Vec::new(),
        InstallableAgent::OpenCode => {
            crate::agents::opencode::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Pi => crate::agents::pi::install_plans_for_platform(platform, action),
        InstallableAgent::Qoder => {
            crate::agents::qoder::install_plans_for_platform(platform, action)
        }
        InstallableAgent::QoderWork => {
            crate::agents::qoderwork::install_plans_for_platform(platform, action)
        }
        InstallableAgent::Trae => crate::agents::trae::install_plans_for_platform(platform, action),
        InstallableAgent::VsCode => {
            crate::agents::vscode::install_plans_for_platform(platform, action)
        }
        InstallableAgent::WorkBuddy => {
            crate::agents::workbuddy::install_plans_for_platform(platform, action)
        }
    }
}

fn uninstall_plans(
    agent: InstallableAgent,
    options: AgentUninstallOptions,
) -> SentraResult<Vec<InstallCommandPlan>> {
    let platform = platform()?;
    plans_or_unsupported(
        agent,
        platform,
        "uninstall",
        uninstall_plans_for_platform(agent, platform, options),
    )
}

fn uninstall_plans_for_platform(
    agent: InstallableAgent,
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match agent {
        InstallableAgent::Antigravity => {
            crate::agents::antigravity::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::CodeBuddy => vec![crate::agents::codebuddy::uninstall_plan_for_platform(
            platform, options,
        )],
        InstallableAgent::Coder => {
            crate::agents::coder::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::Codex => {
            vec![crate::agents::codex::uninstall_plan_for_platform(
                platform, options,
            )]
        }
        InstallableAgent::ClaudeCli => {
            crate::agents::claude_cli::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::Cursor => {
            crate::agents::cursor::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::KimiCode => {
            vec![crate::agents::kimi_code::uninstall_plan_for_platform(
                platform, options,
            )]
        }
        InstallableAgent::Kiro => {
            crate::agents::kiro::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::LingCode | InstallableAgent::Marvis => Vec::new(),
        InstallableAgent::OpenCode => {
            crate::agents::opencode::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::Pi => vec![crate::agents::pi::uninstall_plan_for_platform(
            platform, options,
        )],
        InstallableAgent::Qoder => {
            crate::agents::qoder::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::QoderWork => {
            crate::agents::qoderwork::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::Trae => {
            crate::agents::trae::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::VsCode => {
            crate::agents::vscode::uninstall_plans_for_platform(platform, options)
        }
        InstallableAgent::WorkBuddy => {
            crate::agents::workbuddy::uninstall_plans_for_platform(platform, options)
        }
    }
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

pub(crate) fn curl_bash_plan(
    url: &'static str,
    installer_args: &'static [&'static str],
    method: &'static str,
) -> InstallCommandPlan {
    let mut args = vec![
        "-c".to_string(),
        r#"set -e
script=$(mktemp)
trap 'rm -f "$script"' EXIT
curl -fsSL "$1" -o "$script"
shift
bash "$script" "$@""#
            .to_string(),
        "sentra-installer".to_string(),
        url.to_string(),
    ];
    args.extend(installer_args.iter().map(|arg| (*arg).to_string()));
    InstallCommandPlan {
        program: "sh",
        args,
        method,
    }
}

pub(crate) fn npm_plan(
    package: &'static str,
    global_options: &'static [&'static str],
) -> InstallCommandPlan {
    let mut args = vec![
        "install".to_string(),
        "-g".to_string(),
        "--registry=https://registry.npmjs.org".to_string(),
    ];
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
    let mut args = vec![
        "add".to_string(),
        "-g".to_string(),
        "--registry=https://registry.npmjs.org".to_string(),
    ];
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
        "--registry=https://registry.npmjs.org".to_string(),
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
        "--registry=https://registry.npmjs.org".to_string(),
    ];
    args.extend(global_options.iter().map(|arg| (*arg).to_string()));
    args.push(package.to_string());
    InstallCommandPlan {
        program: "cmd",
        args,
        method: "npm",
    }
}

pub(crate) fn brew_plan(package: &'static str, action: AgentInstallAction) -> InstallCommandPlan {
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

pub(crate) fn brew_cask_plan(
    package: &'static str,
    action: AgentInstallAction,
) -> InstallCommandPlan {
    let verb = match action {
        AgentInstallAction::Install => "install",
        AgentInstallAction::Update => "upgrade",
        AgentInstallAction::Uninstall => "uninstall",
    };
    InstallCommandPlan {
        program: "brew",
        args: vec![verb.to_string(), "--cask".to_string(), package.to_string()],
        method: "Homebrew cask",
    }
}

pub(crate) fn brew_cask_uninstall_plan(
    package: &'static str,
    options: AgentUninstallOptions,
    config_paths: &'static [&'static str],
) -> InstallCommandPlan {
    let mut commands = vec![
        "set -e".to_string(),
        format!("brew uninstall --cask {package}"),
    ];
    if options.delete_config {
        push_shell_remove(&mut commands, "rm -rf", config_paths);
    }
    sh_plan(commands.join("; "), "Homebrew cask")
}

pub(crate) fn macos_install_app_script(app_name: &str, source: &str) -> String {
    format!(
        r#"target="$HOME/Applications/{app_name}.app"
if [ -d "/Applications/{app_name}.app" ]; then target="/Applications/{app_name}.app"; fi
if [ "$target" = "/Applications/{app_name}.app" ] && [ ! -w /Applications ]; then
  command -v sudo >/dev/null 2>&1 || {{ echo "administrator privileges are required to update $target" >&2; exit 1; }}
  sudo rm -rf "$target"
  sudo ditto {source} "$target"
else
  mkdir -p "$(dirname "$target")"
  rm -rf "$target"
  ditto {source} "$target"
fi"#
    )
}

pub(crate) fn macos_app_uninstall_plan(
    app_name: &str,
    options: AgentUninstallOptions,
    config_paths: &[&str],
) -> InstallCommandPlan {
    let mut command = format!(
        r#"set -e
rm -rf "$HOME/Applications/{app_name}.app"
if [ -e "/Applications/{app_name}.app" ]; then
  if [ -w /Applications ]; then rm -rf "/Applications/{app_name}.app"; elif command -v sudo >/dev/null 2>&1; then sudo rm -rf "/Applications/{app_name}.app"; else exit 1; fi
fi"#
    );
    if options.delete_config && !config_paths.is_empty() {
        command.push_str("\nrm -rf ");
        command.push_str(&config_paths.join(" "));
    }
    sh_plan(command, "macOS application uninstall")
}

pub(crate) fn linux_deb_rpm_plan(
    deb_x64_url: &'static str,
    rpm_x64_url: &'static str,
    deb_arm64_url: &'static str,
    rpm_arm64_url: &'static str,
    method: &'static str,
) -> InstallCommandPlan {
    let script = r#"set -e
case "$(uname -m)" in
  x86_64|amd64) deb_url=$1; rpm_url=$2 ;;
  aarch64|arm64) deb_url=$3; rpm_url=$4 ;;
  *) echo "unsupported Linux architecture: $(uname -m)" >&2; exit 1 ;;
esac
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
if [ "$(id -u)" -eq 0 ]; then
  elevate=
elif command -v sudo >/dev/null 2>&1; then
  elevate=sudo
else
  echo "root privileges or sudo are required to install this package" >&2
  exit 1
fi
if command -v apt-get >/dev/null 2>&1; then
  archive="$temp_dir/agent.deb"
  curl -fsSL "$deb_url" -o "$archive"
  $elevate apt-get install -y "$archive"
elif command -v dnf >/dev/null 2>&1; then
  archive="$temp_dir/agent.rpm"
  curl -fsSL "$rpm_url" -o "$archive"
  $elevate dnf install -y "$archive"
elif command -v yum >/dev/null 2>&1; then
  archive="$temp_dir/agent.rpm"
  curl -fsSL "$rpm_url" -o "$archive"
  $elevate yum install -y "$archive"
else
  echo "a supported deb or rpm package manager is required" >&2
  exit 1
fi"#;
    InstallCommandPlan {
        program: "sh",
        args: vec![
            "-c".to_string(),
            script.to_string(),
            "sentra-linux-package".to_string(),
            deb_x64_url.to_string(),
            rpm_x64_url.to_string(),
            deb_arm64_url.to_string(),
            rpm_arm64_url.to_string(),
        ],
        method,
    }
}

pub(crate) fn linux_package_uninstall_plan(
    package: &'static str,
    options: AgentUninstallOptions,
    config_paths: &'static [&'static str],
) -> InstallCommandPlan {
    let mut commands = vec![
        "set -e".to_string(),
        "if [ \"$(id -u)\" -eq 0 ]; then elevate=; elif command -v sudo >/dev/null 2>&1; then elevate=sudo; else echo \"root privileges or sudo are required to uninstall this package\" >&2; exit 1; fi".to_string(),
        format!(
            "if command -v dpkg-query >/dev/null 2>&1 && dpkg-query -W -f='${{Status}}' {package} 2>/dev/null | grep -q 'ok installed'; then $elevate apt-get remove -y {package}; elif command -v rpm >/dev/null 2>&1 && rpm -q {package} >/dev/null 2>&1; then if command -v dnf >/dev/null 2>&1; then $elevate dnf remove -y {package}; else $elevate yum remove -y {package}; fi; elif command -v snap >/dev/null 2>&1 && snap list {package} >/dev/null 2>&1; then $elevate snap remove {package}; else exit 1; fi"
        ),
    ];
    if options.delete_config {
        push_shell_remove(&mut commands, "rm -rf", config_paths);
    }
    sh_plan(commands.join("; "), "Linux package manager")
}

pub(crate) fn winget_plan(id: &'static str, action: AgentInstallAction) -> InstallCommandPlan {
    let verb = match action {
        AgentInstallAction::Install => "install",
        AgentInstallAction::Update => "upgrade",
        AgentInstallAction::Uninstall => "uninstall",
    };
    let mut args = vec![
        verb.to_string(),
        "--id".to_string(),
        id.to_string(),
        "--exact".to_string(),
        "--source".to_string(),
        "winget".to_string(),
        "--disable-interactivity".to_string(),
    ];
    if action != AgentInstallAction::Uninstall {
        args.extend([
            "--accept-package-agreements".to_string(),
            "--accept-source-agreements".to_string(),
        ]);
    }
    InstallCommandPlan {
        program: "winget",
        args,
        method: "WinGet",
    }
}

fn winget_update_not_applicable(
    action: AgentInstallAction,
    plan: &InstallCommandPlan,
    exit_code: Option<i32>,
) -> bool {
    const UPDATE_NOT_APPLICABLE: i32 = -1_978_335_189; // 0x8A15002B
    action == AgentInstallAction::Update
        && plan.program.eq_ignore_ascii_case("winget")
        && exit_code == Some(UPDATE_NOT_APPLICABLE)
}

pub(crate) fn winget_install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
    id: &'static str,
) -> Vec<InstallCommandPlan> {
    if platform == Platform::Windows {
        vec![winget_plan(id, action)]
    } else {
        Vec::new()
    }
}

pub(crate) fn winget_uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
    id: &'static str,
    windows_config_paths: &'static [&'static str],
) -> Vec<InstallCommandPlan> {
    if platform != Platform::Windows {
        return Vec::new();
    }
    vec![
        AgentInstallSpec {
            npm_package: None,
            npm_update_package: None,
            npm_global_options: &[],
            pnpm_package: None,
            pnpm_update_package: None,
            pnpm_global_options: &[],
            brew_package: None,
            brew_uninstall_package: None,
            winget_id: Some(id),
            unix_files: &[],
            unix_dirs: &[],
            unix_config_files: &[],
            unix_config_dirs: &[],
            windows_paths: &[],
            windows_config_paths,
            powershell_error_action: "Stop",
        }
        .uninstall_plan_for_platform(platform, options),
    ]
}

fn plans_or_unsupported(
    agent: InstallableAgent,
    platform: Platform,
    operation: &str,
    plans: Vec<InstallCommandPlan>,
) -> SentraResult<Vec<InstallCommandPlan>> {
    if plans.is_empty() {
        let reason = match (agent, platform) {
            (InstallableAgent::LingCode, _) => {
                "the vendor's official distribution source is unavailable"
            }
            (InstallableAgent::Marvis, _) => {
                "no official package source matches the detected product"
            }
            (InstallableAgent::QoderWork, Platform::Linux) => {
                "the vendor does not publish QoderWork for Linux"
            }
            (InstallableAgent::WorkBuddy, Platform::Linux) => {
                "the vendor does not publish WorkBuddy for Linux"
            }
            _ => "no verified package source is configured for this platform",
        };
        return Err(SentraError::Message(format!(
            "cannot {operation} {} automatically on {}: {reason}; use the vendor's official installation instructions",
            agent.name(),
            platform.name()
        )));
    }
    Ok(plans)
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
        "$sentraRemovePaths = @({literal_paths}) | Where-Object {{ -not [string]::IsNullOrWhiteSpace($_) }}; if ($sentraRemovePaths) {{ Remove-Item -LiteralPath $sentraRemovePaths -Recurse -Force -ErrorAction SilentlyContinue }}"
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
    Windows,
    MacOS,
    Linux,
}

impl Platform {
    fn name(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::MacOS => "macOS",
            Self::Linux => "Linux",
        }
    }
}

fn platform() -> SentraResult<Platform> {
    platform_for_os(std::env::consts::OS)
}

fn platform_for_os(os: &str) -> SentraResult<Platform> {
    match os {
        "windows" => Ok(Platform::Windows),
        "macos" => Ok(Platform::MacOS),
        "linux" => Ok(Platform::Linux),
        _ => Err(SentraError::Message(format!(
            "automatic agent installation is not supported on {os}"
        ))),
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
            &mut || Ok(true),
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
            &mut || Ok(true),
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
            &mut || Ok(true),
            Some(&mut |event| progress.push(event)),
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
        assert_eq!(progress.len(), 3);
        assert_eq!(progress[0].current, 1);
        assert_eq!(progress[0].total, 2);
        assert_eq!(progress[0].method, "missing test installer");
        assert_eq!(progress[0].stage, AgentInstallProgressStage::Trying);
        assert_eq!(progress[1].current, 2);
        assert_eq!(progress[1].total, 2);
        assert_eq!(progress[1].method, "rustc test installer");
        assert_eq!(progress[1].stage, AgentInstallProgressStage::Trying);
        assert_eq!(progress[2].current, 2);
        assert_eq!(progress[2].total, 2);
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
            &mut || Ok(true),
            Some(&mut |event| progress.push(event)),
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
        assert_eq!(progress.len(), 2);
        assert_eq!(progress[0].current, 1);
        assert_eq!(progress[0].total, 1);
        assert_eq!(progress[0].method, "rustc test installer");
        assert_eq!(progress[0].stage, AgentInstallProgressStage::Trying);
        assert_eq!(progress[1].current, 1);
        assert_eq!(progress[1].total, 1);
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
            brew_package: None,
            brew_uninstall_package: None,
            winget_id: None,
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

    #[test]
    fn successful_exit_with_failed_probe_continues_to_fallback() {
        let mut installed = [false, true].into_iter();
        let result = execute_agent_command(
            InstallableAgent::Codex,
            AgentInstallAction::Install,
            vec![rustc_plan("first"), rustc_plan("fallback")],
            "install",
            &mut || Ok(installed.next().unwrap()),
            None,
        )
        .unwrap();

        assert_eq!(result.command, "rustc --version");
        assert_eq!(installed.next(), None);
    }

    #[test]
    fn installed_probe_selects_update_without_relying_on_path() {
        assert_eq!(
            install_action(&mut || Ok(true)).unwrap(),
            AgentInstallAction::Update
        );
        assert_eq!(
            install_action(&mut || Ok(false)).unwrap(),
            AgentInstallAction::Install
        );
    }

    #[test]
    fn uninstall_requires_probe_to_report_absent() {
        let mut installed = [true, false].into_iter();
        let result = execute_agent_command(
            InstallableAgent::Codex,
            AgentInstallAction::Uninstall,
            vec![rustc_plan("first"), rustc_plan("fallback")],
            "uninstall",
            &mut || Ok(installed.next().unwrap()),
            None,
        )
        .unwrap();

        assert_eq!(result.action, AgentInstallAction::Uninstall);
        assert_eq!(installed.next(), None);
    }

    #[test]
    fn winget_targets_use_exact_community_source() {
        for (agent, id) in [
            (InstallableAgent::Antigravity, "Google.AntigravityCLI"),
            (InstallableAgent::Coder, "Coder.Coder"),
            (InstallableAgent::Cursor, "Anysphere.Cursor"),
            (InstallableAgent::Kiro, "Amazon.Kiro"),
            (InstallableAgent::Qoder, "Alibaba.Qoder"),
            (InstallableAgent::QoderWork, "Alibaba.QoderWork"),
            (InstallableAgent::Trae, "ByteDance.Trae"),
            (InstallableAgent::VsCode, "Microsoft.VisualStudioCode"),
            (InstallableAgent::WorkBuddy, "Tencent.WorkBuddy"),
        ] {
            let plans =
                install_plans_for_platform(agent, Platform::Windows, AgentInstallAction::Install);
            let command = plans[0].command_line();
            assert!(
                command.contains(&format!("--id {id} --exact --source winget")),
                "{command}"
            );
        }
    }

    #[test]
    fn winget_no_applicable_update_is_verified_as_success() {
        let plan = winget_plan("Vendor.Agent", AgentInstallAction::Update);

        assert!(winget_update_not_applicable(
            AgentInstallAction::Update,
            &plan,
            Some(-1_978_335_189),
        ));
        assert!(!winget_update_not_applicable(
            AgentInstallAction::Install,
            &plan,
            Some(-1_978_335_189),
        ));
    }

    #[test]
    fn claude_uninstall_skips_absent_winget_package_before_legacy_cleanup() {
        let command = uninstall_plans_for_platform(
            InstallableAgent::ClaudeCli,
            Platform::Windows,
            AgentUninstallOptions::default(),
        )[0]
        .command_line();

        assert!(command.contains("winget list --id Anthropic.ClaudeCode --exact"));
        assert!(command.contains("npm uninstall -g @anthropic-ai/claude-code"));
        assert!(command.find("winget list").unwrap() < command.find("npm uninstall").unwrap());
    }

    #[test]
    fn windows_force_removes_roaming_desktop_configuration() {
        for agent in [
            InstallableAgent::Cursor,
            InstallableAgent::Kiro,
            InstallableAgent::Qoder,
            InstallableAgent::QoderWork,
            InstallableAgent::Trae,
            InstallableAgent::VsCode,
            InstallableAgent::WorkBuddy,
        ] {
            let command = uninstall_plans_for_platform(
                agent,
                Platform::Windows,
                AgentUninstallOptions {
                    delete_config: true,
                },
            )[0]
            .command_line();
            assert!(
                command.contains("$env:APPDATA"),
                "{}: {command}",
                agent.name()
            );
        }
    }

    #[test]
    fn blocked_sources_return_specific_errors() {
        for agent in [InstallableAgent::LingCode, InstallableAgent::Marvis] {
            let error = plans_or_unsupported(agent, Platform::Windows, "install", Vec::new())
                .unwrap_err()
                .to_string();
            assert!(error.contains("official"), "{error}");
        }
    }

    #[test]
    fn safe_curl_plan_downloads_before_executing() {
        let plan = curl_bash_plan(
            "https://vendor.example/install.sh",
            &["--stable"],
            "test installer",
        );
        let command = plan.command_line();

        assert!(command.contains("script=$(mktemp)"));
        assert!(command.contains("curl -fsSL \"$1\" -o \"$script\""));
        assert!(command.contains("bash \"$script\" \"$@\""));
        assert!(!command.contains("install.sh |"));
    }

    #[test]
    fn proprietary_targets_have_an_explicit_platform_matrix() {
        let macos_supported = [
            InstallableAgent::Antigravity,
            InstallableAgent::ClaudeCli,
            InstallableAgent::Coder,
            InstallableAgent::Cursor,
            InstallableAgent::Kiro,
            InstallableAgent::Qoder,
            InstallableAgent::QoderWork,
            InstallableAgent::Trae,
            InstallableAgent::VsCode,
            InstallableAgent::WorkBuddy,
        ];
        let linux_supported = [
            InstallableAgent::Antigravity,
            InstallableAgent::ClaudeCli,
            InstallableAgent::Coder,
            InstallableAgent::Cursor,
            InstallableAgent::Kiro,
            InstallableAgent::Qoder,
            InstallableAgent::Trae,
            InstallableAgent::VsCode,
        ];

        for agent in macos_supported {
            assert!(
                !install_plans_for_platform(agent, Platform::MacOS, AgentInstallAction::Install)
                    .is_empty(),
                "{} should support macOS",
                agent.name()
            );
            assert!(
                !uninstall_plans_for_platform(
                    agent,
                    Platform::MacOS,
                    AgentUninstallOptions::default(),
                )
                .is_empty(),
                "{} should support macOS uninstall",
                agent.name()
            );
        }
        for agent in linux_supported {
            assert!(
                !install_plans_for_platform(agent, Platform::Linux, AgentInstallAction::Install)
                    .is_empty(),
                "{} should support Linux",
                agent.name()
            );
            assert!(
                !uninstall_plans_for_platform(
                    agent,
                    Platform::Linux,
                    AgentUninstallOptions::default(),
                )
                .is_empty(),
                "{} should support Linux uninstall",
                agent.name()
            );
        }
        for (agent, platform) in [
            (InstallableAgent::QoderWork, Platform::Linux),
            (InstallableAgent::WorkBuddy, Platform::Linux),
        ] {
            assert!(
                install_plans_for_platform(agent, platform, AgentInstallAction::Install).is_empty(),
                "{} should be blocked on {}",
                agent.name(),
                platform.name()
            );
            assert!(
                uninstall_plans_for_platform(agent, platform, AgentUninstallOptions::default(),)
                    .is_empty(),
                "{} uninstall should be blocked on {}",
                agent.name(),
                platform.name()
            );
        }
    }

    #[test]
    fn unknown_operating_system_is_not_treated_as_linux() {
        let error = platform_for_os("freebsd").unwrap_err().to_string();

        assert!(error.contains("freebsd"));
        assert!(error.contains("not supported"));
    }

    fn rustc_plan(method: &'static str) -> InstallCommandPlan {
        InstallCommandPlan {
            program: "rustc",
            args: vec!["--version".to_string()],
            method,
        }
    }
}
