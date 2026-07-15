# JSON Formatter Maximize Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep the JSON formatter's current initial size while allowing maximization and making both editors fill the remaining window space.

**Architecture:** Preserve the existing responsive GTK `Paned` layout on Linux and explicitly test its resizable-window policy. On Windows, extract a pure remaining-height calculation and use it to size both egui multiline editors so controls and status remain visible while the editor area grows with the viewport.

**Tech Stack:** Rust, GTK 0.18, eframe/egui 0.29, Cargo tests and cross-compilation.

---

### Task 1: Encode the window and adaptive-height policies

**Files:**
- Modify: `src/platform/linux.rs`
- Modify: `src/platform/windows.rs`

1. Add failing unit tests proving the Linux JSON formatter is resizable and the Windows editor height grows with available height while respecting a minimum.
2. Run the focused tests and confirm they fail because the policy helpers do not exist.
3. Add minimal pure helper functions for both policies.
4. Re-run the focused tests.

### Task 2: Apply the policies to production windows

**Files:**
- Modify: `src/platform/linux.rs`
- Modify: `src/platform/windows.rs`

1. Use the Linux policy at dialog construction.
2. Use the Windows height helper with `ui.available_height()` and size both editors from the computed height.
3. Keep existing initial dimensions and do not open maximized by default.

### Task 3: Verify and deploy

1. Run `cargo fmt --check`, `cargo test`, host `cargo check`, Windows GNU `cargo check`, `cargo build --release`, and `git diff --check`.
2. Atomically replace `/usr/local/bin/copy-time-tray`, preserve a timestamped backup, restart `copy-time-tray.service`, and verify hash, service state, logs, and CPU.

No commits are created unless explicitly requested.
