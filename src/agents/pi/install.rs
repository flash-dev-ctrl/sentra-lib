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
        npm_package: Some("@earendil-works/pi-coding-agent"),
        npm_update_package: Some("@earendil-works/pi-coding-agent@latest"),
        npm_global_options: &["--ignore-scripts", "--min-release-age=0"],
        pnpm_package: Some("@earendil-works/pi-coding-agent"),
        pnpm_update_package: Some("@earendil-works/pi-coding-agent@latest"),
        pnpm_global_options: &["--ignore-scripts"],
        curl_command: Some("curl -fsSL https://pi.dev/install.sh | sh"),
        powershell_command: Some("irm https://pi.dev/install.ps1 | iex"),
        brew_package: None,
        brew_uninstall_package: None,
        unix_files: &["\"$HOME/.local/bin/pi\""],
        unix_dirs: &["\"$HOME/.local/lib/node_modules/@earendil-works/pi-coding-agent\""],
        unix_config_files: &[],
        unix_config_dirs: &["\"$HOME/.pi\""],
        windows_paths: &[
            "$env:APPDATA\\npm\\pi.cmd",
            "$env:APPDATA\\npm\\pi.ps1",
            "$env:APPDATA\\npm\\node_modules\\@earendil-works\\pi-coding-agent",
            "$env:LOCALAPPDATA\\pnpm\\pi.cmd",
            "$env:LOCALAPPDATA\\pnpm\\pi.ps1",
        ],
        windows_config_paths: &["$env:USERPROFILE\\.pi"],
        powershell_error_action: "Continue",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_plan_supports_official_methods() {
        let unix = install_plans_for_platform(Platform::Unix, AgentInstallAction::Install);
        assert_eq!(
            unix[0].command_line(),
            "npm install -g --ignore-scripts --min-release-age=0 @earendil-works/pi-coding-agent"
        );
        assert_eq!(
            unix[1].command_line(),
            "pnpm add -g --ignore-scripts @earendil-works/pi-coding-agent"
        );
        assert_eq!(
            unix[2].command_line(),
            "sh -c curl -fsSL https://pi.dev/install.sh | sh"
        );

        let windows = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(
            windows[0].command_line(),
            "cmd /C npm install -g --ignore-scripts --min-release-age=0 @earendil-works/pi-coding-agent"
        );
        assert_eq!(
            windows[1].command_line(),
            "cmd /C pnpm add -g --ignore-scripts @earendil-works/pi-coding-agent"
        );
        assert!(
            windows[2]
                .command_line()
                .contains("https://pi.dev/install.ps1")
        );
    }

    #[test]
    fn update_plan_uses_latest_packages() {
        let unix = install_plans_for_platform(Platform::Unix, AgentInstallAction::Update);

        assert_eq!(
            unix[0].command_line(),
            "npm install -g --ignore-scripts --min-release-age=0 @earendil-works/pi-coding-agent@latest"
        );
        assert_eq!(
            unix[1].command_line(),
            "pnpm add -g --ignore-scripts @earendil-works/pi-coding-agent@latest"
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
        assert!(unix.contains(".local/bin/pi"));
        assert!(unix.contains(".local/lib/node_modules/@earendil-works/pi-coding-agent"));
        assert!(unix.contains("npm uninstall -g @earendil-works/pi-coding-agent"));
        assert!(unix.contains("pnpm remove -g @earendil-works/pi-coding-agent"));
        assert!(unix.contains("rm -rf \"$HOME/.pi\""));

        let windows = uninstall_plan_for_platform(
            Platform::Windows,
            AgentUninstallOptions {
                delete_config: true,
            },
        )
        .command_line();
        assert!(windows.contains("$env:APPDATA\\npm\\pi.cmd"));
        assert!(windows.contains("@earendil-works\\pi-coding-agent"));
        assert!(windows.contains("npm uninstall -g @earendil-works/pi-coding-agent"));
        assert!(windows.contains("pnpm remove -g @earendil-works/pi-coding-agent"));
        assert!(windows.contains("$env:USERPROFILE\\.pi"));
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

        assert!(command.contains("npm uninstall -g @earendil-works/pi-coding-agent"));
        assert!(command.contains("pnpm remove -g @earendil-works/pi-coding-agent"));
        assert!(!command.contains("$HOME/.pi"));
    }
}
