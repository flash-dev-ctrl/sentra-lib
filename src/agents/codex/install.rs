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
                program: "codex",
                args: vec!["update".to_string()],
                method: "codex update",
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
        npm_package: Some("@openai/codex"),
        npm_update_package: Some("@openai/codex@latest"),
        npm_global_options: &[],
        pnpm_package: Some("@openai/codex"),
        pnpm_update_package: Some("@openai/codex@latest"),
        pnpm_global_options: &[],
        brew_package: None,
        brew_uninstall_package: None,
        winget_id: None,
        unix_files: &["\"$HOME/.local/bin/codex\""],
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
    fn install_uses_verified_registry_without_remote_scripts() {
        let plans = install_plans_for_platform(Platform::Linux, AgentInstallAction::Install);
        assert_eq!(
            plans[0].command_line(),
            "npm install -g --registry=https://registry.npmjs.org @openai/codex"
        );
        assert!(
            plans
                .iter()
                .all(|plan| !plan.command_line().contains("curl"))
        );
    }

    #[test]
    fn unix_uninstall_stops_on_failure_and_preserves_config_by_default() {
        let plan = uninstall_plan_for_platform(
            Platform::Linux,
            AgentUninstallOptions {
                delete_config: false,
            },
        )
        .command_line();
        assert!(plan.contains("set -e"));
        assert!(!plan.contains("rm -rf \"$HOME/.codex\""));
    }

    #[test]
    fn update_prefers_native_install_channel() {
        let plans = install_plans_for_platform(Platform::Windows, AgentInstallAction::Update);
        assert_eq!(plans[0].command_line(), "codex update");
    }
}
