# 异步 Termora 导入与入口拆分实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 导入 Termora JSON 时不阻塞 GTK 主循环，并把 `main.rs` 收敛为仅包含模块声明和应用入口。

**架构：** 在 `SnippetStore` 添加纯领域批量导入接口，在内存中用集合完成 ID 和现有名称唯一性去重，整批最多保存一次。GTK 导入回调只负责文件选择、禁用按钮和派发后台线程；工作线程执行读取、解析和批量存储，主线程经 `async_channel` 消费完成消息后刷新 UI 与显示通知。将应用装配、通用工具、平台托盘和 Linux GTK 管理器迁出入口。

**技术栈：** Rust 2021、gtk 0.18、glib、async-channel、crossbeam-channel、serde。

---

## 文件职责

- 修改：`src/snippet/store.rs` — 批量导入汇总、去重和单次持久化。
- 修改：`src/snippet/import_export.rs` — 仅保留 Termora 文件解析/转换；沿用现有测试。
- 创建：`src/app.rs` — 程序状态、管理器初始化、响应处理、命令处理与运行装配。
- 创建：`src/utilities.rs` — 时间、密码与 MD5 的无状态函数。
- 创建：`src/platform/mod.rs` — 目标平台模块导出。
- 创建：`src/platform/linux.rs` — GTK 初始化、Linux 托盘与 GTK 主线程命令调度。
- 创建：`src/platform/windows.rs` — Windows 托盘与搜索窗口。
- 创建：`src/snippet/ui/linux_manager/{mod.rs,tree.rs,dialogs.rs,transfer.rs}` — Linux GTK 片段管理器、树、对话框以及导入导出操作。
- 修改：`src/main.rs` — 只保留模块声明与 `app::run()`。

### 任务 1：批量导入存储 API

**文件：**
- 修改：`src/snippet/store.rs`
- 测试：`src/snippet/store.rs`

- [ ] **步骤 1：编写失败的测试**

```rust
#[test]
fn test_import_batch_adds_unique_items_and_skips_id_and_name_conflicts() {
    let (mut store, _dir) = temp_store();
    let mut existing = Snippet::new_snippet("existing".into(), "one".into(), None);
    existing.id = "existing-id".into();
    store.add(existing).unwrap();

    let mut same_id = Snippet::new_snippet("replacement".into(), "two".into(), None);
    same_id.id = "existing-id".into();
    let duplicate_name = Snippet::new_snippet("existing".into(), "three".into(), None);
    let mut new_item = Snippet::new_snippet("new".into(), "four".into(), None);
    new_item.id = "new-id".into();

    let summary = store.import_batch(vec![same_id, duplicate_name, new_item]).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(summary.skipped_existing_id, 1);
    assert_eq!(summary.skipped_duplicate, 1);
    assert_eq!(store.list().len(), 2);
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- snippet::store::tests::test_import_batch_adds_unique_items_and_skips_id_and_name_conflicts`

预期：FAIL，因 `import_batch` 和 `ImportSummary` 尚未定义。

- [ ] **步骤 3：编写最少实现代码**

```rust
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    pub skipped_existing_id: usize,
    pub skipped_duplicate: usize,
}

pub fn import_batch(&mut self, snippets: Vec<Snippet>) -> Result<ImportSummary, StoreError> {
    // 从当前数据建立 ID 集合和与 add() 相同的唯一性键集合。
    // 遍历输入：ID 已存在优先计为 skipped_existing_id；名称键冲突计为
    // skipped_duplicate；新增项同时写入两集合和 data.snippets。
    // 仅当 added > 0 时执行一次 self.save()。
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- snippet::store::tests::test_import_batch_adds_unique_items_and_skips_id_and_name_conflicts`

预期：PASS。

### 任务 2：后台导入完成消息

**文件：**
- 创建：`src/snippet/ui/linux_manager/transfer.rs`
- 修改：`src/snippet/ui/linux_manager/mod.rs`
- 测试：`src/snippet/ui/linux_manager/transfer.rs`

- [ ] **步骤 1：编写失败的状态映射测试**

```rust
#[test]
fn completed_import_requests_refresh_and_sync() {
    let action = completion_action(Ok(ImportSummary {
        added: 2,
        skipped_existing_id: 1,
        skipped_duplicate: 0,
    }));
    assert!(action.refresh);
    assert!(action.schedule_sync);
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- snippet::ui::linux_manager::transfer::tests::completed_import_requests_refresh_and_sync`

预期：FAIL，因完成消息和映射尚未定义。

- [ ] **步骤 3：实现后台边界**

```rust
// GTK 回调：获取 PathBuf 后立即禁用 import_button，随后只 spawn 工作线程。
std::thread::spawn(move || {
    let completion = import_from_termora(&path)
        .map_err(|e| e.to_string())
        .and_then(|items| app::import_snippets(items));
    completion_tx.send_blocking(completion).ok();
});

// GTK 主线程 async receiver：重新启用按钮、通知、刷新树和安排同步。
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- snippet::ui::linux_manager::transfer::tests::completed_import_requests_refresh_and_sync`

预期：PASS。

### 任务 3：拆分入口与修正 Linux GTK 调度

**文件：**
- 创建：`src/app.rs`、`src/utilities.rs`、`src/platform/mod.rs`、`src/platform/linux.rs`、`src/platform/windows.rs`
- 创建：`src/snippet/ui/linux_manager/mod.rs`、`tree.rs`、`dialogs.rs`
- 修改：`src/main.rs`、`src/snippet/ui/mod.rs`

- [ ] **步骤 1：提取无状态工具和应用装配**

`main.rs` 只保留：

```rust
mod app;
mod platform;
mod snippet;
mod utilities;

fn main() {
    app::run();
}
```

- [ ] **步骤 2：将 Linux GTK UI 调度到已有 GTK 主线程**

`platform::linux::run` 创建 GTK 主线程后，命令接收在 `glib::idle_add_local` 中轮询；`UiCommand::Show` 直接在该线程显示片段窗口，绝不从 `std::thread::spawn` 调用 GTK API。

- [ ] **步骤 3：按 UI 职责移动代码**

- `tree.rs`：树填充和刷新。
- `dialogs.rs`：新建、编辑、删除对话框。
- `transfer.rs`：Termora 文件选择、后台导入与导出。
- `mod.rs`：管理器窗口装配、选择、拖放、CRUD。

- [ ] **步骤 4：完成验证**

运行：`cargo fmt --check && cargo test && cargo build`

预期：全部命令退出码为 0。
