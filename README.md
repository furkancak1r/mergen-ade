# Mergen ADE (Prototype)

Mergen ADE is a **Windows desktop ADE** (Application Development Environment) focused on terminal orchestration and project contexts.

It is **not an IDE** and does not include an editor, LSP, or debugging UI.

## Goals in this prototype

- Native Rust desktop app (no Electron)
- Terminal emulation via `alacritty_terminal`
- ConPTY-backed terminal process integration on Windows
- Two left sidebars + tiled main terminal area
- Very small persisted state in local TOML config

## Assumptions used for this prototype

- This repository started empty (greenfield implementation).
- Target platform is Windows 10/11.
- Rust toolchain is expected to be installed by the user.
- Default shell is PowerShell (`powershell -NoLogo`).
- Terminal title max length is 40 characters.
- Terminal scrollback is intentionally limited (`1000` lines) and not persisted to disk.

## Build (Windows)

1. Install Rust stable (`x86_64-pc-windows-msvc`) from https://rustup.rs
2. Build:

```powershell
cargo build --release
```

3. Run:

```powershell
cargo run --release
```

## How ConPTY is used

- Each terminal session is created through `alacritty_terminal::tty::new(...)`.
- On Windows, `alacritty_terminal` uses its internal ConPTY backend (`tty::windows::conpty`) to create pseudoterminal pipes and child process wiring.
- Terminal emulation/parsing and PTY I/O run through `alacritty_terminal::event_loop::EventLoop` on a background thread.
- The UI thread receives wake/title/exit notifications through a channel and renders terminal snapshots incrementally.

## UI Overview

- **Sidebar A (leftmost):** Project list + project explorer tree (toggleable)
- **Sidebar B:** Terminal Manager grouped by project, with separate Foreground/Background sections
- **Main area:** Embedded, tiled terminal panes

## Key bindings

- `Ctrl+B`: Toggle Project Explorer
- `Ctrl+Shift+F`: Toggle Project Filter Mode
- `Ctrl+Shift+T`: New terminal for selected project
- `Ctrl+Shift+G`: Auto Tile (all visible)
- `Ctrl+Alt+G`: Auto Tile (selected project only)
- `Ctrl+Tab`: Next active terminal
- `Ctrl+Shift+Tab`: Previous active terminal
- `Ctrl+Shift+P`: Open saved messages picker

## Configuration and storage

Config file is stored in Windows app data using `directories`:

- `%APPDATA%\Mergen\MergenADE\config\config.toml` (platform-dependent exact path resolved by `ProjectDirs`)

Persisted data includes:

- Global default shell
- Projects (id, name, path)
- Optional per-project shell override
- Per-project saved messages
- UI state:
  - Project explorer visibility
  - Last selected project
  - Project filter mode
  - Auto tile scope

Not persisted:

- Terminal scrollback
- Live terminal sessions

## Minimal test strategy

The prototype includes unit tests for:

- Tiling grid calculation (`src/layout.rs`)
- Terminal title update logic (`src/title.rs`)

Run tests:

```powershell
cargo test
```

## Performance notes and profiling guidance

Low-memory / responsiveness choices in this prototype:

- Bounded terminal scrollback (`1000` lines)
- No terminal scrollback persistence
- PTY I/O and parsing isolated from UI thread
- UI updates driven by terminal wake events
- Main area redraw avoids unnecessary recompute except dirty sessions

Simple profiling checks:

1. Startup timing:

```powershell
Measure-Command { cargo run --release }
```

2. Memory snapshot while running:

```powershell
Get-Process mergen-ade | Select-Object Name, Id, WorkingSet64, PM, CPU
```

3. Multi-terminal behavior:
- Open many terminals across projects
- Verify input responsiveness and no obvious UI jank while output streams

## Non-goals (explicit)

- Built-in code editor
- LSP/debugging IDE workflows
- Telemetry/sign-in/network account features
