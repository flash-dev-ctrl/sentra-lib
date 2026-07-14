use crate::agents::install::{AgentInstallSpec, InstallCommandPlan, Platform};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    install_spec().plans_for_platform(platform, action)
}

pub(crate) fn uninstall_plan_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> InstallCommandPlan {
    install_spec().uninstall_plan_for_platform(platform, options)
}

fn install_spec() -> AgentInstallSpec {
    AgentInstallSpec {
        npm_package: Some("@anthropic-ai/claude-code"),
        npm_update_package: Some("@anthropic-ai/claude-code@latest"),
        npm_global_options: &[],
        pnpm_package: Some("@anthropic-ai/claude-code"),
        pnpm_update_package: Some("@anthropic-ai/claude-code@latest"),
        pnpm_global_options: &[],
        curl_command: Some("curl -fsSL https://claude.ai/install.sh | bash"),
        powershell_command: Some(
            "Import-Module Microsoft.PowerShell.Utility; irm https://claude.ai/install.ps1 | iex",
        ),
        brew_package: None,
        brew_uninstall_package: None,
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
    fn windows_install_prefers_package_managers_then_powershell() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);

        assert_eq!(plans.len(), 3);
        assert_eq!(
            plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code"
        );
        assert_eq!(
            plans[1].command_line(),
            "cmd /C pnpm add -g @anthropic-ai/claude-code"
        );
        assert!(
            plans[2]
                .command_line()
                .contains("https://claude.ai/install.ps1")
        );
    }

    #[test]
    fn windows_update_uses_package_managers_then_powershell() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);

        assert_eq!(
            plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code@latest"
        );
        assert_eq!(
            plans[1].command_line(),
            "cmd /C pnpm add -g @anthropic-ai/claude-code@latest"
        );
    }

    #[test]
    fn install_and_update_prefer_npm_then_fallbacks() {
        let unix_install_plans =
            install_plans_for_platform(Platform::Unix, AgentInstallAction::Install);
        assert_eq!(
            unix_install_plans[0].command_line(),
            "npm install -g @anthropic-ai/claude-code"
        );
        assert_eq!(
            unix_install_plans[1].command_line(),
            "pnpm add -g @anthropic-ai/claude-code"
        );
        assert_eq!(
            unix_install_plans[2].command_line(),
            "sh -c curl -fsSL https://claude.ai/install.sh | bash"
        );

        let unix_update_plans =
            install_plans_for_platform(Platform::Unix, AgentInstallAction::Update);
        assert_eq!(
            unix_update_plans[0].command_line(),
            "npm install -g @anthropic-ai/claude-code@latest"
        );
        assert_eq!(
            unix_update_plans[1].command_line(),
            "pnpm add -g @anthropic-ai/claude-code@latest"
        );

        let install_plans =
            install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(
            install_plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code"
        );
        assert_eq!(
            install_plans[1].command_line(),
            "cmd /C pnpm add -g @anthropic-ai/claude-code"
        );

        let update_plans =
            install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);
        assert_eq!(
            update_plans[0].command_line(),
            "cmd /C npm install -g @anthropic-ai/claude-code@latest"
        );
        assert_eq!(
            update_plans[1].command_line(),
            "cmd /C pnpm add -g @anthropic-ai/claude-code@latest"
        );
    }

    #[test]
    fn plan_supports_unix_and_windows_installers() {
        let unix = install_plans_for_platform(Platform::Unix, AgentInstallAction::Install);
        assert_eq!(
            unix[2].command_line(),
            "sh -c curl -fsSL https://claude.ai/install.sh | bash"
        );
        let windows = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert!(windows[2].command_line().contains("powershell -NoProfile -ExecutionPolicy Bypass -Command Import-Module Microsoft.PowerShell.Utility; irm https://claude.ai/install.ps1 | iex"));
    }

    #[test]
    fn uninstall_plan_removes_user_data() {
        let unix = uninstall_plan_for_platform(
            Platform::Unix,
            AgentUninstallOptions {
                delete_config: true,
            },
        )
        .command_line();
        assert!(unix.contains(".local/bin/claude"));
        assert!(unix.contains(".local/share/claude"));
        assert!(unix.contains("npm uninstall -g @anthropic-ai/claude-code"));
        assert!(unix.contains("pnpm remove -g @anthropic-ai/claude-code"));
        assert!(unix.contains("\"$HOME/.claude.json\""));
        assert!(unix.contains("\"$HOME/.claude\""));

        let windows = uninstall_plan_for_platform(
            Platform::Windows,
            AgentUninstallOptions {
                delete_config: true,
            },
        )
        .command_line();
        assert!(windows.contains(".local\\bin\\claude.exe"));
        assert!(windows.contains(".local\\share\\claude"));
        assert!(windows.contains("npm uninstall -g @anthropic-ai/claude-code"));
        assert!(windows.contains("pnpm remove -g @anthropic-ai/claude-code"));
        assert!(windows.ends_with("exit 0"));
        assert!(windows.contains("$env:USERPROFILE\\.claude"));
        assert!(windows.contains("$env:USERPROFILE\\.claude.json"));
    }

    #[test]
    fn uninstall_plan_preserves_config_when_requested() {
        let command = uninstall_plan_for_platform(
            Platform::Unix,
            AgentUninstallOptions {
                delete_config: false,
            },
        )
        .command_line();

        assert!(command.contains("npm uninstall -g @anthropic-ai/claude-code"));
        assert!(command.contains("pnpm remove -g @anthropic-ai/claude-code"));
        assert!(command.contains("rm -rf \"$HOME/.local/share/claude\""));
        assert!(!command.contains("\"$HOME/.claude.json\""));
        assert!(!command.contains("\"$HOME/.claude\""));
    }
}
