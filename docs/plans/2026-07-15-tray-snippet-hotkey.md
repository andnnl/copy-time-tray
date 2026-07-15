# Tray Snippet Hotkey Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Do not use subagents for this project because the user explicitly prohibited them.

**Goal:** Put “代码片段” first in both tray menus and add a cross-platform, directly captured, transactionally applied global shortcut setting that reports binding success or failure.

**Architecture:** Add a platform-neutral hotkey model plus a runtime `HotkeyService` around `global-hotkey`. Persist only the last successfully bound shortcut in `GeneralConfig`; Linux GTK and Windows egui capture candidate keys but delegate validation and binding to the shared service. Keep the existing tray backends and code-snippet windows.

**Tech Stack:** Rust 2021, `global-hotkey` 0.6, crossbeam-channel, serde/YAML, GTK 3 on Linux, eframe/egui 0.29 on Windows.

---

## Constraints

- Preserve all pre-existing uncommitted user changes, especially Linux JSON text-view monospace settings and the CPU polling fix.
- Do not use subagents.
- Do not commit unless the user explicitly requests it.
- Follow strict red-green-refactor: each production behavior begins with a failing test.
- Never use `unwrap` in new runtime hotkey error paths.
- Linux builds are the primary local verification target; also compile-check the Windows target when the installed toolchain permits it.

### Task 1: Persist a backward-compatible shortcut setting

**Files:**
- Modify: `src/snippet/config.rs:59-87`
- Test: `src/snippet/config.rs:302-436`

**Step 1: Write failing config tests**

Add tests asserting:

```rust
#[test]
fn legacy_general_config_uses_default_snippet_hotkey() {
    let yaml = r#"
sync: null
general:
  auto_sync: true
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.general.snippet_hotkey, "Ctrl+Shift+S");
}

#[test]
fn snippet_hotkey_round_trips_through_yaml() {
    let mut config = Config::default();
    config.general.snippet_hotkey = "Alt+F8".to_string();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let restored: Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(restored.general.snippet_hotkey, "Alt+F8");
}
```

**Step 2: Verify RED**

Run: `cargo test legacy_general_config_uses_default_snippet_hotkey snippet_hotkey_round_trips_through_yaml`

Expected: compilation fails because `GeneralConfig::snippet_hotkey` does not exist.

**Step 3: Implement the minimal config field**

Add:

```rust
fn default_snippet_hotkey() -> String {
    "Ctrl+Shift+S".to_string()
}
```

and to `GeneralConfig`:

```rust
#[serde(default = "default_snippet_hotkey")]
pub snippet_hotkey: String,
```

Initialize it from `default_snippet_hotkey()` in `Default`.

**Step 4: Verify GREEN**

Run the two targeted tests separately if Cargo accepts only one filter, then run `cargo test snippet_hotkey`.

Expected: all matching tests pass.

### Task 2: Add the shared shortcut model and validation

**Files:**
- Create: `src/hotkey.rs`
- Modify: `src/main.rs:1-5`

**Step 1: Write failing model tests**

In `src/hotkey.rs`, define tests first for the wished-for API:

```rust
#[test]
fn function_keys_are_valid_without_modifiers() {
    for key in [Code::F1, Code::F6, Code::F12] {
        assert!(Shortcut::new(None, key).is_ok());
    }
}

#[test]
fn ordinary_keys_require_a_primary_modifier() {
    assert!(Shortcut::new(None, Code::KeyA).is_err());
    assert!(Shortcut::new(Some(Modifiers::SHIFT), Code::KeyA).is_err());
    assert!(Shortcut::new(Some(Modifiers::CONTROL), Code::KeyA).is_ok());
    assert!(Shortcut::new(Some(Modifiers::ALT), Code::Digit1).is_ok());
    assert!(Shortcut::new(Some(Modifiers::SUPER), Code::KeyS).is_ok());
}

#[test]
fn shortcut_display_round_trips() {
    let shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyS,
    )
    .unwrap();
    let text = shortcut.to_string();
    assert_eq!(text.parse::<Shortcut>().unwrap(), shortcut);
}
```

Add parsing tests for the stored default and rejection tests for invalid/no-primary-modifier strings.

**Step 2: Verify RED**

Run: `cargo test hotkey::tests`

Expected: compilation fails because `Shortcut` is missing.

**Step 3: Implement the model**

Implement a `Shortcut` wrapper around `global_hotkey::hotkey::HotKey` data with `Clone`, `Copy`, `Debug`, `Eq`, and `PartialEq`. Expose:

```rust
pub fn new(modifiers: Option<Modifiers>, code: Code) -> Result<Self, HotkeyError>;
pub fn modifiers(self) -> Option<Modifiers>;
pub fn code(self) -> Code;
pub fn to_hotkey(self) -> HotKey;
```

Implement `FromStr` by parsing with `HotKey`, then validating through `Shortcut::new`. Implement `Display` using a stable application-owned canonical order (`Ctrl`, `Alt`, `Shift`, `Super`, then code) so Linux and Windows show/save identical text. Normalize aliases accepted from existing config if necessary, but always emit one canonical form.

Define `HotkeyError` with user-facing variants for invalid syntax, missing primary modifier, unsupported key, manager initialization, registration, unregistration, and rollback failure. Its `Display` text must be suitable for the settings status label.

The primary-modifier rule is:

```rust
let is_function_key = matches!(code, Code::F1 | ... | Code::F12);
let has_primary = modifiers
    .map(|mods| mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER))
    .unwrap_or(false);
if !is_function_key && !has_primary { ... }
```

**Step 4: Verify GREEN**

Run: `cargo test hotkey::tests`

Expected: all shortcut model tests pass.

### Task 3: Implement transaction-safe runtime registration

**Files:**
- Modify: `src/hotkey.rs`
- Modify: `src/app.rs:1-49,83-98`

**Step 1: Write failing state-transition tests**

Avoid requiring a real desktop registration in unit tests. Introduce a private `HotkeyBackend` trait with `register` and `unregister`, implemented by `GlobalHotKeyManager`, and a fake backend in tests.

Cover:

```rust
#[test]
fn successful_rebind_replaces_current_shortcut() { ... }

#[test]
fn failed_new_registration_keeps_original_shortcut() { ... }

#[test]
fn failed_old_unregistration_rolls_back_new_registration() { ... }

#[test]
fn only_matching_pressed_event_opens_snippets() { ... }
```

The event predicate should accept `(current_id, event_id, state)` and return true only for matching `Pressed` events.

**Step 2: Verify RED**

Run: `cargo test hotkey::tests`

Expected: tests fail because runtime service/state transition APIs are missing.

**Step 3: Implement `HotkeyService`**

Add a mutex-protected service that owns the backend and current shortcut/hotkey. Its public API should include:

```rust
pub fn new(initial: Shortcut) -> Result<Self, HotkeyError>;
pub fn current(&self) -> Shortcut;
pub fn rebind(&mut self, candidate: Shortcut) -> Result<(), HotkeyError>;
pub fn current_id(&self) -> u32;
```

Use register-new-first semantics. On register failure, leave state untouched. If old unregister fails, unregister the newly registered key and keep the old state. Do not persist configuration inside this service; persistence remains an application transaction.

Create an application-level `OnceLock<Mutex<HotkeyService>>`. Replace `launch_global_hotkey` with initialization that:

1. loads `Config::load().unwrap_or_default().general.snippet_hotkey`;
2. parses it, falling back to the default shortcut with a notification if invalid;
3. creates/registers the service without panicking;
4. starts one event receiver thread;
5. checks event ID and `Pressed` before sending `UiCommand::Show`.

Expose an application function such as:

```rust
pub fn apply_config_with_hotkey(mut config: Config) -> Result<(), String>
```

It parses the candidate, locks the service, rebinds it, saves the config only after registration succeeds, and rolls the runtime binding back if config save fails. Reload `SnippetManager` only after the whole transaction succeeds.

**Step 4: Verify GREEN**

Run: `cargo test hotkey::tests && cargo test snippet_hotkey`

Expected: all matching tests pass.

### Task 4: Add a reusable captured-key representation

**Files:**
- Modify: `src/hotkey.rs`

**Step 1: Write failing capture conversion tests**

Define a UI-neutral `CapturedShortcut` or conversion helper and test modifier normalization, `Esc` cancellation semantics at the UI boundary, supported alphanumeric keys, digits, navigation keys if intentionally supported, and `F1-F12`.

Test that unsupported modifier-only events do not replace the candidate.

**Step 2: Verify RED**

Run: `cargo test hotkey::tests::capture`

Expected: fail because capture conversion is absent.

**Step 3: Implement minimal conversion helpers**

Keep GTK keyval and egui `Key` mappings inside their platform modules, but feed both into the shared `Shortcut::new`. Only map keys supported by `global-hotkey::hotkey::Code`; return a clear error for unsupported keys.

**Step 4: Verify GREEN**

Run: `cargo test hotkey::tests`

Expected: all tests pass.

### Task 5: Add Linux GTK capture and transaction-aware save

**Files:**
- Modify: `src/snippet/ui/tabs/general_tab.rs`
- Modify: `src/snippet/ui/config_window.rs`
- Modify: `src/platform/linux.rs:208-368`

**Step 1: Write failing pure mapping tests**

Extract GTK keyval/modifier mapping into testable functions under `#[cfg(target_os = "linux")]`. Add tests for `Ctrl+Shift+S`, standalone F8, invalid `Shift+A`, and Escape cancellation.

**Step 2: Verify RED**

Run: `cargo test general_tab`

Expected: fail because key capture mapping/state is missing.

**Step 3: Build the GTK shortcut controls**

Extend `GeneralTab` with:

- a non-editable `gtk::Entry` or focusable capture button;
- a status `gtk::Label`;
- candidate shortcut state using `Rc<RefCell<Shortcut>>` or another GTK-main-thread-safe owner;
- key press handling that maps GDK modifiers/keyvals to the shared model;
- Escape cancellation and readable placeholder instructions.

Load `general.snippet_hotkey` into the control. Make `on_save` validate both the sync interval and candidate shortcut. Add a method returning the canonical candidate string and a method setting success/failure status styling.

**Step 4: Keep the dialog open on failure**

Refactor `ConfigWindow::show` into a response loop:

```rust
loop {
    match dialog.window.run() {
        gtk::ResponseType::Ok => match validate_and_build_config(&dialog) {
            Ok(config) => return Some(config),
            Err(message) => {
                dialog.general_tab.show_error(&message);
                continue;
            }
        },
        _ => return None,
    }
}
```

The platform callback must call `apply_config_with_hotkey`. On failure, return the error to the still-open dialog rather than closing it. If necessary, move transaction execution into a callback passed to `ConfigWindow::show`, so the dialog controls whether it closes.

**Step 5: Verify GREEN**

Run: `cargo test general_tab && cargo test config_window && cargo check`

Expected: tests pass and Linux code compiles.

### Task 6: Add the Windows egui settings window

**Files:**
- Modify: `src/platform/windows.rs`
- Modify: `src/app.rs` only if a public config-apply function or window command is needed

**Step 1: Write failing Windows-independent state tests**

Extract a `HotkeySettingsState` that can be tested on Linux without creating an eframe window. Cover candidate capture, Escape cancel, success status, failure status, and preserving the current displayed binding after a failed apply.

**Step 2: Verify RED**

Run: `cargo test hotkey_settings`

Expected: fail because settings state is missing.

**Step 3: Implement the egui window**

Add a single-instance window guard similar to `JSON_WINDOW_OPEN`. The app should:

- load the current `Config`;
- display a focusable “按下新快捷键” capture control;
- consume egui key events and map egui modifiers/keys to `Shortcut`;
- show canonical candidate text;
- call `apply_config_with_hotkey` on save;
- show green success or red error text;
- remain open on failure;
- close only on cancel or successful save.

Replace the Windows “手动修改配置文件” notification for `open_config` with the settings window launcher.

**Step 4: Verify GREEN**

Run: `cargo test hotkey_settings`

Then, if the Windows GNU target is installed, run:

`cargo check --target x86_64-pc-windows-gnu`

Expected: state tests pass and Windows target check exits 0. If system cross-link dependencies are missing, record the exact environmental failure and still run host tests/checks.

### Task 7: Put “代码片段” first in both menus

**Files:**
- Modify: `src/platform/linux.rs:241-365`
- Modify: `src/platform/windows.rs:205-267`

**Step 1: Write failing menu-order tests**

Extract platform-neutral menu IDs/labels into small functions returning ordered slices/vectors. Test:

```rust
assert_eq!(linux_menu_order().first(), Some(&"代码片段"));
assert_eq!(windows_menu_order().first(), Some(&"search_snippets"));
```

Use these definitions when constructing the actual menus so the tests verify production data, not duplicate expectations.

**Step 2: Verify RED**

Run: `cargo test menu_order`

Expected: fail because the current first items are time-copy actions.

**Step 3: Reorder menu construction**

Create and append the code-snippet item before all other menu items on Linux and Windows. Preserve the existing refresh-before-show behavior on Windows and the existing command channel behavior on Linux.

**Step 4: Verify GREEN**

Run: `cargo test menu_order`

Expected: both order tests pass.

### Task 8: Full verification and runtime checks

**Files:**
- Review all modified files

**Step 1: Run diagnostics**

Run `lsp_diagnostics` for every modified Rust file. If rust-analyzer again fails from the known environment recursion issue, record it and rely on compiler evidence rather than claiming LSP success.

**Step 2: Run the complete automated suite**

Run sequentially:

```bash
cargo fmt --check
cargo test
cargo check
cargo build --release
git diff --check
```

Expected: every command exits 0; report the exact test count and any pre-existing warnings.

**Step 3: Inspect the final diff**

Run `git status --short` and `git diff -- src/main.rs src/app.rs src/hotkey.rs src/snippet/config.rs src/snippet/ui/tabs/general_tab.rs src/snippet/ui/config_window.rs src/platform/linux.rs src/platform/windows.rs docs/superpowers/specs/2026-07-15-tray-snippet-hotkey-design.md docs/plans/2026-07-15-tray-snippet-hotkey.md`.

Confirm no unrelated user edits were removed and no generated binaries/artifacts were introduced.

**Step 4: Runtime validation on Linux**

Using the locally built release binary in an available desktop session:

1. Open the tray menu and verify “代码片段” is first.
2. Open settings and verify the current shortcut is displayed.
3. Capture and save a free shortcut; verify “绑定成功”.
4. Press it and verify the code-snippet window opens.
5. Attempt an occupied/invalid shortcut; verify the window stays open and the previous shortcut still opens snippets.
6. Restart the app; verify the successful shortcut persisted.
7. Check process CPU to ensure the previous GTK polling fix remains effective.

Do not install or restart the system service unless deployment is explicitly requested.
