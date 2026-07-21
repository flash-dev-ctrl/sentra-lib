use crate::agents::install::{
    InstallCommandPlan, Platform, macos_app_uninstall_plan, macos_install_app_script, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Alibaba.QoderWork";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.qoderwork", "$env:APPDATA\\QoderWork"];
const MACOS_CONFIG: &[&str] = &[
    "\"$HOME/.qoderwork\"",
    "\"$HOME/Library/Application Support/QoderWork\"",
];
const MACOS_X64_URL: &str =
    "https://download.qoder.com/qoder-work/releases/latest/QoderWork-x64.dmg";
const MACOS_ARM64_URL: &str =
    "https://download.qoder.com/qoder-work/releases/latest/QoderWork-arm64.dmg";

pub(crate) fn install_plans_for_platform(
    platform: Platform,
    action: AgentInstallAction,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_install_plans_for_platform(platform, action, ID),
        Platform::MacOS => vec![macos_install_plan()],
        Platform::Linux => Vec::new(),
    }
}

pub(crate) fn uninstall_plans_for_platform(
    platform: Platform,
    options: AgentUninstallOptions,
) -> Vec<InstallCommandPlan> {
    match platform {
        Platform::Windows => winget_uninstall_plans_for_platform(platform, options, ID, CONFIG),
        Platform::MacOS => vec![macos_app_uninstall_plan("QoderWork", options, MACOS_CONFIG)],
        Platform::Linux => Vec::new(),
    }
}

fn macos_install_plan() -> InstallCommandPlan {
    let mut command = format!(
        r#"set -e
case "$(uname -m)" in arm64|aarch64) url="{MACOS_ARM64_URL}" ;; x86_64|amd64) url="{MACOS_X64_URL}" ;; *) echo "unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;; esac
temp_dir=$(mktemp -d)
mkdir "$temp_dir/mount"
trap 'hdiutil detach "$temp_dir/mount" >/dev/null 2>&1 || true; rm -rf "$temp_dir"' EXIT
curl -fsSL "$url" -o "$temp_dir/QoderWork.dmg"
hdiutil attach -nobrowse -readonly -mountpoint "$temp_dir/mount" "$temp_dir/QoderWork.dmg" >/dev/null
test -d "$temp_dir/mount/QoderWork.app"
codesign --verify --deep --strict "$temp_dir/mount/QoderWork.app"
spctl --assess --type execute "$temp_dir/mount/QoderWork.app""#
    );
    command.push('\n');
    command.push_str(&macos_install_app_script(
        "QoderWork",
        "\"$temp_dir/mount/QoderWork.app\"",
    ));
    sh_plan(command, "QoderWork official signed DMG")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_downloads_and_verifies_the_full_application() {
        let plans = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        let command = plans[0].command_line();
        assert!(command.contains("https://download.qoder.com/qoder-work/releases/latest/"));
        assert!(command.contains("QoderWork.app"));
        assert!(command.contains("codesign --verify --deep --strict"));
        assert!(command.contains("spctl --assess"));
        assert!(command.contains("/Applications/QoderWork.app"));
        assert!(command.contains("sudo ditto"));
        assert!(!command.contains("curl -fsSL $url |"));
    }

    #[test]
    fn linux_is_blocked_because_qoderwork_is_not_published_there() {
        assert!(
            install_plans_for_platform(Platform::Linux, AgentInstallAction::Install).is_empty()
        );
    }
}
