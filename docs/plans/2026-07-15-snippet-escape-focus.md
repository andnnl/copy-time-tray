# Snippet Escape Hide and Search Focus Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Hide the Code Snippets main window on Escape and focus its search field every time it is shown, without changing Escape behavior in settings or child editors.

**Architecture:** Linux handles Escape on the main GTK dialog, hides it, resets the single-window guard, and grabs focus on the `GtkSearchEntry` after display. Windows keeps the root eframe viewport alive, uses `ViewportCommand::Visible(false)` instead of `Close`, restores it with `Visible(true)` plus `Focus`, and requests widget focus for the search `TextEdit` on every show transition.

**Tech Stack:** Rust, GTK 3 / gtk-rs 0.18, eframe/egui 0.29, crossbeam-channel.

---

### Task 1: Define and test reusable snippet window transitions

**Files:**
- Modify: `src/snippet/ui/search.rs`
- Modify: `src/snippet/ui/linux_manager/mod.rs`

1. Write failing pure-state tests that specify: Escape hides the main search window, Show makes it visible and requests search focus, and child editor Escape remains local.
2. Run the targeted tests and verify RED for missing transition helpers/state.
3. Add the smallest state flags/helpers needed by the platform UI code.
4. Run the targeted tests and verify GREEN.

### Task 2: Implement Linux GTK Escape hide and search focus

**Files:**
- Modify: `src/snippet/ui/linux_manager/mod.rs`

1. Add a failing test for the main-dialog Escape decision if a pure helper is required.
2. Connect Escape handling only to the main snippets dialog; call `hide()` and reset `DIALOG_OPEN`.
3. After showing the dialog, call `grab_focus()` on the search entry.
4. Preserve child create/edit/delete dialogs and settings shortcut-capture behavior.
5. Run Linux-targeted tests and `cargo check`.

### Task 3: Implement Windows viewport hide/reopen and search focus

**Files:**
- Modify: `src/snippet/ui/search.rs`
- Modify if needed: `src/app.rs`

1. Add failing tests for Show/Hide transitions and one-shot focus requests.
2. Replace root `ViewportCommand::Close` on Escape with `Visible(false)`.
3. On `UiCommand::Show`, mark visible, request native `Visible(true)` and `Focus`, and request focus on the search `TextEdit` response.
4. Keep editor Escape handling local by processing the editor before main-window Escape.
5. Run targeted tests and Windows GNU cross-check.

### Task 4: Full verification and runtime check

**Files:**
- Verify all modified Rust files.

1. Stop only the temporary `target/release/copy-time-tray` process started for testing; do not touch `/usr/local/bin` or the systemd user service.
2. Run `cargo fmt --check`.
3. Run `cargo test` and require zero failures.
4. Run host `cargo check` and `cargo check --target x86_64-pc-windows-gnu`.
5. Run `cargo build --release` and `git diff --check`.
6. Start the new release temporarily, verify stable CPU/process state, manually exercise open → search focus → Escape hide → reopen, then leave it running only if requested.

**Constraints:** No subagents during implementation, no commits, no deployment, no installed-service restart, and preserve all existing uncommitted changes.
