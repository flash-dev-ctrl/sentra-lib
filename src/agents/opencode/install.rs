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
        npm_package: Some("opencode-ai"),
        npm_update_package: Some("opencode-ai@latest"),
        npm_global_options: &[],
        pnpm_package: Some("opencode-ai"),
        pnpm_update_package: Some("opencode-ai@latest"),
        pnpm_global_options: &[],
        curl_command: Some("curl -fsSL https://opencode.ai/install | bash"),
        powershell_command: None,
        brew_package: Some("anomalyco/tap/opencode"),
        brew_uninstall_package: Some("opencode"),
        unix_files: &[],
        unix_dirs: &[],
        unix_config_files: &[],
        unix_config_dirs: &[
            "\"$HOME/.opencode\"",
            "\"$HOME/.config/opencode\"",
            "\"$HOME/.local/share/opencode\"",
        ],
        windows_paths: &[],
        windows_config_paths: &[
            "$env:USERPROFILE\\.opencode",
            "$env:USERPROFILE\\.config\\opencode",
            "$env:USERPROFILE\\.local\\share\\opencode",
        ],
        powershell_error_action: "Continue",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_text(plan: InstallCommandPlan) -> String {
        plan.command_line()
    }

    #[test]
    fn plan_supports_official_installers() {
        let unix = install_plans_for_platform(Platform::Unix, AgentInstallAction::Install);
        assert_eq!(unix[0].command_line(), "npm install -g opencode-ai");
        assert_eq!(unix[1].command_line(), "pnpm add -g opencode-ai");
        assert_eq!(
            unix[2].command_line(),
            "brew install anomalyco/tap/opencode"
        );
        assert_eq!(
            unix[3].command_line(),
            "sh -c curl -fsSL https://opencode.ai/install | bash"
        );

        let windows = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(
            windows[0].command_line(),
            "cmd /C npm install -g opencode-ai"
        );
        assert_eq!(windows[1].command_line(), "cmd /C pnpm add -g opencode-ai");
    }

    #[test]
    fn uninstall_plan_removes_user_data() {
        let unix = command_text(uninstall_plan_for_platform(
            Platform::Unix,
            AgentUninstallOptions {
                delete_config: true,
            },
        ));
        assert!(unix.contains("npm uninstall -g opencode-ai"));
        assert!(unix.contains("pnpm remove -g opencode-ai"));
        assert!(unix.contains("brew uninstall opencode"));
        assert!(unix.contains("rm -rf \"$HOME/.opencode\""));
        assert!(unix.contains("\"$HOME/.config/opencode\""));
        assert!(unix.contains("\"$HOME/.local/share/opencode\""));

        let windows = command_text(uninstall_plan_for_platform(
            Platform::Windows,
            AgentUninstallOptions {
                delete_config: true,
            },
        ));
        assert!(windows.contains("npm uninstall -g opencode-ai"));
        assert!(windows.contains("pnpm remove -g opencode-ai"));
        assert!(windows.contains("$ErrorActionPreference = 'Continue'"));
        assert!(windows.ends_with("exit 0"));
        assert!(windows.contains("$env:USERPROFILE\\.opencode"));
        assert!(windows.contains("$env:USERPROFILE\\.config\\opencode"));
        assert!(windows.contains("$env:USERPROFILE\\.local\\share\\opencode"));
    }

    #[test]
    fn uninstall_plan_preserves_config_when_requested() {
        let command = command_text(uninstall_plan_for_platform(
            Platform::Unix,
            AgentUninstallOptions {
                delete_config: false,
            },
        ));

        assert!(command.contains("npm uninstall -g opencode-ai"));
        assert!(command.contains("pnpm remove -g opencode-ai"));
        assert!(!command.contains("$HOME/.opencode"));
        assert!(!command.contains("$HOME/.config/opencode"));
        assert!(!command.contains("$HOME/.local/share/opencode"));
    }
}
