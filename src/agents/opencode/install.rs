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
                program: "opencode",
                args: vec!["upgrade".to_string()],
                method: "opencode upgrade",
            },
        );
    }
    plans
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    let mut args = vec!["uninstall".to_string(), "--force".to_string()];
    if !options.delete_config {
        args.extend(["--keep-config".to_string(), "--keep-data".to_string()]);
    }
    vec![
        InstallCommandPlan {
            program: "opencode",
            args,
            method: "opencode uninstall",
        },
        install_spec().uninstall_plan_for_platform(platform, options),
    ]
}

fn install_spec() -> AgentInstallSpec {
    AgentInstallSpec {
        npm_package: Some("opencode-ai"),
        npm_update_package: Some("opencode-ai@latest"),
        npm_global_options: &[],
        pnpm_package: Some("opencode-ai"),
        pnpm_update_package: Some("opencode-ai@latest"),
        pnpm_global_options: &[],
        brew_package: Some("anomalyco/tap/opencode"),
        brew_uninstall_package: Some("opencode"),
        winget_id: None,
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

    #[test]
    fn update_prefers_native_install_channel() {
        let plans = install_plans_for_platform(Platform::Linux, AgentInstallAction::Update);
        assert_eq!(plans[0].command_line(), "opencode upgrade");
    }

    #[test]
    fn native_uninstall_preserves_config_and_data_when_requested() {
        let plans = uninstall_plans_for_platform(
            Platform::Linux,
            AgentUninstallOptions {
                delete_config: false,
            },
        );
        assert_eq!(
            plans[0].command_line(),
            "opencode uninstall --force --keep-config --keep-data"
        );
        assert!(plans[1].command_line().contains("set -e"));
    }
}
