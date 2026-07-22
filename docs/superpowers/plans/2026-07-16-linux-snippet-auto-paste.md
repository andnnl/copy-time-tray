# Linux 代码片段自动粘贴实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 X11 中复制代码片段后隐藏管理器窗口、恢复此前活动窗口并发送 `Ctrl+V`。

**架构：** `linux_manager` 在显示 GTK 对话框前保存当前 X11 窗口 ID。复制入口统一调用一个辅助函数，该函数写剪贴板、隐藏对话框，并通过 `xdotool` 激活保存的窗口并模拟粘贴；任何 X11 自动化错误降级为仅复制。

**技术栈：** Rust 2021、GTK 3、X11、系统 `xdotool`。

---

## 文件

- 修改：`src/snippet/ui/linux_manager/mod.rs` — X11 目标窗口捕获、命令构造、复制/隐藏/粘贴流程及单元测试。
- 创建：`docs/superpowers/specs/2026-07-16-linux-snippet-auto-paste-design.md` — 已批准的设计。
- 创建：`docs/superpowers/plans/2026-07-16-linux-snippet-auto-paste.md` — 本计划。

### 任务 1：为 X11 自动粘贴辅助逻辑编写失败测试

- [x] **步骤 1：编写失败的测试**

在 `src/snippet/ui/linux_manager/mod.rs` 的测试模块加入测试，断言 `parse_window_id("81788938\n") == Some(81788938)`，并拒绝 `""`、`"0"`、`"not-a-window"`；断言 `paste_commands(81788938)` 依次产生：

```rust
vec![
    ("windowactivate", vec!["--sync", "81788938"]),
    ("key", vec!["--clearmodifiers", "ctrl+v"]),
]
```

- [x] **步骤 2：运行测试验证失败**

运行：`cargo test -- snippet::ui::linux_manager::tests::`

预期：FAIL，原因是 `parse_window_id` 和 `paste_commands` 尚未定义。

- [x] **步骤 3：实现最少的可测试 X11 辅助逻辑**

在同一模块添加 `parse_window_id`，仅接受修剪后可解析且大于零的 `u64`；添加 `paste_commands`，返回激活和按键命令的参数；添加 `capture_active_window` 运行 `xdotool getactivewindow`，添加 `paste_into_window` 顺序运行返回的命令并在失败时提前返回。

- [x] **步骤 4：运行测试验证通过**

运行：`cargo test -- snippet::ui::linux_manager::tests::`

预期：PASS。

### 任务 2：接入两个 Linux 复制入口

- [x] **步骤 1：为统一流程编写失败的测试**

添加一个纯函数测试，断言复制动作顺序为 `HideDialog` 后 `Paste(window_id)`；当窗口 ID 为 `None` 时只有 `HideDialog`。该纯计划函数避免在单元测试中调用 GTK 或 X11。

- [x] **步骤 2：运行测试验证失败**

运行：`cargo test -- snippet::ui::linux_manager::tests::copy`

预期：FAIL，原因是复制动作计划函数未定义。

- [x] **步骤 3：实现最少接入代码**

定义复制动作计划函数。`show` 调用 `capture_active_window` 并将结果复制进双击和复制按钮回调。回调先写 GTK 剪贴板和通知，然后调用 `dialog.hide()`、释放 `DIALOG_OPEN`，最后依据计划调用 `paste_into_window`。双击和按钮共享一个闭包或辅助函数，避免行为分歧。

- [x] **步骤 4：运行测试验证通过**

运行：`cargo test -- snippet::ui::linux_manager::tests::`

预期：PASS。

### 任务 3：格式、全量验证与本机部署

- [x] **步骤 1：格式化检查**

运行：`cargo fmt --check`

预期：PASS；如失败先运行 `cargo fmt`，再重新检查。

- [x] **步骤 2：运行完整测试**

运行：`cargo test`

预期：所有测试通过。

- [x] **步骤 3：构建发布程序**

运行：`cargo build --release`

预期：exit code 0，生成 `target/release/copy-time-tray`。

- [x] **步骤 4：部署已验证二进制**

运行：`install -m 755 target/release/copy-time-tray /usr/local/bin/copy-time-tray`，然后以 `sha256sum` 对比安装文件与发布文件。

预期：两项校验和相同。
