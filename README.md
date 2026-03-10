<p align="center">
  <img src="logo.png" alt="Mergen ADE logo" width="220">
</p>

<h1 align="center">Mergen ADE</h1>

<p align="center">
  Windows-native terminal workspace for running and organizing multiple project contexts.
</p>

<p align="center">
  <a href="https://github.com/furkancak1r/mergen-ade/releases/latest"><strong>Download for Windows</strong></a>
  |
  <a href="#build-from-source"><strong>Build from Source</strong></a>
</p>

<p align="center">
  No release published yet? Use the one-command local build below.
</p>

Mergen ADE is a desktop ADE focused on terminal orchestration, project context switching, and lightweight workspace management on Windows.

It is not an IDE. There is no built-in editor, LSP, or debugger UI in this project.

## Why Mergen ADE

- Keep multiple terminals visible without turning your desktop into window clutter.
- Group sessions by project so context switches stay fast and predictable.
- Run a native Rust desktop app instead of a browser or Electron shell.
- Persist only the small amount of state that helps you get back to work quickly.

## Quick Start

### Download the Windows build

The canonical download location is the GitHub Releases page:

- https://github.com/furkancak1r/mergen-ade/releases/latest

As of March 10, 2026, this repository does not have a published GitHub Release yet. When the first release is published, the Windows portable ZIP will appear there.

### Local build

Preferred one-command build:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1
```

This produces the portable Windows executable at:

```text
target\x86_64-pc-windows-msvc\release\mergen-ade.exe
```

For normal local development:

```powershell
cargo build --release
cargo test
```

If `cargo` is not on PATH in PowerShell:

```powershell
$env:USERPROFILE\.cargo\bin\cargo.exe build --release
$env:USERPROFILE\.cargo\bin\cargo.exe test
```

## Core Features

- Native Windows desktop app built in Rust
- Embedded terminal panes with tiled layout management
- Project-aware terminal grouping in the side panel
- ConPTY-backed shell sessions with responsive IO flow
- Lightweight local TOML configuration
- Portable Windows release pipeline through GitHub Actions

## How It Works

- Terminal sessions are created through `portable-pty` using the native Windows PTY system.
- Terminal emulation and parsing are handled by `tattoy-wezterm-term`.
- PTY reads, writes, and resize handling run off the UI thread to keep the app responsive.
- The main window combines an activity rail, collapsible side panels, and tiled terminal panes.

## UI Overview

- **Activity rail:** icon-first left rail for switching between `Project Explorer` and `Terminal Manager`
- **Project Explorer:** project picker, quick actions, search, indexed folder tree, and source control view
- **Terminal Manager:** project-grouped foreground and background terminal lists
- **Main area:** embedded tiled terminal panes for concurrent terminal work
- **Terminal visibility mode:** configurable between global visibility and selected-project-only visibility

## Build From Source

The release script is the supported path for a portable Windows binary.

What it does:

1. Builds a portable release for `x86_64-pc-windows-msvc`
2. Ensures the `stable-x86_64-pc-windows-msvc` toolchain is available
3. Resolves the local Visual Studio build environment when needed
4. Statically links the MSVC CRT for a portable EXE workflow
5. Verifies imports with repo-local `llvm-objdump.exe` when available, otherwise `dumpbin.exe`
6. Produces `target\x86_64-pc-windows-msvc\release\mergen-ade.exe`

Regression check:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\build-release.tests.ps1
```

## GitHub Releases

This repository includes a Windows release workflow at `.github/workflows/release.yml`.

When a tag starting with `v` is pushed, GitHub Actions will:

1. Build the portable `mergen-ade.exe` for `x86_64-pc-windows-msvc`
2. Package it as `mergen-ade-<tag>-windows-x64-portable.zip`
3. Publish a GitHub Release and attach the ZIP asset

Maintainer tag example:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

## Configuration

Config is stored in Windows app data via `ProjectDirs`:

- `%APPDATA%\Mergen\MergenADE\config\config.toml`

Persisted data includes:

- global default shell
- projects with id, name, and path
- per-project saved messages
- UI state such as visible panels, selected project, filter mode, and auto tile scope

Not persisted:

- terminal scrollback
- live terminal sessions

## Testing

The project currently includes unit tests for:

- tiling grid calculation in `src/layout.rs`
- terminal title update logic in `src/title.rs`
- Windows release helper regressions in `scripts/__tests__/build-release.tests.ps1`

Run checks:

```powershell
cargo test
powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\build-release.tests.ps1
```

## Non-goals

- Built-in code editor
- LSP or debugger workflows
- Telemetry, sign-in, or online account features

## Build Troubleshooting

- `link.exe not found`
  - Install Visual Studio Build Tools or Visual Studio 2022 with `Desktop development with C++`, then rerun the release script.
- `Required x64 MSVC/SDK libraries were not found in LIB`
  - Install the Windows SDK and MSVC CRT libraries that ship with the Desktop development with C++ workload, then rerun the release script.
- `MSVC Rust toolchain not found`
  - The release script provisions `stable-x86_64-pc-windows-msvc` automatically when `rustup` is available.
- `toolchain 'stable-x86_64-pc-windows-msvc' is not installed`
  - The release script installs it automatically through `rustup toolchain install ... --profile minimal`.
- `dependency tool not found`
  - The release script first checks repo-local `llvm-objdump.exe`, then resolves `dumpbin.exe` from Visual Studio or Build Tools even outside Developer PowerShell.
- `x86_64-w64-mingw32-clang.exe not found`
  - Plain local `cargo` builds still depend on the repo-local LLVM-MinGW linker configured in `.cargo\config.toml`; make sure `.toolchain\llvm-mingw-20260224-ucrt-x86_64\bin` exists.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).
