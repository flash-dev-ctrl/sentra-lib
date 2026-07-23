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
        brew_package: None,
        brew_uninstall_package: None,
        winget_id: None,
        unix_files: &["\"$HOME/.local/bin/kimi\"", "\"$HOME/.kimi-code/bin/kimi\""],
        unix_dirs: &[],
        unix_config_files: &[],
        unix_config_dirs: &["\"$HOME/.kimi-code\""],
        windows_paths: &[
            "$env:USERPROFILE\\.local\\bin\\kimi.exe",
            "$env:USERPROFILE\\.local\\bin\\kimi.cmd",
            "$env:USERPROFILE\\.kimi-code\\bin\\kimi.exe",
            "$env:USERPROFILE\\.kimi-code\\bin\\kimi.cmd",
        ],
        windows_config_paths: &["$env:USERPROFILE\\.kimi-code"],
        powershell_error_action: "Stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_prefers_native_channel_then_verified_registry() {
        let plans = install_plans_for_platform(Platform::Linux, AgentInstallAction::Update);
        assert_eq!(plans[0].command_line(), "kimi upgrade");
        assert_eq!(
            plans[1].command_line(),
            "npm install -g --registry=https://registry.npmjs.org @moonshot-ai/kimi-code@latest"
        );
    }
}
