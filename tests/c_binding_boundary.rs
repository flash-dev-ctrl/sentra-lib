use std::fs;
use std::path::Path;

#[test]
fn c_binding_surface_lives_under_bindings_c() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let normalized_lib_rs = lib_rs.replace("\r\n", "\n");
    let bindings_mod = fs::read_to_string(root.join("src/bindings/mod.rs")).unwrap();

    assert!(
        manifest.contains("c-binding ="),
        "sentra-lib must expose an explicit c-binding feature"
    );
    assert!(
        manifest.contains("crate-type = [\"cdylib\", \"staticlib\", \"rlib\"]"),
        "C binding artifacts must default to cdylib while retaining staticlib and rlib fallback outputs"
    );
    assert!(
        normalized_lib_rs.contains("#[cfg(feature = \"c-binding\")]\npub mod bindings;"),
        "bindings must be feature-gated at the crate root"
    );
    assert!(
        !lib_rs.contains("pub mod ffi"),
        "C FFI must not be exposed as sentra_lib::ffi"
    );
    assert!(
        bindings_mod.contains("pub mod c"),
        "C binding must live under sentra_lib::bindings::c"
    );
    assert!(
        !root.join("src/adapter/mod.rs").exists()
            && !root.join("src/adapter/types.rs").exists()
            && !root.join("src/adapter/collect.rs").exists(),
        "binding DTO/adapter source files must not live in the core src/adapter path"
    );
    assert!(
        root.join("src/bindings/c/ffi.rs").is_file(),
        "C FFI implementation must live under src/bindings/c"
    );
    assert!(
        root.join("src/bindings/c/types.rs").is_file(),
        "C binding DTOs must live under src/bindings/c"
    );
    assert!(
        root.join("src/bindings/c/adapter.rs").is_file(),
        "C binding adapter must live under src/bindings/c"
    );
}

#[test]
fn c_binding_default_distribution_uses_dynamic_library() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts").join("build-adapter-apple.sh")).unwrap();
    let podspec = fs::read_to_string(root.join("SentraLib.podspec")).unwrap();
    let example_makefile = fs::read_to_string(root.join("examples").join("Makefile")).unwrap();

    assert!(
        script.contains("libsentra_lib.dylib") && script.contains("libsentra.dylib"),
        "Apple adapter build must package the Rust cdylib as libsentra.dylib"
    );
    assert!(
        script.contains("install_name_tool -id @rpath/libsentra.dylib"),
        "libsentra.dylib must use an @rpath install name"
    );
    assert!(
        script.contains("--features c-binding"),
        "C ABI symbols must be compiled into the dylib"
    );
    assert!(
        podspec.contains(
            "spec.vendored_libraries = \"dist/apple-darwin-universal/lib/libsentra.dylib\""
        ),
        "podspec must vend the dynamic library"
    );
    assert!(
        podspec.contains("spec.prepare_command")
            && podspec.contains("./scripts/build-adapter-apple.sh"),
        "podspec must build the vendored dylib before CocoaPods collects file paths"
    );
    assert!(
        example_makefile.contains("-lsentra")
            && example_makefile.contains("-Wl,-rpath")
            && !example_makefile.contains("-llzma"),
        "C++ example should link the dylib directly without static-library transitive system flags"
    );
}
