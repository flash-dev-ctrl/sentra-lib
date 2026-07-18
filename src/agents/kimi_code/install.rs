use crate::agents::install::{AgentInstallSpec, InstallCommandPlan, Platform};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    let mut plans = install_spec().plans_for_platform(platform, action);
    if action == AgentInstallAction::Update {
        plans.insert(
            0,
            InstallCommandPlan {
                program: "kimi",
                args: vec!["upgrade".to_string()],
                method: "kimi upgrade",
            },
        );
    }
    plans
}

pub(crate) fn uninstall_plan_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> InstallCommandPlan {
    install_spec().uninstall_plan_for_platform(platform, options)
}

fn install_spec() -> AgentInstallSpec {
    AgentInstallSpec {
        npm_package: Some("@moonshot-ai/kimi-code"),
        npm_update_package: Some("@moonshot-ai/kimi-code@latest"),
        npm_global_options: &[],
        pnpm_package: Some("@moonshot-ai/kimi-code"),
        pnpm_update_package: Some("@moonshot-ai/kimi-code@latest"),
        pnpm_global_options: &[],
        curl_command: Some("curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash"),
        powershell_command: Some(
            "Import-Module Microsoft.PowerShell.Utility; irm https://code.kimi.com/kimi-code/install.ps1 | iex",
        ),
        brew_package: None,
        brew_uninstall_package: None,
        unix_files: &["\"$HOME/.local/bin/kimi\"", "\"$HOME/.kimi-code/bin/kimi\""],
        unix_dirs: &[],
        unix_config_files: &[],
        unix_config_dirs: &["\"$HOME/.kimi-code\""],
        windows_paths: &[
            "$env:USERPROFILE\\.local\\bin\\kimi.exe",
            "$env:USERPROFILE\\.local\\bin\\kimi.cmd",
            "$env:USERPROFILE\\.local\\bin\\kimi.bat",
            "$env:USERPROFILE\\.kimi-code\\bin\\kimi.exe",
            "$env:USERPROFILE\\.kimi-code\\bin\\kimi.cmd",
            "$env:USERPROFILE\\.kimi-code\\bin\\kimi.bat",
        ],
        windows_config_paths: &["$env:USERPROFILE\\.kimi-code"],
        powershell_error_action: "Stop",
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
            "npm install -g @moonshot-ai/kimi-code"
        );
        assert_eq!(unix[1].command_line(), "pnpm add -g @moonshot-ai/kimi-code");
        assert_eq!(
            unix[2].command_line(),
            "sh -c curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash"
        );

        let windows = install_plans_for_platform(Platform::Windows, AgentInstallAction::Install);
        assert_eq!(
            windows[0].command_line(),
            "cmd /C npm install -g @moonshot-ai/kimi-code"
        );
        assert_eq!(
            windows[1].command_line(),
            "cmd /C pnpm add -g @moonshot-ai/kimi-code"
        );
        assert!(
            windows[2]
                .command_line()
                .contains("https://code.kimi.com/kimi-code/install.ps1")
        );
    }

    #[test]
    fn update_plan_uses_latest_package() {
        let unix = install_plans_for_platform(Platform::Unix, AgentInstallAction::Update);

        assert_eq!(unix[0].command_line(), "kimi upgrade");
        assert_eq!(
            unix[1].command_line(),
            "npm install -g @moonshot-ai/kimi-code@latest"
        );
        assert_eq!(
            unix[2].command_line(),
            "pnpm add -g @moonshot-ai/kimi-code@latest"
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
        assert!(unix.contains(".local/bin/kimi"));
        assert!(unix.contains(".kimi-code/bin/kimi"));
        assert!(unix.contains("npm uninstall -g @moonshot-ai/kimi-code"));
        assert!(unix.contains("pnpm remove -g @moonshot-ai/kimi-code"));
        assert!(unix.contains("rm -rf \"$HOME/.kimi-code\""));

        let windows = uninstall_plan_for_platform(
            Platform::Windows,
            AgentUninstallOptions {
                delete_config: true,
            },
        )
        .command_line();
        assert!(windows.contains(".local\\bin\\kimi.exe"));
        assert!(windows.contains(".local\\bin\\kimi.cmd"));
        assert!(windows.contains(".kimi-code\\bin\\kimi.exe"));
        assert!(windows.contains(".kimi-code\\bin\\kimi.cmd"));
        assert!(windows.contains("npm uninstall -g @moonshot-ai/kimi-code"));
        assert!(windows.contains("pnpm remove -g @moonshot-ai/kimi-code"));
        assert!(windows.contains("$env:USERPROFILE\\.kimi-code"));
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

        assert!(command.contains("npm uninstall -g @moonshot-ai/kimi-code"));
        assert!(command.contains("pnpm remove -g @moonshot-ai/kimi-code"));
        assert!(command.contains(".local/bin/kimi"));
        assert!(!command.contains("rm -rf \"$HOME/.kimi-code\""));
    }
}
