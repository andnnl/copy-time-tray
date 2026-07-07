# 配置窗口实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将单一配置对话框升级为多 tab 配置窗口，支持同步、通用设置和关于 tab

**架构：** 使用 GTK Notebook 实现多 tab 窗口，通过 ConfigTab trait 定义统一接口，每个 tab 独立实现

**技术栈：** GTK3, Rust, serde, chrono

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/snippet/ui/tabs/mod.rs` | 创建 | Tab trait 定义 |
| `src/snippet/ui/tabs/sync_tab.rs` | 创建 | 同步 tab 实现 |
| `src/snippet/ui/tabs/general_tab.rs` | 创建 | 通用设置 tab 实现 |
| `src/snippet/ui/tabs/about_tab.rs` | 创建 | 关于 tab 实现 |
| `src/snippet/ui/config_window.rs` | 创建 | 主配置窗口框架 |
| `src/snippet/ui/mod.rs` | 修改 | 导出新模块 |
| `src/snippet/config.rs` | 修改 | 添加 GeneralConfig 和 SyncStatus |
| `src/main.rs` | 修改 | 替换菜单项，使用 ConfigWindow |

---

### 任务 1：创建 Tab trait 定义

**文件：**
- 创建：`src/snippet/ui/tabs/mod.rs`

- [ ] **步骤 1：创建 tabs 模块目录和 mod.rs**

```bash
mkdir -p src/snippet/ui/tabs
```

- [ ] **步骤 2：编写 Tab trait**

```rust
// src/snippet/ui/tabs/mod.rs
use gtk::prelude::*;

pub mod sync_tab;
pub mod general_tab;
pub mod about_tab;

/// 配置 tab 的统一接口
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

- [ ] **步骤 3：Commit**

```bash
git add src/snippet/ui/tabs/
git commit -m "feat(ui): add ConfigTab trait definition"
```

---

### 任务 2：扩展 Config 数据模型

**文件：**
- 修改：`src/snippet/config.rs`

- [ ] **步骤 1：添加 GeneralConfig 结构体**

在 `config.rs` 的 `GiteeConfig` 之后添加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_true")]
    pub auto_sync: bool,
    #[serde(default = "default_sync_interval")]
    pub sync_interval: u64,
    #[serde(default = "default_true")]
    pub notify_on_success: bool,
    #[serde(default = "default_true")]
    pub notify_on_failure: bool,
    #[serde(default = "default_true")]
    pub notify_on_create: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            auto_sync: true,
            sync_interval: 300,
            notify_on_success: true,
            notify_on_failure: true,
            notify_on_create: true,
        }
    }
}
```

- [ ] **步骤 2：添加 SyncStatus 和 SyncHistoryEntry**

```rust
use chrono::{DateTime, Local};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncDirection {
    Push,
    Pull,
}

impl fmt::Display for SyncDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncDirection::Push => write!(f, "Push"),
            SyncDirection::Pull => write!(f, "Pull"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncResultType {
    Success,
    Failed,
}

impl fmt::Display for SyncResultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncResultType::Success => write!(f, "成功"),
            SyncResultType::Failed => write!(f, "失败"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHistoryEntry {
    pub timestamp: DateTime<Local>,
    pub direction: SyncDirection,
    pub result: SyncResultType,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_sync_time: Option<DateTime<Local>>,
    pub pending_count: usize,
    pub last_sync_result: Option<SyncResultType>,
    #[serde(default)]
    pub history: Vec<SyncHistoryEntry>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            last_sync_time: None,
            pending_count: 0,
            last_sync_result: None,
            history: Vec::new(),
        }
    }
}
```

- [ ] **步骤 3：更新 Config 结构体**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sync: Option<SyncConfig>,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub sync_status: SyncStatus,
}
```

- [ ] **步骤 4：添加 SyncStatus 读写方法**

```rust
impl Config {
    // ... 现有方法 ...

    pub fn sync_status_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("sync_status.json"))
    }

    pub fn load_sync_status() -> Result<SyncStatus, ConfigError> {
        let path = Self::sync_status_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let status: SyncStatus = serde_json::from_str(&content)
                .map_err(|e| ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                )))?;
            Ok(status)
        } else {
            Ok(SyncStatus::default())
        }
    }

    pub fn save_sync_status(status: &SyncStatus) -> Result<(), ConfigError> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)?;
        let content = serde_json::to_string_pretty(status)
            .map_err(|e| ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )))?;
        let path = Self::sync_status_path()?;
        fs::write(&path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    pub fn add_sync_history(&mut self, entry: SyncHistoryEntry) {
        self.sync_status.history.push(entry);
        // 只保留最近 20 条
        if self.sync_status.history.len() > 20 {
            self.sync_status.history = self.sync_status.history.split_off(
                self.sync_status.history.len() - 20
            );
        }
    }
}
```

- [ ] **步骤 5：更新测试**

```rust
#[cfg(test)]
mod tests {
    // ... 现有测试 ...

    #[test]
    fn test_general_config_defaults() {
        let config = GeneralConfig::default();
        assert!(config.auto_sync);
        assert_eq!(config.sync_interval, 300);
        assert!(config.notify_on_success);
        assert!(config.notify_on_failure);
        assert!(config.notify_on_create);
    }

    #[test]
    fn test_sync_status_defaults() {
        let status = SyncStatus::default();
        assert!(status.last_sync_time.is_none());
        assert_eq!(status.pending_count, 0);
        assert!(status.history.is_empty());
    }

    #[test]
    fn test_parse_config_with_general() {
        let yaml = r#"
sync: null
general:
  auto_sync: false
  sync_interval: 600
  notify_on_success: false
  notify_on_failure: true
  notify_on_create: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.general.auto_sync);
        assert_eq!(config.general.sync_interval, 600);
        assert!(!config.general.notify_on_success);
    }
}
```

- [ ] **步骤 6：运行测试验证**

```bash
cargo test --lib snippet::config
```

预期：所有测试通过

- [ ] **步骤 7：Commit**

```bash
git add src/snippet/config.rs
git commit -m "feat(config): add GeneralConfig, SyncStatus, and sync history"
```

---

### 任务 3：创建 SyncTab 实现

**文件：**
- 创建：`src/snippet/ui/tabs/sync_tab.rs`

- [ ] **步骤 1：编写 SyncTab 结构体**

```rust
// src/snippet/ui/tabs/sync_tab.rs
use gtk::prelude::*;
use gtk::{self, glib};
use crate::snippet::config::{
    Config, SyncProvider, GiteeConfig, SyncStatus, SyncHistoryEntry,
    SyncDirection, SyncResultType,
};
use super::ConfigTab;
use std::cell::RefCell;
use std::rc::Rc;

pub struct SyncTab {
    provider_combo: gtk::ComboBoxText,
    github_box: gtk::Box,
    gitee_box: gtk::Box,
    owner_entry: gtk::Entry,
    repo_entry: gtk::Entry,
    branch_entry: gtk::Entry,
    gh_token_entry: gtk::Entry,
    gitee_api_entry: gtk::Entry,
    gitee_token_entry: gtk::Entry,
    status_label: gtk::Label,
    history_box: gtk::Box,
    history_expander: gtk::Expander,
}

impl SyncTab {
    pub fn new() -> Self {
        let provider_combo = gtk::ComboBoxText::new();
        provider_combo.append_text("github");
        provider_combo.append_text("gitee");
        provider_combo.set_active(Some(0));

        // GitHub fields
        let gh_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let owner_entry = Self::create_entry("GitHub 用户名");
        let repo_entry = Self::create_entry("仓库名称");
        let branch_entry = Self::create_entry("分支名称");
        let gh_token_entry = Self::create_password_entry("GitHub Personal Access Token");

        gh_box.pack_start(&Self::create_row("Owner:", &owner_entry), false, false, 0);
        gh_box.pack_start(&Self::create_row("Repo:", &repo_entry), false, false, 0);
        gh_box.pack_start(&Self::create_row("Branch:", &branch_entry), false, false, 0);
        gh_box.pack_start(&Self::create_row("Token:", &gh_token_entry), false, false, 0);

        // Gitee fields
        let gitee_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let gitee_api_entry = Self::create_entry("Gitee API 地址");
        gitee_api_entry.set_text("https://gitee.com/api/v5");
        let gitee_token_entry = Self::create_password_entry("Gitee 私人令牌");

        gitee_box.pack_start(&Self::create_row("API Base:", &gitee_api_entry), false, false, 0);
        gitee_box.pack_start(&Self::create_row("Token:", &gitee_token_entry), false, false, 0);

        // Status area
        let status_label = gtk::Label::new(Some("未配置"));
        status_label.set_xalign(0.0);

        // History area
        let history_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let history_expander = gtk::Expander::new(Some("同步历史"));
        history_expander.add(&history_box);

        // Toggle visibility based on provider
        gitee_box.set_no_show_all(true);
        gitee_box.hide();

        let gitee_clone = gitee_box.clone();
        let gh_clone = gh_box.clone();
        provider_combo.connect_changed(move |combo| {
            let is_gitee = combo.active_text().as_deref() == Some("gitee");
            if is_gitee {
                gitee_clone.set_no_show_all(false);
                gitee_clone.show_all();
                gh_clone.hide();
            } else {
                gh_clone.set_no_show_all(false);
                gh_clone.show_all();
                gitee_clone.hide();
            }
        });

        Self {
            provider_combo,
            github_box: gh_box,
            gitee_box,
            owner_entry,
            repo_entry,
            branch_entry,
            gh_token_entry,
            gitee_api_entry,
            gitee_token_entry,
            status_label,
            history_box,
            history_expander,
        }
    }

    fn create_entry(placeholder: &str) -> gtk::Entry {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(placeholder));
        entry
    }

    fn create_password_entry(placeholder: &str) -> gtk::Entry {
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(placeholder));
        entry.set_visibility(false);
        entry
    }

    fn create_row(label_text: &str, widget: &gtk::Widget) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(0.0);
        label.set_width_chars(10);
        row.pack_start(&label, false, false, 0);
        row.pack_start(widget, true, true, 0);
        row
    }

    fn update_status_display(&self, status: &SyncStatus) {
        if let Some(time) = &status.last_sync_time {
            let result_text = match &status.last_sync_result {
                Some(SyncResultType::Success) => "✓ 成功",
                Some(SyncResultType::Failed) => "✗ 失败",
                None => "未知",
            };
            self.status_label.set_text(&format!(
                "上次同步: {}\n待同步: {} 个片段\n状态: {}",
                time.format("%Y-%m-%d %H:%M:%S"),
                status.pending_count,
                result_text
            ));
        } else {
            self.status_label.set_text("尚未同步");
        }
    }

    fn update_history_display(&self, history: &[SyncHistoryEntry]) {
        // Clear existing children
        for child in self.history_box.children() {
            self.history_box.remove(&child);
        }

        if history.is_empty() {
            let label = gtk::Label::new(Some("暂无同步记录"));
            label.set_xalign(0.0);
            self.history_box.pack_start(&label, false, false, 4);
        } else {
            for entry in history.iter().rev().take(10) {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
                let result_icon = match entry.result {
                    SyncResultType::Success => "✓",
                    SyncResultType::Failed => "✗",
                };
                let text = format!(
                    "{} {} {} {}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    entry.direction,
                    result_icon,
                    entry.result
                );
                let label = gtk::Label::new(Some(&text));
                label.set_xalign(0.0);
                row.pack_start(&label, false, false, 2);

                if let Some(error) = &entry.error {
                    let error_label = gtk::Label::new(Some(&format!("  错误: {}", error)));
                    error_label.set_xalign(0.0);
                    error_label.style_context().add_class("dim-label");
                    row.pack_start(&error_label, false, false, 2);
                }

                self.history_box.pack_start(&row, false, false, 4);
            }
        }
        self.history_box.show_all();
    }
}

impl ConfigTab for SyncTab {
    fn title(&self) -> &str {
        "同步"
    }

    fn build(&self, container: &gtk::Box) {
        // Provider selection
        let provider_row = Self::create_row("Provider:", &self.provider_combo);
        container.pack_start(&provider_row, false, false, 0);

        // GitHub config
        container.pack_start(&self.github_box, false, false, 0);

        // Gitee config
        container.pack_start(&self.gitee_box, false, false, 0);

        // Separator
        container.pack_start(&gtk::Separator::new(gtk::Orientation::Horizontal), false, false, 8);

        // Status area
        let status_frame = gtk::Frame::new(Some("同步状态"));
        status_frame.add(&self.status_label);
        container.pack_start(&status_frame, false, false, 0);

        // History
        container.pack_start(&self.history_expander, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        // Validation is done here
        let provider = match self.provider_combo.active_text() {
            Some(text) if text == "gitee" => SyncProvider::Gitee,
            _ => SyncProvider::Github,
        };

        match provider {
            SyncProvider::Github => {
                let owner = self.owner_entry.text().to_string();
                let repo = self.repo_entry.text().to_string();
                let branch = self.branch_entry.text().to_string();
                let token = self.gh_token_entry.text().to_string();
                if owner.is_empty() || repo.is_empty() || branch.is_empty() || token.is_empty() {
                    return Err("请填写所有 GitHub 字段".to_string());
                }
            }
            SyncProvider::Gitee => {
                let api_base = self.gitee_api_entry.text().to_string();
                let token = self.gitee_token_entry.text().to_string();
                if api_base.is_empty() || token.is_empty() {
                    return Err("请填写 API 地址和令牌".to_string());
                }
            }
        }

        Ok(())
    }

    fn on_load(&self) {
        let config = Config::load().unwrap_or_default();

        if let Some(sync_config) = &config.sync {
            match sync_config.provider {
                SyncProvider::Github => {
                    self.provider_combo.set_active(Some(0));
                    self.owner_entry.set_text(&sync_config.owner);
                    self.repo_entry.set_text(&sync_config.repo);
                    self.branch_entry.set_text(&sync_config.branch);
                    self.gh_token_entry.set_text(&sync_config.token);
                }
                SyncProvider::Gitee => {
                    self.provider_combo.set_active(Some(1));
                    self.gitee_api_entry.set_text(&sync_config.gitee_config.api_base);
                    self.gitee_token_entry.set_text(&sync_config.gitee_config.token);
                }
            }
        }

        // Load sync status
        let status = Config::load_sync_status().unwrap_or_default();
        self.update_status_display(&status);
        self.update_history_display(&status.history);
    }
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/snippet/ui/tabs/sync_tab.rs
git commit -m "feat(ui): implement SyncTab with provider config and status display"
```

---

### 任务 4：创建 GeneralTab 实现

**文件：**
- 创建：`src/snippet/ui/tabs/general_tab.rs`

- [ ] **步骤 1：编写 GeneralTab 结构体**

```rust
// src/snippet/ui/tabs/general_tab.rs
use gtk::prelude::*;
use crate::snippet::config::{Config, GeneralConfig};
use super::ConfigTab;

pub struct GeneralTab {
    auto_sync_switch: gtk::Switch,
    sync_interval_entry: gtk::Entry,
    notify_success_switch: gtk::Switch,
    notify_failure_switch: gtk::Switch,
    notify_create_switch: gtk::Switch,
}

impl GeneralTab {
    pub fn new() -> Self {
        let auto_sync_switch = gtk::Switch::new();
        auto_sync_switch.set_active(true);

        let sync_interval_entry = gtk::Entry::new();
        sync_interval_entry.set_text("300");
        sync_interval_entry.set_width_chars(8);

        let notify_success_switch = gtk::Switch::new();
        notify_success_switch.set_active(true);

        let notify_failure_switch = gtk::Switch::new();
        notify_failure_switch.set_active(true);

        let notify_create_switch = gtk::Switch::new();
        notify_create_switch.set_active(true);

        Self {
            auto_sync_switch,
            sync_interval_entry,
            notify_success_switch,
            notify_failure_switch,
            notify_create_switch,
        }
    }

    fn create_switch_row(label_text: &str, switch: &gtk::Switch) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(0.0);
        row.pack_start(&label, true, true, 0);
        row.pack_start(switch, false, false, 0);
        row
    }
}

impl ConfigTab for GeneralTab {
    fn title(&self) -> &str {
        "通用设置"
    }

    fn build(&self, container: &gtk::Box) {
        // Sync settings
        let sync_frame = gtk::Frame::new(Some("同步设置"));
        let sync_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sync_box.set_margin_top(8);
        sync_box.set_margin_bottom(8);
        sync_box.set_margin_start(8);
        sync_box.set_margin_end(8);

        sync_box.pack_start(
            &Self::create_switch_row("启动时自动同步", &self.auto_sync_switch),
            false, false, 0
        );

        let interval_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let interval_label = gtk::Label::new(Some("同步间隔:"));
        interval_label.set_xalign(0.0);
        interval_row.pack_start(&interval_label, true, true, 0);
        interval_row.pack_start(&self.sync_interval_entry, false, false, 0);
        let sec_label = gtk::Label::new(Some("秒"));
        interval_row.pack_start(&sec_label, false, false, 0);
        sync_box.pack_start(&interval_row, false, false, 0);

        sync_frame.add(&sync_box);
        container.pack_start(&sync_frame, false, false, 0);

        // Notification settings
        let notify_frame = gtk::Frame::new(Some("通知设置"));
        let notify_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        notify_box.set_margin_top(8);
        notify_box.set_margin_bottom(8);
        notify_box.set_margin_start(8);
        notify_box.set_margin_end(8);

        notify_box.pack_start(
            &Self::create_switch_row("同步成功时通知", &self.notify_success_switch),
            false, false, 0
        );
        notify_box.pack_start(
            &Self::create_switch_row("同步失败时通知", &self.notify_failure_switch),
            false, false, 0
        );
        notify_box.pack_start(
            &Self::create_switch_row("创建 Snippet 时通知", &self.notify_create_switch),
            false, false, 0
        );

        notify_frame.add(&notify_box);
        container.pack_start(&notify_frame, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        let interval_text = self.sync_interval_entry.text().to_string();
        let interval: u64 = interval_text.parse().map_err(|_| "同步间隔必须是数字".to_string())?;
        if interval < 10 {
            return Err("同步间隔不能小于 10 秒".to_string());
        }
        Ok(())
    }

    fn on_load(&self) {
        let config = Config::load().unwrap_or_default();
        let general = &config.general;

        self.auto_sync_switch.set_active(general.auto_sync);
        self.sync_interval_entry.set_text(&general.sync_interval.to_string());
        self.notify_success_switch.set_active(general.notify_on_success);
        self.notify_failure_switch.set_active(general.notify_on_failure);
        self.notify_create_switch.set_active(general.notify_on_create);
    }
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/snippet/ui/tabs/general_tab.rs
git commit -m "feat(ui): implement GeneralTab with sync and notification settings"
```

---

### 任务 5：创建 AboutTab 实现

**文件：**
- 创建：`src/snippet/ui/tabs/about_tab.rs`

- [ ] **步骤 1：编写 AboutTab 结构体**

```rust
// src/snippet/ui/tabs/about_tab.rs
use gtk::prelude::*;
use super::ConfigTab;

pub struct AboutTab {
    container: gtk::Box,
}

impl AboutTab {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 12);
        container.set_margin_top(20);
        container.set_margin_bottom(20);
        container.set_margin_start(20);
        container.set_margin_end(20);

        Self { container }
    }
}

impl ConfigTab for AboutTab {
    fn title(&self) -> &str {
        "关于"
    }

    fn build(&self, container: &gtk::Box) {
        // App name
        let name_label = gtk::Label::new(Some("Copy Time Tray"));
        name_label.style_context().add_class("title-1");
        name_label.set_halign(gtk::Align::Center);
        container.pack_start(&name_label, false, false, 0);

        // Version
        let version = env!("CARGO_PKG_VERSION");
        let version_label = gtk::Label::new(Some(&format!("版本: {}", version)));
        version_label.set_halign(gtk::Align::Center);
        container.pack_start(&version_label, false, false, 0);

        // Links
        let links_box = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        links_box.set_halign(gtk::Align::Center);
        links_box.set_margin_top(16);

        let github_link = gtk::LinkButton::with_label(
            "https://github.com/your-username/copy-time-tray",
            Some("GitHub")
        );
        links_box.pack_start(&github_link, false, false, 0);

        container.pack_start(&links_box, false, false, 0);

        // License
        let license_label = gtk::Label::new(Some("MIT License"));
        license_label.set_halign(gtk::Align::Center);
        license_label.set_margin_top(16);
        container.pack_start(&license_label, false, false, 0);

        // Description
        let desc_label = gtk::Label::new(Some(
            "一个简洁的系统托盘工具，用于快速复制时间、生成密码和管理代码片段。"
        ));
        desc_label.set_halign(gtk::Align::Center);
        desc_label.set_line_wrap(true);
        desc_label.set_max_width_chars(40);
        desc_label.set_margin_top(8);
        container.pack_start(&desc_label, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        // About tab has nothing to save
        Ok(())
    }

    fn on_load(&self) {
        // About tab has nothing to load
    }
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/snippet/ui/tabs/about_tab.rs
git commit -m "feat(ui): implement AboutTab with version and links"
```

---

### 任务 6：创建 ConfigWindow 主窗口

**文件：**
- 创建：`src/snippet/ui/config_window.rs`

- [ ] **步骤 1：编写 ConfigWindow 结构体**

```rust
// src/snippet/ui/config_window.rs
use gtk::prelude::*;
use crate::snippet::config::{Config, SyncConfig, SyncProvider, GiteeConfig};
use super::tabs::{ConfigTab, sync_tab, general_tab, about_tab};

pub struct ConfigWindow {
    window: gtk::Dialog,
    notebook: gtk::Notebook,
    sync_tab: sync_tab::SyncTab,
    general_tab: general_tab::GeneralTab,
    about_tab: about_tab::AboutTab,
}

impl ConfigWindow {
    pub fn new(parent: Option<&gtk::Window>) -> Self {
        let window = gtk::Dialog::with_buttons(
            Some("配置"),
            parent,
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            &[
                ("保存", gtk::ResponseType::Ok),
                ("取消", gtk::ResponseType::Cancel),
            ],
        );
        window.set_default_size(450, 500);
        window.set_resizable(true);

        let notebook = gtk::Notebook::new();

        let sync_tab = sync_tab::SyncTab::new();
        let general_tab = general_tab::GeneralTab::new();
        let about_tab = about_tab::AboutTab::new();

        // Build tabs
        let sync_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sync_container.set_margin_top(12);
        sync_container.set_margin_bottom(12);
        sync_container.set_margin_start(12);
        sync_container.set_margin_end(12);
        sync_tab.build(&sync_container);
        notebook.append_page(&sync_container, Some(&gtk::Label::new(Some(sync_tab.title()))));

        let general_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        general_container.set_margin_top(12);
        general_container.set_margin_bottom(12);
        general_container.set_margin_start(12);
        general_container.set_margin_end(12);
        general_tab.build(&general_container);
        notebook.append_page(&general_container, Some(&gtk::Label::new(Some(general_tab.title()))));

        let about_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        about_container.set_margin_top(12);
        about_container.set_margin_bottom(12);
        about_container.set_margin_start(12);
        about_container.set_margin_end(12);
        about_tab.build(&about_container);
        notebook.append_page(&about_container, Some(&gtk::Label::new(Some(about_tab.title()))));

        let content = window.content_area();
        content.pack_start(&notebook, true, true, 0);

        // Load current config into tabs
        sync_tab.on_load();
        general_tab.on_load();
        about_tab.on_load();

        Self {
            window,
            notebook,
            sync_tab,
            general_tab,
            about_tab,
        }
    }

    pub fn show(parent: Option<&gtk::Window>) -> Option<Config> {
        let dialog = Self::new(parent);
        dialog.window.show_all();

        let result = dialog.window.run();
        dialog.window.hide();

        if result == gtk::ResponseType::Ok {
            // Validate all tabs
            if let Err(e) = dialog.sync_tab.on_save() {
                show_error_dialog(parent, &e);
                return None;
            }
            if let Err(e) = dialog.general_tab.on_save() {
                show_error_dialog(parent, &e);
                return None;
            }

            // Build config from tab values
            let sync_config = Some(SyncConfig {
                provider: match dialog.sync_tab.provider_combo.active_text().as_deref() {
                    Some("gitee") => SyncProvider::Gitee,
                    _ => SyncProvider::Github,
                },
                owner: dialog.sync_tab.owner_entry.text().to_string(),
                repo: dialog.sync_tab.repo_entry.text().to_string(),
                branch: dialog.sync_tab.branch_entry.text().to_string(),
                token: dialog.sync_tab.gh_token_entry.text().to_string(),
                gitee_config: GiteeConfig {
                    api_base: dialog.sync_tab.gitee_api_entry.text().to_string(),
                    token: dialog.sync_tab.gitee_token_entry.text().to_string(),
                    auto_sync: true,
                    sync_interval: 300,
                },
            });

            let general_config = crate::snippet::config::GeneralConfig {
                auto_sync: dialog.general_tab.auto_sync_switch.is_active(),
                sync_interval: dialog.general_tab.sync_interval_entry.text().to_string()
                    .parse().unwrap_or(300),
                notify_on_success: dialog.general_tab.notify_success_switch.is_active(),
                notify_on_failure: dialog.general_tab.notify_failure_switch.is_active(),
                notify_on_create: dialog.general_tab.notify_create_switch.is_active(),
            };

            let mut config = Config::load().unwrap_or_default();
            config.sync = sync_config;
            config.general = general_config;

            Some(config)
        } else {
            None
        }
    }
}

fn show_error_dialog(parent: Option<&gtk::Window>, message: &str) {
    let dialog = gtk::Dialog::with_buttons(
        Some("错误"),
        parent,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("确定", gtk::ResponseType::Ok)],
    );
    dialog.set_default_size(300, -1);

    let content = dialog.content_area();
    let label = gtk::Label::new(Some(message));
    label.set_line_wrap(true);
    label.set_max_width_chars(50);
    content.pack_start(&label, true, true, 12);
    content.show_all();

    dialog.run();
    dialog.hide();
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/snippet/ui/config_window.rs
git commit -m "feat(ui): implement ConfigWindow with GTK Notebook tabs"
```

---

### 任务 7：更新 UI 模块导出

**文件：**
- 修改：`src/snippet/ui/mod.rs`

- [ ] **步骤 1：添加新模块导出**

```rust
// src/snippet/ui/mod.rs
pub mod config_dialog;
pub mod config_window;
pub mod search;
pub mod editor;
pub mod tabs;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiCommand {
    Show,
    RefreshSnippets(Vec<crate::snippet::model::Snippet>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiResponse {
    SnippetCopied(String),
    SnippetCreated(crate::snippet::model::Snippet),
    SnippetUpdated(crate::snippet::model::Snippet),
    SnippetDeleted(String),
    RequestPull,
    RequestPush,
    Close,
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/snippet/ui/mod.rs
git commit -m "feat(ui): export config_window and tabs modules"
```

---

### 任务 8：更新 Linux 平台菜单

**文件：**
- 修改：`src/main.rs:189-279`

- [ ] **步骤 1：替换同步菜单项为配置菜单项**

在 `main.rs` 的 `#[cfg(target_os = "linux")]` 模块中，找到并替换：

```rust
// 找到这段代码（约第 189-279 行）
let sync_item = MenuItem::with_label("同步 Snippet");
sync_item.connect_activate(move |_| {
    // ... 整个 sync_item 的 connect_activate 闭包 ...
});
menu.append(&sync_item);

// 替换为：
let config_item = MenuItem::with_label("配置");
config_item.connect_activate(move |_| {
    glib::idle_add(move || {
        if let Some(new_config) = crate::snippet::ui::config_window::ConfigWindow::show(None) {
            if let Err(e) = new_config.save() {
                let _ = notify_rust::Notification::new()
                    .summary("配置保存失败")
                    .body(&e.to_string())
                    .show();
            } else {
                if let Some(mut m) = MANAGER.get().and_then(|m| m.lock().ok()) {
                    let _ = m.reload_config();
                }
                let _ = notify_rust::Notification::new()
                    .summary("配置已保存")
                    .body("配置已更新")
                    .show();
            }
        }
        glib::ControlFlow::Break
    });
});
menu.append(&config_item);
```

- [ ] **步骤 2：Commit**

```bash
git add src/main.rs
git commit -m "feat(linux): replace sync menu item with config menu"
```

---

### 任务 9：更新 Windows 平台菜单

**文件：**
- 修改：`src/main.rs:378-443`

- [ ] **步骤 1：替换同步菜单项为配置菜单项**

在 `main.rs` 的 `#[cfg(target_os = "windows")]` 模块中，找到并替换：

```rust
// 找到这段代码（约第 378-384 行）
let sync_id = muda::MenuId::new("sync_snippets");
let sync_item = MenuItem::with_id(sync_id.clone(), "同步 Snippet", true, None);

// 替换为：
let config_id = muda::MenuId::new("open_config");
let config_item = MenuItem::with_id(config_id.clone(), "配置", true, None);
```

然后在事件处理循环中（约第 417-443 行），找到并替换：

```rust
// 找到这段代码
"sync_snippets" => {
    // ... 整个 sync_snippets 分支 ...
}

// 替换为：
"open_config" => {
    // Windows 暂不支持 ConfigWindow，显示提示
    let _ = notify_rust::Notification::new()
        .summary("配置")
        .body("请在 ~/.config/copy-time-tray/config.yaml 中手动配置")
        .show();
}
```

- [ ] **步骤 2：Commit**

```bash
git add src/main.rs
git commit -m "feat(windows): replace sync menu item with config menu"
```

---

### 任务 10：构建和测试

**文件：**
- 无新增

- [ ] **步骤 1：编译检查**

```bash
cargo build 2>&1
```

预期：编译成功，无错误

- [ ] **步骤 2：运行测试**

```bash
cargo test 2>&1
```

预期：所有测试通过

- [ ] **步骤 3：手动测试 Linux**

```bash
cargo run
```

测试项：
1. 点击 tray 菜单「配置」
2. 验证三个 tab 正常显示
3. 切换 Provider，验证字段显示/隐藏
4. 修改设置并保存
5. 验证配置文件已更新

- [ ] **步骤 4：最终 Commit**

```bash
git add -A
git commit -m "feat: complete config window implementation with tabs"
```

---

## 实现优先级

1. ✅ 任务 1：Tab trait 定义
2. ✅ 任务 2：Config 数据模型扩展
3. ✅ 任务 3：SyncTab 实现
4. ✅ 任务 4：GeneralTab 实现
5. ✅ 任务 5：AboutTab 实现
6. ✅ 任务 6：ConfigWindow 主窗口
7. ✅ 任务 7：UI 模块导出
8. ✅ 任务 8：Linux 菜单更新
9. ✅ 任务 9：Windows 菜单更新
10. ✅ 任务 10：构建和测试
