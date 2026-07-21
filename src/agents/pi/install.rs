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
                program: "pi",
                args: vec!["update".to_string(), "self".to_string()],
                method: "pi update self",
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
        npm_package: Some("@earendil-works/pi-coding-agent"),
        npm_update_package: Some("@earendil-works/pi-coding-agent@latest"),
        npm_global_options: &["--ignore-scripts", "--min-release-age=0"],
        pnpm_package: None,
        pnpm_update_package: None,
        pnpm_global_options: &[],
        brew_package: None,
        brew_uninstall_package: None,
        winget_id: None,
        unix_files: &["\"$HOME/.local/bin/pi\""],
        unix_dirs: &["\"$HOME/.local/lib/node_modules/@earendil-works/pi-coding-agent\""],
        unix_config_files: &[],
        unix_config_dirs: &["\"$HOME/.pi\""],
        windows_paths: &[
            "$env:APPDATA\\npm\\pi.cmd",
            "$env:APPDATA\\npm\\pi.ps1",
            "$env:APPDATA\\npm\\node_modules\\@earendil-works\\pi-coding-agent",
        ],
        windows_config_paths: &["$env:USERPROFILE\\.pi"],
        powershell_error_action: "Continue",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_uses_only_supported_official_npm_source() {
        let plans = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].command_line(),
            "npm install -g --registry=https://registry.npmjs.org --ignore-scripts --min-release-age=0 @earendil-works/pi-coding-agent"
        );
        assert!(!plans[0].command_line().contains("pnpm"));
    }

    #[test]
    fn update_prefers_native_install_channel() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);
        assert_eq!(plans[0].command_line(), "pi update self");
    }
}
