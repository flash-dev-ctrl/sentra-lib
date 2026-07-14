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
        npm_package: Some("@openai/codex"),
        npm_update_package: Some("@openai/codex@latest"),
        npm_global_options: &[],
        pnpm_package: Some("@openai/codex"),
        pnpm_update_package: Some("@openai/codex@latest"),
        pnpm_global_options: &[],
        curl_command: Some("curl -fsSL https://chatgpt.com/codex/install.sh | sh"),
        powershell_command: Some(
            "Import-Module Microsoft.PowerShell.Utility; irm https://chatgpt.com/codex/install.ps1 | iex",
        ),
        brew_package: None,
        brew_uninstall_package: None,
        unix_files: &[
            "\"$HOME/.local/bin/codex\"",
            "\"${CODEX_INSTALL_DIR:-$HOME/.local/bin}/codex\"",
        ],
        unix_dirs: &[],
        unix_config_files: &[],
        unix_config_dirs: &["\"$HOME/.codex\""],
        windows_paths: &["$env:LOCALAPPDATA\\Programs\\OpenAI\\Codex\\bin"],
        windows_config_paths: &["$env:USERPROFILE\\.codex"],
        powershell_error_action: "Stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_plan_supports_configured_methods() {
        let unix = install_plans_for_platform(Platform::Unix, AgentInstallAction::Install);
        assert_eq!(unix[0].command_line(), "npm install -g @openai/codex");
        assert_eq!(unix[1].command_line(), "pnpm add -g @openai/codex");
        assert_eq!(
            unix[2].command_line(),
            "sh -c curl -fsSL https://chatgpt.com/codex/install.sh | sh"
        );

        let windows = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(
            windows[0].command_line(),
            "cmd /C npm install -g @openai/codex"
        );
        assert_eq!(
            windows[1].command_line(),
            "cmd /C pnpm add -g @openai/codex"
        );
        let powershell = windows[2].command_line();

        assert!(powershell.contains("Import-Module Microsoft.PowerShell.Utility"));
        assert!(powershell.contains("https://chatgpt.com/codex/install.ps1"));
    }

    #[test]
    fn install_and_update_prefer_npm_then_fallbacks() {
        let unix_install_plans =
            install_plans_for_platform(Platform::Unix, AgentInstallAction::Install);
        assert_eq!(
            unix_install_plans[0].command_line(),
            "npm install -g @openai/codex"
        );
        assert_eq!(
            unix_install_plans[1].command_line(),
            "pnpm add -g @openai/codex"
        );
        assert_eq!(
            unix_install_plans[2].command_line(),
            "sh -c curl -fsSL https://chatgpt.com/codex/install.sh | sh"
        );

        let unix_update_plans =
            install_plans_for_platform(Platform::Unix, AgentInstallAction::Update);
        assert_eq!(
            unix_update_plans[0].command_line(),
            "npm install -g @openai/codex@latest"
        );
        assert_eq!(
            unix_update_plans[1].command_line(),
            "pnpm add -g @openai/codex@latest"
        );

        let install_plans =
            install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(
            install_plans[0].command_line(),
            "cmd /C npm install -g @openai/codex"
        );
        assert_eq!(
            install_plans[1].command_line(),
            "cmd /C pnpm add -g @openai/codex"
        );
        assert!(
            install_plans[2]
                .command_line()
                .contains("https://chatgpt.com/codex/install.ps1")
        );

        let update_plans =
            install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);
        assert_eq!(
            update_plans[0].command_line(),
            "cmd /C npm install -g @openai/codex@latest"
        );
        assert_eq!(
            update_plans[1].command_line(),
            "cmd /C pnpm add -g @openai/codex@latest"
        );
        assert!(
            update_plans[2]
                .command_line()
                .contains("https://chatgpt.com/codex/install.ps1")
        );
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
        assert!(unix.contains(".local/bin/codex"));
        assert!(unix.contains("rm -rf \"$HOME/.codex\""));
        assert!(unix.contains("@openai/codex"));
        assert!(unix.contains("pnpm remove -g @openai/codex"));

        let windows = uninstall_plan_for_platform(
            Platform::Windows,
            AgentUninstallOptions {
                delete_config: true,
            },
        )
        .command_line();
        assert!(windows.contains("LOCALAPPDATA"));
        assert!(windows.contains("$env:USERPROFILE\\.codex"));
        assert!(windows.contains("@openai/codex"));
        assert!(windows.contains("pnpm remove -g @openai/codex"));
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

        assert!(command.contains("@openai/codex"));
        assert!(command.contains("pnpm remove -g @openai/codex"));
        assert!(!command.contains("rm -rf \"$HOME/.codex\""));
    }
}
