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
                program: "codebuddy",
                args: vec!["update".to_string()],
                method: "codebuddy update",
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
        npm_package: Some("@tencent-ai/codebuddy-code"),
        npm_update_package: Some("@tencent-ai/codebuddy-code@latest"),
        npm_global_options: &[],
        pnpm_package: None,
        pnpm_update_package: None,
        pnpm_global_options: &[],
        brew_package: None,
        brew_uninstall_package: None,
        winget_id: None,
        unix_files: &[],
        unix_dirs: &[],
        unix_config_files: &[],
        unix_config_dirs: &["\"$HOME/.codebuddy\""],
        windows_paths: &[],
        windows_config_paths: &["$env:USERPROFILE\\.codebuddy"],
        powershell_error_action: "Stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_uses_official_npm_registry() {
        let plan = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        assert_eq!(
            plan[0].command_line(),
            "npm install -g --registry=https://registry.npmjs.org @tencent-ai/codebuddy-code"
        );
    }

    #[test]
    fn update_prefers_native_install_channel() {
        let plan = install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);
        assert_eq!(plan[0].command_line(), "codebuddy update");
    }
}
