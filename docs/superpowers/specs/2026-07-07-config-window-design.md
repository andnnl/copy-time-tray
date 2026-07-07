# 配置窗口设计规格

## 概述

重新设计 Copy Time Tray 的配置系统，将现有单一配置对话框升级为多 tab 配置窗口。主要变更：

1. 将 tray 菜单中的「同步 Snippet」替换为「配置」菜单项
2. 使用 GTK Notebook 实现多 tab 配置窗口
3. 三个 tab：同步、通用设置、关于
4. 详细同步状态显示（历史日志、错误详情）

## 架构

### 文件结构

```
src/snippet/ui/
├── mod.rs                 # 已有
├── config_window.rs       # 新增：主配置窗口（GTK Notebook）
├── tabs/
│   ├── mod.rs             # Tab trait 定义
│   ├── sync_tab.rs        # 同步 tab
│   ├── general_tab.rs     # 通用设置 tab
│   └── about_tab.rs       # 关于 tab
├── config_dialog.rs       # 保留（向后兼容）
├── search.rs              # 已有
└── editor.rs              # 已有
```

### Tab Trait

```rust
pub trait ConfigTab {
    /// Tab 显示标题
    fn title(&self) -> &str;

    /// 构建 tab 内容到容器中
    fn build(&self, container: &gtk::Box);

    /// 保存配置时调用
    fn on_save(&self) -> Result<(), String>;

    /// 加载配置时调用
    fn on_load(&self);
}
```

### ConfigWindow

```rust
pub struct ConfigWindow {
    window: gtk::Dialog,
    notebook: gtk::Notebook,
    tabs: Vec<Box<dyn ConfigTab>>,
    config: Config,
}
```

## 同步 Tab

### 布局

```
┌─────────────────────────────────────┐
│ Provider: [GitHub ▾]                │
├─────────────────────────────────────┤
│ GitHub 配置区域                      │
│ Owner: [________________]           │
│ Repo:  [________________]           │
│ Branch: [_______________]           │
│ Token:  [________________]          │
│         [测试连接]                   │
├─────────────────────────────────────┤
│ 同步状态                            │
│ 上次同步: 2026-07-07 14:30:22      │
│ 待同步: 3 个片段                    │
│ 状态: ✓ 成功                        │
├─────────────────────────────────────┤
│ 同步历史 [展开]                     │
│ ▸ 2026-07-07 14:30:22 Push 成功    │
│ ▸ 2026-07-07 14:25:10 Pull 成功    │
│ ▸ 2026-07-07 14:20:05 Push 失败    │
│   错误: Connection timeout         │
└─────────────────────────────────────┘
```

### 功能

1. **Provider 选择**：下拉框选择 GitHub 或 Gitee
2. **配置字段**：
   - GitHub：Owner、Repo、Branch、Token（密码模式）
   - Gitee：API Base、Token（密码模式）
3. **测试连接**：点击按钮验证配置，显示结果通知
4. **同步状态区域**：
   - 上次同步时间
   - 待同步片段数量
   - 最后同步结果（成功/失败）
5. **同步历史**：可展开的列表，显示最近 10 条同步记录
6. **错误详情**：失败记录可展开查看详细错误信息

## 通用设置 Tab

### 布局

```
┌─────────────────────────────────────┐
│ 同步设置                            │
│ □ 启动时自动同步                    │
│ 同步间隔: [300] 秒                  │
├─────────────────────────────────────┤
│ 通知设置                            │
│ □ 同步成功时通知                    │
│ □ 同步失败时通知                    │
│ □ 创建 Snippet 时通知              │
└─────────────────────────────────────┘
```

### 配置项

```rust
pub struct GeneralConfig {
    pub auto_sync: bool,          // 默认 true
    pub sync_interval: u64,       // 默认 300 秒
    pub notify_on_success: bool,  // 默认 true
    pub notify_on_failure: bool,  // 默认 true
    pub notify_on_create: bool,   // 默认 true
}
```

## 关于 Tab

### 布局

```
┌─────────────────────────────────────┐
│           Copy Time Tray            │
│                                     │
│           版本: 0.4.0               │
│                                     │
│    [GitHub]    [Gitee]              │
│                                     │
│         MIT License                 │
└─────────────────────────────────────┘
```

## 数据模型

### Config 扩展

```rust
pub struct Config {
    pub sync: Option<SyncConfig>,
    pub general: Option<GeneralConfig>,
    pub sync_status: Option<SyncStatus>,
}

pub struct SyncStatus {
    pub last_sync_time: Option<DateTime<Local>>,
    pub pending_count: usize,
    pub last_sync_result: Option<SyncResult>,
    pub history: Vec<SyncHistoryEntry>,
}

pub struct SyncHistoryEntry {
    pub timestamp: DateTime<Local>,
    pub direction: SyncDirection,
    pub result: SyncResult,
    pub error: Option<String>,
}

pub enum SyncDirection {
    Push,
    Pull,
}

pub enum SyncResult {
    Success,
    Failed,
}
```

### 存储位置

- 配置文件：`~/.config/copy-time-tray/config.yaml`
- 同步状态：`~/.config/copy-time-tray/sync_status.json`
- 历史日志：`~/.config/copy-time-tray/sync_history.json`

## 菜单变更

### Linux

```rust
// 移除
let sync_item = MenuItem::with_label("同步 Snippet");

// 新增
let config_item = MenuItem::with_label("配置");
config_item.connect_activate(move |_| {
    glib::idle_add(move || {
        if let Some(result) = config_window::show_config_window(None) {
            // 保存配置
        }
        glib::ControlFlow::Break
    });
});
```

### Windows

```rust
// 移除
let sync_id = muda::MenuId::new("sync_snippets");

// 新增
let config_id = muda::MenuId::new("open_config");
let config_item = MenuItem::with_id(config_id.clone(), "配置", true, None);
```

## 错误处理

1. **配置验证**：在保存前验证所有必填字段
2. **Token 安全**：Token 字段使用密码模式显示
3. **文件权限**：配置文件权限设置为 600（仅用户可读写）
4. **向后兼容**：保留 `config_dialog.rs`，新代码使用 `config_window.rs`

## 测试策略

1. 单元测试：Config 序列化/反序列化
2. 集成测试：ConfigWindow 生命周期
3. 手动测试：UI 交互验证

## 实现优先级

1. ConfigWindow 框架 + Tab trait
2. 同步 Tab（迁移现有功能）
3. 通用设置 Tab
4. 关于 Tab
5. 菜单变更
6. 同步状态存储
7. 错误处理和测试
