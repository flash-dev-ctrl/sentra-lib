[CmdletBinding()]
param(
    [string[]]$Target = @(),
    [switch]$SkipSetup,
    [switch]$Help
)

$ErrorActionPreference = "Stop"
$Targets = @(
    "x86_64-pc-windows-gnu",
    "aarch64-pc-windows-gnullvm",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl"
)

function Show-Help {
    @"
Usage:
  powershell -ExecutionPolicy Bypass -File scripts/build-cross.ps1 [options]

Options:
  -Target <target[]>  Build only selected Rust target triples. Repeat or pass comma-separated values.
  -SkipSetup          Do not install Zig, cargo-zigbuild, or Rust targets.
  -Help               Show this help.

Targets:
  x86_64-pc-windows-gnu
  aarch64-pc-windows-gnullvm
  x86_64-apple-darwin
  aarch64-apple-darwin
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-musl

Output:
  dist/<target>/sentra(.exe)
"@
}

function Resolve-RepoRoot {
    $scriptDir = Split-Path -Parent $PSCommandPath
    Resolve-Path (Join-Path $scriptDir "..")
}

function Command-Exists($name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

function Add-Zig-To-Path-If-WingetInstalled {
    $wingetRoot = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
    if (!(Test-Path $wingetRoot)) {
        return
    }
    $zig = Get-ChildItem -Path $wingetRoot -Recurse -Filter zig.exe -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($zig) {
        $zigDir = Split-Path -Parent $zig.FullName
        if (($env:PATH -split [IO.Path]::PathSeparator) -notcontains $zigDir) {
            $env:PATH = "$zigDir$([IO.Path]::PathSeparator)$env:PATH"
        }
    }
}

function Ensure-Zig {
    Add-Zig-To-Path-If-WingetInstalled
    if (Command-Exists "zig") {
        return
    }
    if (Command-Exists "winget") {
        winget install --id zig.zig -e --accept-package-agreements --accept-source-agreements
        Add-Zig-To-Path-If-WingetInstalled
    }
    if (!(Command-Exists "zig")) {
        throw "zig was not found. Install Zig manually or make zig.exe available on PATH."
    }
}

function Ensure-Cargo-Zigbuild {
    if (!(Command-Exists "cargo-zigbuild")) {
        cargo install cargo-zigbuild
    }
}

function Ensure-Rust-Targets($selectedTargets) {
    foreach ($target in $selectedTargets) {
        rustup target add $target
    }
}

function Normalize-Targets($rawTargets) {
    if ($rawTargets.Count -eq 0) {
        return $Targets
    }
    $selected = @()
    foreach ($item in $rawTargets) {
        foreach ($part in ($item -split ",")) {
            $trimmed = $part.Trim()
            if ($trimmed.Length -gt 0) {
                $selected += $trimmed
            }
        }
    }
    foreach ($target in $selected) {
        if ($Targets -notcontains $target) {
            throw "unknown target '$target'. Use -Help to list supported targets."
        }
    }
    $selected
}

function Artifact-Name($target) {
    if ($target -like "*windows*") {
        "sentra.exe"
    } else {
        "sentra"
    }
}

if ($Help) {
    Show-Help
    exit 0
}

$root = Resolve-RepoRoot
Set-Location $root
$env:CARGO_TARGET_DIR = Join-Path $root "target"
$selectedTargets = Normalize-Targets $Target

if (!$SkipSetup) {
    Ensure-Zig
    Ensure-Cargo-Zigbuild
    Ensure-Rust-Targets $selectedTargets
}

New-Item -ItemType Directory -Force -Path "dist" | Out-Null
foreach ($target in $selectedTargets) {
    Write-Host "==> Building $target"
    cargo zigbuild --release --target $target --bin sentra

    $name = Artifact-Name $target
    $source = Join-Path $env:CARGO_TARGET_DIR "$target\release\$name"
    $destDir = Join-Path $root "dist\$target"
    New-Item -ItemType Directory -Force -Path $destDir | Out-Null
    Copy-Item -Force $source (Join-Path $destDir $name)
}

Write-Host "Build outputs written to $root\dist"
