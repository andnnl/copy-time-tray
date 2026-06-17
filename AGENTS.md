# AGENTS.md — Copy Time Tray

## Commands

```bash
cargo build --release          # Build (Linux target)
./target/release/copy-time-tray  # Run
./run.sh                       # Shorthand to run release binary
```

## Architecture

- **Single file**: `src/main.rs` — all logic, ~267 lines
- **Conditional compilation**: `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "windows")]` modules under `mod platform`
- **No macOS impl** despite Cargo.toml having macOS deps — only Linux and Windows are implemented
- `main()` delegates to `platform::main()`

### Platform split

| Linux | Windows |
|-------|---------|
| GTK + `libappindicator` | `tray-icon` + `muda` |
| `gtk::Clipboard` | `arboard::Clipboard` |
| `gtk::main()` event loop | `thread::sleep` loop |
| SVG icon from `icons/copy-time.svg` | Procedurally generated 32x32 RGBA icon |

## Linux Prerequisites

```bash
sudo apt install -y libgtk-3-dev libappindicator3-dev
```

Without these, `cargo build` fails with linker errors (not Rust errors).

## Cross-compile Windows from Linux

`.cargo/config.toml` sets `linker = "x86_64-w64-mingw32-gcc"` for `x86_64-pc-windows-gnu` target.

## Release Profile

Optimized for minimal binary size: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.

## Features

- Copy time (full/date/compact) to clipboard via tray menu
- Password generation (8/12/16 chars) — copies to clipboard
- MD5 hash generation (16/32 chars) from current timestamp — copies to clipboard
- Desktop notification via `notify-rust` after each copy action

## No tests, no CI, no linter config

No test framework, lint rules, or CI workflows exist. Don't assume any.
