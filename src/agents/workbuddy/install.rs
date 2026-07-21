use crate::agents::install::{
    InstallCommandPlan, Platform, macos_app_uninstall_plan, macos_install_app_script, sh_plan,
    winget_install_plans_for_platform, winget_uninstall_plans_for_platform,
};
use crate::interfaces::{AgentInstallAction, AgentUninstallOptions};

const ID: &str = "Tencent.WorkBuddy";
const CONFIG: &[&str] = &["$env:USERPROFILE\\.workbuddy", "$env:APPDATA\\WorkBuddy"];
const MACOS_CONFIG: &[&str] = &[
    "\"$HOME/.workbuddy\"",
    "\"$HOME/Library/Application Support/WorkBuddy\"",
];

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
        Platform::MacOS => vec![macos_app_uninstall_plan("WorkBuddy", options, MACOS_CONFIG)],
        Platform::Linux => Vec::new(),
    }
}

fn macos_install_plan() -> InstallCommandPlan {
    let mut command = r#"set -e
case "$(uname -m)" in arm64|aarch64) platform=workbuddy-darwin-arm64 ;; x86_64|amd64) platform=workbuddy-darwin-x64 ;; *) echo "unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;; esac
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
curl -fsSL "https://copilot.tencent.com/v2/update?platform=$platform" -o "$temp_dir/release.json"
url=$(plutil -extract url raw -o - "$temp_dir/release.json")
expected_sha256=$(plutil -extract sha256hash raw -o - "$temp_dir/release.json")
case "$url" in https://download.codebuddy.cn/workbuddy/*) ;; *) echo "WorkBuddy API returned an untrusted download URL" >&2; exit 1 ;; esac
curl -fsSL "$url" -o "$temp_dir/WorkBuddy.zip"
actual_sha256=$(shasum -a 256 "$temp_dir/WorkBuddy.zip" | awk '{print $1}')
test "$actual_sha256" = "$expected_sha256"
mkdir "$temp_dir/extracted"
ditto -x -k "$temp_dir/WorkBuddy.zip" "$temp_dir/extracted"
app=$(find "$temp_dir/extracted" -maxdepth 2 -type d -name WorkBuddy.app -print -quit)
test -n "$app"
codesign --verify --deep --strict "$app"
spctl --assess --type execute "$app""#
        .to_string();
    command.push('\n');
    command.push_str(&macos_install_app_script("WorkBuddy", "\"$app\""));
    sh_plan(command, "WorkBuddy official signed package")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_uses_vendor_metadata_checksum_and_full_app_bundle() {
        let plans = install_plans_for_platform(Platform::MacOS, AgentInstallAction::Install);
        let command = plans[0].command_line();
        assert!(command.contains("https://copilot.tencent.com/v2/update"));
        assert!(command.contains("https://download.codebuddy.cn/workbuddy/"));
        assert!(command.contains("sha256hash"));
        assert!(command.contains("WorkBuddy.app"));
        assert!(command.contains("codesign --verify"));
        assert!(command.contains("/Applications/WorkBuddy.app"));
        assert!(command.contains("sudo ditto"));
    }

    #[test]
    fn linux_is_blocked_because_workbuddy_is_not_published_there() {
        assert!(
            install_plans_for_platform(Platform::Linux, AgentInstallAction::Install).is_empty()
        );
    }
}
