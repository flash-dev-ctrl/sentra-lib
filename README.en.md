# sentra-lib

[![CI](https://github.com/flash-dev-ctrl/sentra-lib/actions/workflows/ci.yml/badge.svg)](https://github.com/flash-dev-ctrl/sentra-lib/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/flash-dev-ctrl/sentra-lib?include_prereleases)](https://github.com/flash-dev-ctrl/sentra-lib/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`sentra-lib` is the reusable Rust runtime library behind Sentra. It discovers
local AI Agent installations, reads assets such as skills, providers, MCP
servers, memory, and cron entries, and runs risk checks over those assets.

The command-line interface lives in the parent `sentra-cli` crate. This crate
keeps the reusable Rust API, rule loading, risk scanners, and optional C ABI.

## Features

- Discover supported local AI Agent installations.
- Read and serialize Agent assets as structured data.
- Release bundled YARA, hash, and local threat-intelligence rules.
- Load custom YARA, hash, and local threat-intelligence rules.
- Run optional LLM and online threat-intelligence checks.
- Expose a Rust public API.
- Expose a C ABI behind the `c-binding` feature.

## Prebuilt Libraries

Download the latest prebuilt library archives from GitHub Releases:

```text
https://github.com/flash-dev-ctrl/sentra-lib/releases/latest
```

Release assets:

- Linux x86_64 static library: `sentra-lib-linux-x86_64-musl.tar.gz`
- Linux aarch64 static library: `sentra-lib-linux-aarch64-musl.tar.gz`
- Windows x86_64 static CRT: `sentra-lib-windows-x86_64-static.zip`
- macOS Intel: `sentra-lib-macos-x86_64.tar.gz`
- macOS Apple Silicon: `sentra-lib-macos-aarch64.tar.gz`
- Checksums: `SHA256SUMS.txt`

## Rust Usage

Cargo package name:

```toml
[dependencies]
sentra-lib = { path = "../sentra-lib" }
```

Rust crate name:

```rust
use sentra_lib::{
    SentraResult,
    agents::discover_agents,
    interfaces::AssetType,
    users::list_users,
};
```

Minimal discovery example:

```rust
use sentra_lib::{
    SentraResult,
    agents::discover_agents,
    interfaces::AssetType,
    users::list_users,
};

fn main() -> SentraResult<()> {
    for user in list_users() {
        for agent in discover_agents(&user.home) {
            println!("{}: {}", agent.name(), agent.home().display());

            for asset in agent.get_assets(AssetType::Skill)? {
                println!("{}", asset.data()?);
            }
        }
    }

    Ok(())
}
```

## Build And Test

```bash
cargo test --locked --all-targets
cargo test --locked --features c-binding
cargo build --locked --release
```

## C ABI

Enable the `c-binding` feature to export C ABI functions for C, C++,
Objective-C, and Swift hosts.

Build the Apple universal dynamic library:

```bash
./scripts/build-adapter-apple.sh
```

Output:

```text
dist/apple-darwin-universal/
  include/sentra.h
  lib/libsentra.dylib
```

## License

`sentra-lib` is released under the MIT License. See [LICENSE](LICENSE).
