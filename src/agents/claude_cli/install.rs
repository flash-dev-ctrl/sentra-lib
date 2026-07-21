use crate::agents::install::{
    AgentInstallSpec, InstallCommandPlan, Platform, brew_cask_plan, brew_cask_uninstall_plan,
    curl_bash_plan, powershell_plan, winget_install_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const MACOS_CONFIG: &[&str] = &["\"$HOME/.claude\"", "\"$HOME/.claude.json\""];

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    let mut plans = match platform {
        Platform::Windows => {
            let mut plans =
                winget_install_plans_for_platform(platform, action, "Anthropic.ClaudeCode");
            plans.push(windows_native_install_plan());
            plans
        }
        Platform::MacOS => vec![
            brew_cask_plan("claude-code", action),
            curl_bash_plan(
                "https://claude.ai/install.sh",
                &[],
                "Anthropic native installer",
            ),
        ],
        Platform::Linux => vec![curl_bash_plan(
            "https://claude.ai/install.sh",
            &[],
            "Anthropic native installer",
        )],
    };
    if action == AgentInstallAction::Update {
        plans.insert(
            0,
            InstallCommandPlan {
                program: "claude",
                args: vec!["update".to_string()],
                method: "claude update",
            },
        );
    }
    plans
}

fn windows_native_install_plan() -> InstallCommandPlan {
    powershell_plan(
        "$ErrorActionPreference = 'Stop'; $script = Join-Path ([System.IO.Path]::GetTempPath()) ('claude-install-' + [System.Guid]::NewGuid().ToString('N') + '.ps1'); try { Invoke-WebRequest -UseBasicParsing -Uri 'https://claude.ai/install.ps1' -OutFile $script; & $script; if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { exit $LASTEXITCODE } } finally { Remove-Item -LiteralPath $script -Force -ErrorAction SilentlyContinue }",
        "Anthropic native installer",
    )
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    let native = install_spec().uninstall_plan_for_platform(platform, options);
    if platform == Platform::MacOS {
        vec![
            brew_cask_uninstall_plan("claude-code", options, MACOS_CONFIG),
            native,
        ]
    } else {
        vec![native]
    }
}

fn install_spec() -> AgentInstallSpec {
    AgentInstallSpec {
        // Retained only to remove legacy package-manager installations.
        npm_package: Some("@anthropic-ai/claude-code"),
        npm_update_package: None,
        npm_global_options: &[],
        pnpm_package: Some("@anthropic-ai/claude-code"),
        pnpm_update_package: None,
        pnpm_global_options: &[],
        brew_package: None,
        brew_uninstall_package: None,
        winget_id: Some("Anthropic.ClaudeCode"),
        unix_files: &["\"$HOME/.local/bin/claude\""],
        unix_dirs: &["\"$HOME/.local/share/claude\""],
        unix_config_files: &["\"$HOME/.claude.json\""],
        unix_config_dirs: &["\"$HOME/.claude\""],
        windows_paths: &[
            "$env:USERPROFILE\\.local\\bin\\claude.exe",
            "$env:USERPROFILE\\.local\\share\\claude",
        ],
        windows_config_paths: &[
            "$env:USERPROFILE\\.claude",
            "$env:USERPROFILE\\.claude.json",
        ],
        powershell_error_action: "Stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_install_uses_exact_winget_source() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(plans.len(), 2);
        let command = plans[0].command_line();
        assert!(command.contains("--id Anthropic.ClaudeCode --exact --source winget"));
        assert!(
            plans[1]
                .command_line()
                .contains("https://claude.ai/install.ps1")
        );
        assert!(!plans[1].command_line().contains("| iex"));
    }

    #[test]
    fn update_prefers_native_channel() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);
        assert_eq!(plans[0].command_line(), "claude update");
    }

    #[test]
    fn macos_and_linux_use_official_non_piped_sources() {
        let macos = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        assert_eq!(macos[0].command_line(), "brew install --cask claude-code");

        let linux = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        let command = linux[0].command_line();
        assert!(command.contains("https://claude.ai/install.sh"));
        assert!(command.contains("curl -fsSL \"$1\" -o \"$script\""));
        assert!(!command.contains("curl -fsSL https://claude.ai/install.sh |"));
    }
}
