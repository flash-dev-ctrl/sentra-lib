cargo_manifest = File.read(File.join(__dir__, "Cargo.toml"))
cargo_version = cargo_manifest[/^version\s*=\s*"([^"]+)"/, 1]

Pod::Spec.new do |spec|
  spec.name = "SentraLib"
  spec.version = cargo_version
  spec.summary = "Sentra agent asset discovery and risk scanning C API."
  spec.description = <<-DESC
    SentraLib packages sentra-lib's Rust C ABI as a macOS vendored dynamic
    library for Objective-C, Swift, C, and C++ consumers.
  DESC

  spec.homepage = "https://github.com/chaitin/sentra-lib"
  spec.license = { :type => "MIT", :file => "LICENSE" }
  spec.authors = { "Chaitin Endpoint Team" => "endpoint-team@chaitin.com" }
  spec.source = {
    :git => "https://github.com/chaitin/sentra-lib.git",
    :tag => "#{spec.version}"
  }

  spec.osx.deployment_target = "10.15"
  spec.cocoapods_version = ">= 1.11"

  spec.prepare_command = <<-CMD
    set -euo pipefail
    ./scripts/build-adapter-apple.sh
  CMD

  spec.source_files = "dist/apple-darwin-universal/include/**/*.h"
  spec.public_header_files = "dist/apple-darwin-universal/include/**/*.h"
  spec.header_mappings_dir = "dist/apple-darwin-universal/include"
  spec.vendored_libraries = "dist/apple-darwin-universal/lib/libsentra.dylib"
  spec.preserve_paths = [
    "dist/apple-darwin-universal/include/sentra.h",
    "dist/apple-darwin-universal/lib/libsentra.dylib"
  ]
end
