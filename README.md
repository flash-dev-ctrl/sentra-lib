# sentra-lib

[![CI](https://github.com/flash-dev-ctrl/sentra-lib/actions/workflows/ci.yml/badge.svg)](https://github.com/flash-dev-ctrl/sentra-lib/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/flash-dev-ctrl/sentra-lib?include_prereleases)](https://github.com/flash-dev-ctrl/sentra-lib/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[English](README.en.md)

`sentra-lib` 是 Sentra 的 Rust 运行时库，用于发现本机 AI Agent，读取 Skill、Provider、MCP、Memory、Cron 等资产数据，并执行风险扫描。

CLI 已移到上层 `sentra-cli` crate；本 crate 只维护可复用库 API、规则加载/扫描能力和 C ABI。

## 功能

- 发现当前用户目录下的 Agent 实例。
- 读取 Agent 资产数据并序列化为结构化 JSON。
- 解析 Skill、Provider、MCP、Memory、Cron 等资产类型。
- 加载 YARA、Hash、本地威胁情报规则。
- 支持 LLM 和在线威胁情报检查器。
- 提供 Rust 公共 API。
- 在 `c-binding` feature 下提供 C ABI。

## 下载预编译库

最新版预编译库可从 GitHub Releases 下载：

```text
https://github.com/flash-dev-ctrl/sentra-lib/releases/latest
```

平台包名：

- Linux x86_64 静态库：`sentra-lib-linux-x86_64-musl.tar.gz`
- Linux aarch64 静态库：`sentra-lib-linux-aarch64-musl.tar.gz`
- Windows x86_64 static CRT：`sentra-lib-windows-x86_64-static.zip`
- macOS Intel：`sentra-lib-macos-x86_64.tar.gz`
- macOS Apple Silicon：`sentra-lib-macos-aarch64.tar.gz`
- 校验文件：`SHA256SUMS.txt`

## 包和导入名

Cargo package 名称是 `sentra-lib`：

```toml
[dependencies]
sentra-lib = { path = "../sentra-lib" }
```

Rust 代码中的 crate 名称是 `sentra_lib`：

```rust
use sentra_lib::{
    SentraResult,
    agents::discover_agents,
    interfaces::AssetType,
    users::list_users,
};
```

公开 API 位于 crate root 下：

```text
sentra_lib::agents
sentra_lib::interfaces
sentra_lib::protocol
sentra_lib::risks
sentra_lib::users
sentra_lib::{SentraError, SentraResult}
```

`utils` 是内部实现模块，不作为公共 API 使用。

## Rust 示例

发现当前机器上的用户与 Agent，并读取 Skill 资产：

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

常用导入：

```rust
use sentra_lib::agents::{Agent, discover_agents};
use sentra_lib::interfaces::{AssetType, ErasedAsset, ProviderData, SkillData};
use sentra_lib::protocol::WireProtocol;
use sentra_lib::risks::{RiskScanner, RuleDirectoryConfig, ScanOptions};
use sentra_lib::users::{UserHome, list_users};
use sentra_lib::{SentraError, SentraResult};
```

## 规则与扫描

`sentra-lib` 的风险扫描入口位于 `sentra_lib::risks`：

```rust
use sentra_lib::risks::{RiskScanner, ScanOptions, ensure_bundled_rules};

let rules = ensure_bundled_rules(home_dir)?;
let options = ScanOptions {
    rules: Some(rules),
    ..Default::default()
};

let mut scanner = RiskScanner::new(options);
scanner.load_rules()?;
```

如果需要自定义规则目录，也可以继续手动传入 `RuleDirectoryConfig`。

支持的规则类型：

- YARA：`.yar`、`.yara`，或内容中包含 `rule <name>`。
- 威胁情报：`.txt`、`.csv`，每行一个 IP 或域名；自动识别时至少需要 3 行有效数据，且其中至少一半能识别为 IP 或域名。
- Hash 黑名单：建议使用 `black*.txt` 命名。
- Hash 白名单：建议使用 `white*.txt` 命名。

导入本地规则可以使用 `RuleStore`：

```rust
use sentra_lib::risks::{RuleDirectoryConfig, RuleStore};

let store = RuleStore::new(RuleDirectoryConfig {
    yara: Some("rules/yara".into()),
    ti: Some("rules/ti".into()),
    hash: Some("rules/hash".into()),
});

store.import("./rules.zip")?;
store.import("./rules/yara")?;
```

## C ABI

启用 `c-binding` feature 后，`sentra-lib` 会导出 C ABI，用于 C、C++、Objective-C、Swift 等宿主集成。

验证 C binding：

```bash
cargo test --features c-binding
```

生成 Apple universal 动态库：

```bash
./scripts/build-adapter-apple.sh
```

输出目录：

```text
dist/apple-darwin-universal/
  include/sentra.h
  lib/libsentra.dylib
```

`libsentra.dylib` 的 install name 是 `@rpath/libsentra.dylib`。

## CocoaPods 集成

`SentraLib.podspec` 将 `dist/apple-darwin-universal/lib/libsentra.dylib` 作为 vendored dynamic library 暴露，并导出 `dist/apple-darwin-universal/include/sentra.h`。

Git 依赖会在 CocoaPods 下载后执行 `prepare_command`，自动构建动态库：

```ruby
target 'SentraWorker' do
  inherit! :search_paths
  use_frameworks!
  pod 'SentraLib', :git => 'https://github.com/flash-dev-ctrl/sentra-lib.git', :tag => '0.1.0'
end
```

本地 `:path` 开发依赖不会执行 CocoaPods `prepare_command`，需要先在 `sentra-lib` 目录运行：

```bash
./scripts/build-adapter-apple.sh
```

然后在调用方 Podfile 中使用：

```ruby
pod 'SentraLib', :path => '../../sentra-lib'
```

## 构建和测试

运行库测试：

```bash
cargo test
```

运行 C binding 测试：

```bash
cargo test --features c-binding
```

构建 Rust library：

```bash
cargo build --release
```

## 开源协议

本项目使用 MIT License。
