use std::path::Path;

#[test]
fn cross_build_scripts_define_expected_target_matrix() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scripts = [
        std::fs::read_to_string(root.join("scripts").join("build-cross.ps1")).unwrap(),
        std::fs::read_to_string(root.join("scripts").join("build-cross.sh")).unwrap(),
    ];

    for target in [
        "x86_64-pc-windows-gnu",
        "aarch64-pc-windows-gnullvm",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
    ] {
        for script in &scripts {
            assert!(script.contains(target), "missing target {target}");
        }
    }
}

#[test]
fn cross_build_scripts_have_setup_and_selection_flags() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ps1 = std::fs::read_to_string(root.join("scripts").join("build-cross.ps1")).unwrap();
    let sh = std::fs::read_to_string(root.join("scripts").join("build-cross.sh")).unwrap();

    for required in ["cargo-zigbuild", "zig", "rustup target add", "dist"] {
        assert!(
            ps1.contains(required),
            "PowerShell script missing {required}"
        );
        assert!(sh.contains(required), "shell script missing {required}");
    }

    for flag in ["Target", "SkipSetup", "Help"] {
        assert!(ps1.contains(flag), "PowerShell script missing {flag}");
    }
    for flag in ["--target", "--skip-setup", "--help"] {
        assert!(sh.contains(flag), "shell script missing {flag}");
    }
}

#[test]
fn cross_build_scripts_only_package_cli_binary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scripts = [
        std::fs::read_to_string(root.join("scripts").join("build-cross.ps1")).unwrap(),
        std::fs::read_to_string(root.join("scripts").join("build-cross.sh")).unwrap(),
    ];

    for script in &scripts {
        assert!(
            !script.contains("--lib"),
            "cross build scripts should only build the CLI binary"
        );
    }
}
