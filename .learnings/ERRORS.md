# Errors

Command failures and integration errors.

---
## [ERR-20260711-001] cargo_test_compile

**Logged**: 2026-07-11T00:00:00+08:00
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
Initial export-worker test refactor renamed the existing import result type, breaking Linux UI references.

### Error
`ImportResult` could not be found in `transfer` while compiling `linux_manager/mod.rs`.

### Context
The new export result was combined into a renamed enum before all existing import call sites were updated.

### Resolution
Restored the existing `ImportResult` API; the export test now targets a new pure export helper.

---

## [ERR-20260711-002] visual_companion_start_path

**Logged**: 2026-07-11T10:02:46+08:00
**Priority**: low
**Status**: resolved
**Area**: docs

### Summary
The visual-companion startup command failed because `$USER_SKILLS` was unset in the shell despite the logical mount being available.

### Error
```
/brainstorming/scripts/start-server.sh: 没有那个文件或目录
```

### Context
The first startup attempt interpolated `$USER_SKILLS`; rerunning with `@user-skills/brainstorming/scripts/start-server.sh` succeeded.

### Resolution
Use the documented logical mount path directly for skill scripts in this environment.

---
## [ERR-20260711-003] local_install_permission

**Logged**: 2026-07-11T13:34:00+08:00
**Priority**: low
**Status**: resolved
**Area**: infra

### Summary
Direct replacement of the existing executable under `/usr/local/bin` was denied because the current user lacks write permission.

### Error
```
cp: 无法创建普通文件 '/usr/local/bin/copy-time-tray.v0.3.0.pre-json-font': 权限不够
```

### Context
The installed executable is root-owned. Non-interactive `sudo` access is available for the local update.

### Resolution
Use `sudo install` and an atomic `sudo mv` for deployment.

---
## [ERR-20260717-001] cargo verification timeout

**Logged**: 2026-07-17T09:52:24+08:00
**Priority**: medium
**Status**: resolved
**Area**: tests

### Summary
Combined format, full test, and build command exceeded the 120 second tool timeout.

### Error
```
Execution timed out after 120000 ms
```

### Context
- Command: cargo fmt --check && cargo test --no-fail-fast && cargo build
- The full test suite was still being compiled/run.

### Suggested Fix
Run verification stages separately with longer timeouts so each result is observable.

### Metadata
- Reproducible: yes
- Related Files: Cargo.toml

### Resolution
- **Resolved**: 2026-07-17T09:52:24+08:00
- **Notes**: Subsequent verification is split into separate commands.

---

## [ERR-20260716-001] cargo-test-filter

**Logged**: 2026-07-16
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
向 `cargo test` 同时传入两个位置过滤参数会失败。

### Error
`unexpected argument 'second_test_name' found`

### Context
尝试一次运行两个指定测试名；Cargo 只接受一个 TESTNAME 过滤参数。

### Suggested Fix
分别执行测试，或使用能共同匹配两者的单一过滤字符串。

### Metadata
- Reproducible: yes
- Related Files: src/snippet/ui/linux_manager/mod.rs

### Resolution
改为分别运行两个测试命令。

---

## [ERR-20260716-001] cargo-test-filter

**Logged**: 2026-07-16T00:00:00Z
**Priority**: low
**Status**: resolved
**Area**: tests

### Summary
Cargo test accepts one positional name filter, not multiple test names.

### Error
`unexpected argument 'second_test_name' found`

### Context
Attempted to run two exact unit test names in one `cargo test` invocation.

### Suggested Fix
Use one shared substring filter or run the two tests in separate commands.

### Metadata
- Reproducible: yes
- Related Files: src/snippet/ui/linux_manager/mod.rs, src/snippet/ui/search.rs

### Resolution
Use separate `cargo test <filter>` commands.

---
