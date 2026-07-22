# 代码片段窗口最大化实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 代码片段窗口同时支持系统最大化、界面内最大化/还原按钮、标题栏双击切换，并记住上次窗口状态。

**架构：** 在通用配置中增加代码片段窗口状态，Linux GTK 窗口和 Windows eframe 窗口都以该状态初始化并在状态变化/隐藏时保存。原生窗口保持可调整大小并启用系统最大化按钮；界面按钮只调用原生最大化命令，按钮文案跟随真实窗口状态。

**技术栈：** Rust 2021、GTK 3 (`gtk` 0.18)、egui/eframe 0.29、Serde YAML。

---

### 任务 1：窗口状态配置

**文件：**
- 修改：`src/snippet/config.rs`

- [ ] 先添加旧配置默认值、最大化状态和普通窗口几何信息往返序列化测试。
- [ ] 运行 `cargo test snippet_window`，确认因字段/类型缺失失败。
- [ ] 添加 `SnippetWindowState` 及 `GeneralConfig.snippet_window`，默认尺寸为 800×550，位置可选，默认非最大化。
- [ ] 再次运行单项测试确认通过。

### 任务 2：Linux GTK 最大化交互

**文件：**
- 修改：`src/snippet/ui/linux_manager/mod.rs`

- [ ] 添加窗口能力、按钮文案和状态转换的纯函数测试并确认失败。
- [ ] 将对话框设为普通、可调整大小的顶层窗口，使用 HeaderBar 提供最大化/还原按钮并保留系统窗口按钮。
- [ ] 监听真实最大化状态同步按钮；普通状态下记录尺寸和位置；关闭、Esc、复制隐藏前持久化状态。
- [ ] 恢复保存的普通几何信息，并在显示后恢复最大化。
- [ ] 运行 `cargo test linux_manager`。

### 任务 3：Windows eframe 最大化交互

**文件：**
- 修改：`src/app.rs`
- 修改：`src/snippet/ui/search.rs`

- [ ] 添加最大化按钮行为和状态更新测试并确认失败。
- [ ] NativeOptions 启用系统最大化按钮，并从配置恢复尺寸、位置与最大化状态。
- [ ] 将代码片段内容置于原生窗口主面板，右上角提供最大化/还原按钮，调用 `ViewportCommand::Maximized`；原生标题栏双击由系统处理。
- [ ] 读取 `ViewportInfo` 同步真实状态，并在变化或隐藏时保存。
- [ ] 运行 `cargo test search`。

### 任务 4：验证

- [ ] 运行 `cargo fmt -- --check`。
- [ ] 运行 `cargo test`。
- [ ] 运行 `cargo build`。
- [ ] 检查 diff，确保没有覆盖工作区内既有未提交改动。
