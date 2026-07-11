# JSON 格式化窗口实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 Linux 和 Windows 的系统托盘菜单中提供 JSON 格式化窗口，默认以 4 个空格美化合法 JSON，并支持剪贴板载入与复制。

**架构：** 新建无 GUI 依赖的 `json_formatter` 模块统一定义格式选项与序列化逻辑；Linux 将其接入 GTK 双栏对话框，Windows 将其接入独立的 eframe 原生窗口。平台窗口只保存输入、输出与状态，所有 JSON 转换都委托共享模块；格式失败仅更新错误提示，绝不清除最后一次成功输出或写入剪贴板。

**技术栈：** Rust 2021、serde_json、GTK 3（Linux）、eframe/egui 与 arboard（Windows）、Rust 内置单元测试。

---

## 计划前提与验收方式

- 剪贴板无文本或访问失败时：保留当前输入，显示窗口内错误提示；复制失败时显示错误且不显示成功反馈。
- 同一平台的 JSON 窗口采用单例：已打开时忽略重复菜单点击，关闭后可再次打开。这与现有 Linux 对话框的单例模式一致，同时避免 Windows 重复 eframe 线程。
- GUI 没有既有自动化测试设施：共享逻辑以单元测试验证；各平台 UI 以 `cargo check`/目标检查和手动冒烟验收验证。

## 文件清单

- 创建：`src/json_formatter.rs` — `JsonFormat` 枚举、格式名称和确定性 `format_json` 函数，以及完整单元测试。
- 修改：`src/main.rs` — 声明共享 JSON 格式化模块。
- 修改：`src/platform/linux.rs` — GTK JSON 格式化窗口、剪贴板操作与托盘“JSON”菜单入口。
- 修改：`src/platform/windows.rs` — eframe JSON 原生窗口、arboard 剪贴板操作、窗口单例和托盘“JSON”菜单入口。
- 创建：`docs/superpowers/plans/2026-07-11-json-formatter-window-implementation.md` — 本计划。

### 任务 1：实现共享 JSON 格式化逻辑

**文件：**
- 创建：`src/json_formatter.rs`
- 修改：`src/main.rs:1-4`
- 测试：`src/json_formatter.rs` 内的 `#[cfg(test)] mod tests`

- [ ] **步骤 1：编写失败的格式化单元测试**

创建 `JsonFormat::{PrettyFourSpaces, PrettyTwoSpaces, Compact, EscapedString}` 的精确输出测试，以及对 `"{invalid"` 的四种模式错误测试。测试使用下列紧凑输入：

```rust
let input = r#"{"name":"小花","items":[1,true]}"#;
assert_eq!(
    format_json(input, JsonFormat::PrettyFourSpaces).unwrap(),
    "{\n    \"name\": \"小花\",\n    \"items\": [\n        1,\n        true\n    ]\n}"
);
assert_eq!(
    format_json(input, JsonFormat::PrettyTwoSpaces).unwrap(),
    "{\n  \"name\": \"小花\",\n  \"items\": [\n    1,\n    true\n  ]\n}"
);
assert_eq!(
    format_json(" { \"a\" : 1 } ", JsonFormat::Compact).unwrap(),
    r#"{"a":1}"#
);
let original = "{\n  \"a\": \"x\"\n}";
assert_eq!(
    format_json(original, JsonFormat::EscapedString).unwrap(),
    serde_json::to_string(original).unwrap()
);
```

同时断言 `JsonFormat::default()` 为 `PrettyFourSpaces`，并让每种模式对非法 JSON 返回 `Err`。

- [ ] **步骤 2：运行测试验证失败**

运行：

```bash
cargo test json_formatter
```

预期：编译失败，原因是 `crate::json_formatter`/`JsonFormat`/`format_json` 尚未定义。

- [ ] **步骤 3：编写最少实现代码**

在 `src/json_formatter.rs` 定义：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JsonFormat {
    #[default]
    PrettyFourSpaces,
    PrettyTwoSpaces,
    Compact,
    EscapedString,
}

impl JsonFormat {
    pub const ALL: [Self; 4] = [
        Self::PrettyFourSpaces,
        Self::PrettyTwoSpaces,
        Self::Compact,
        Self::EscapedString,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::PrettyFourSpaces => "4 空格美化",
            Self::PrettyTwoSpaces => "2 空格美化",
            Self::Compact => "压缩单行",
            Self::EscapedString => "转义为 JSON 字符串",
        }
    }
}
```

实现 `format_json(input, format)`：先以 `serde_json::from_str::<serde_json::Value>(input)?` 验证；压缩使用 `serde_json::to_string(&value)`；两种美化创建 `serde_json::Serializer::with_formatter` 与 `serde_json::ser::PrettyFormatter::with_indent(b"    ")` 或 `b"  "`；转义模式在验证成功后调用 `serde_json::to_string(input)`，确保保留原始空白。

在 `src/main.rs` 的 `mod app;` 之前或之后加入：

```rust
mod json_formatter;
```

- [ ] **步骤 4：运行单元测试验证通过**

运行：

```bash
cargo test json_formatter
```

预期：所有 `json_formatter` 测试通过，且四空格、两空格、紧凑和原文转义输出完全匹配。

- [ ] **步骤 5：提交任务 1**

```bash
git add src/main.rs src/json_formatter.rs
git commit -m "feat: add JSON formatting core"
```

### 任务 2：接入 Linux GTK 格式化窗口

**文件：**
- 修改：`src/platform/linux.rs:1-179`
- 测试：任务 1 的共享逻辑单测；Linux 构建检查

- [ ] **步骤 1：扩展失败测试以锁定 UI 所依赖的格式标签**

在 `src/json_formatter.rs` 增加测试，断言：

```rust
assert_eq!(JsonFormat::ALL.map(JsonFormat::label), [
    "4 空格美化",
    "2 空格美化",
    "压缩单行",
    "转义为 JSON 字符串",
]);
```

这使两个平台的选择器使用同一组顺序稳定的显示名称，默认第一项即四空格美化。

- [ ] **步骤 2：运行标签测试验证失败**

运行：

```bash
cargo test json_formatter::tests::format_labels_are_stable
```

预期：若标签 API 尚未实现，失败；若任务 1 已实现标签 API，则此测试应先以预期通过的方式确认共享契约，而 UI 集成以前必须保持该测试为绿灯。

- [ ] **步骤 3：实现 GTK 对话框与托盘入口**

在 `src/platform/linux.rs`：

1. 导入 `crate::json_formatter::{format_json, JsonFormat}` 以及 GTK 的 `Button`、`ComboBoxText`、`Dialog`、`Label`、`Orientation`、`Paned`、`ScrolledWindow`、`TextView`。
2. 添加 `static JSON_DIALOG_OPEN: AtomicBool`，在 `show_json_formatter()` 入口用 `swap(true, Ordering::SeqCst)` 防重复；在对话框 `connect_destroy` 和关闭按钮回调中重置为 `false`。
3. 用非阻塞 GTK `Dialog` 创建标题为“JSON 格式化”的可调整大小窗口，默认约 `900x600` 并居中。内容区依次包含：格式选择器、水平 `Paned` 双栏、状态错误标签、四个按钮。
4. 选择器按 `JsonFormat::ALL` 逐项 `append_text(format.label())`，`set_active(Some(0))`。格式化回调按 active index 映射到 `JsonFormat::ALL`，不可得时回退 `JsonFormat::default()`。
5. 左侧为可编辑 `TextView`，右侧 `TextView::set_editable(false)`；两者分别装入有标签的滚动容器。格式化成功才以 `output_buffer.set_text(&result)` 更新输出，并清除错误。失败时仅 `status_label.set_text(&error.to_string())`，不修改输出或剪贴板。
6. “从剪贴板粘贴”通过 GTK `Clipboard::wait_for_text()` 读取；有文本时覆盖输入、清除读取错误且不格式化；无文本时保留输入并显示错误。
7. “复制结果”读取输出 buffer；文本为空时显示“没有可复制的格式化结果”，否则 `set_text` + `store` 并显示“已复制格式化结果”。“关闭”销毁对话框，不进行任何剪贴板操作。
8. 在 MD5 菜单项后、分隔线前追加顶级 `MenuItem::with_label("JSON")`，其 activate 回调调用 `show_json_formatter()`。

- [ ] **步骤 4：运行 Linux 编译与共享测试验证通过**

运行：

```bash
cargo test json_formatter && cargo check
```

预期：所有 JSON 单测通过，Linux GTK 编译路径无错误。

- [ ] **步骤 5：手工 Linux 冒烟验收**

运行：

```bash
cargo run
```

在桌面环境中点击托盘“JSON”，检查默认选择为“4 空格美化”；分别确认四种输出、载入剪贴板不自动格式化、复制结果、非法 JSON 保留最后成功输出并显示错误、关闭不写剪贴板。结束应用后继续。

- [ ] **步骤 6：提交任务 2**

```bash
git add src/json_formatter.rs src/platform/linux.rs
git commit -m "feat: add Linux JSON formatter window"
```

### 任务 3：接入 Windows eframe 格式化窗口

**文件：**
- 修改：`src/platform/windows.rs:1-170`
- 测试：任务 1 的共享逻辑单测；Windows 目标检查（若工具链可用）

- [ ] **步骤 1：设计并实现独立窗口状态**

在 `src/platform/windows.rs` 的 Windows 专用代码中新增：

```rust
struct JsonFormatterApp {
    input: String,
    output: String,
    format: JsonFormat,
    error: Option<String>,
    status: Option<String>,
}
```

并实现 `Default`，使 `format` 为 `JsonFormat::PrettyFourSpaces`。新增 `static JSON_WINDOW_OPEN: AtomicBool`。`launch_json_formatter()` 使用 `compare_exchange(false, true, ...)`；成功后 `std::thread::spawn` 调用 `eframe::run_native`，窗口退出后无论结果如何都把标记设回 `false`。这个函数只在新线程执行，菜单事件接收线程不被阻塞。

- [ ] **步骤 2：实现 eframe 界面和剪贴板交互**

实现 `eframe::App for JsonFormatterApp`：

1. 在 `egui::CentralPanel` 中显示 `ComboBox::from_label("格式方式")`，遍历 `JsonFormat::ALL`；默认值显示 `self.format.label()`。
2. 使用 `ui.columns(2, ...)`：左边 `TextEdit::multiline(&mut self.input)`，右边将 `TextEdit::multiline(&mut self.output).interactive(false)` 放入滚动区域，均以无限可用宽度展示。
3. “从剪贴板粘贴”以 `Clipboard::new().and_then(|mut c| c.get_text())` 读取：成功覆盖 `input` 并清除旧错误，失败保留 `input` 并填充 `error`。
4. “格式化”调用 `format_json(&self.input, self.format)`：成功覆盖 `output`、清除错误并将状态设为“格式化成功”；失败只写入 `error`，不修改 `output`。
5. “复制结果”在 `output.is_empty()` 时设置错误；否则以 `Clipboard::new()` + `set_text(&self.output)` 写入，成功才设置“已复制格式化结果”，失败设置错误。
6. 显示 `error`（红色）或 `status`（正常状态文本）；“关闭”发送 `egui::ViewportCommand::Close`。

- [ ] **步骤 3：接入 Windows 菜单**

在 `main`：

1. 创建 `muda::MenuId::new("open_json_formatter")` 和 `MenuItem::with_id(..., "JSON", true, None)`。
2. 将 JSON 项放在配置、代码片断项目附近的顶级菜单中，不删除原项目。
3. 在 `MenuEvent` 的 match 中增加 `"open_json_formatter" => launch_json_formatter()` 分支。调用仅负责请求/启动线程，确保接收循环继续处理其余菜单项。

- [ ] **步骤 4：运行 Windows 目标检查与共享测试验证通过**

运行：

```bash
cargo test json_formatter
cargo check --target x86_64-pc-windows-gnu
```

预期：共享逻辑测试全绿；若已安装 Windows GNU target 和 linker，Windows 条件编译路径检查通过。若环境缺 target/linker，记录确切环境错误，但不把它误报为源码错误。

- [ ] **步骤 5：手工 Windows 冒烟验收**

在 Windows 上运行应用，确认点击“JSON”打开独立窗口且托盘菜单仍能处理时间、密码、MD5、代码片断和退出；按规格逐项验证格式、剪贴板、错误保留和默认四空格行为。

- [ ] **步骤 6：提交任务 3**

```bash
git add src/platform/windows.rs
git commit -m "feat: add Windows JSON formatter window"
```

### 任务 4：完整回归验证与规格核对

**文件：**
- 修改：仅在验证发现缺陷时，最小化修改 `src/json_formatter.rs`、`src/platform/linux.rs` 或 `src/platform/windows.rs`
- 测试：全量 Rust 测试、Linux 构建、可用时 Windows 目标检查

- [ ] **步骤 1：运行完整自动化验证**

运行：

```bash
cargo fmt --check
cargo test
cargo build
cargo check --target x86_64-pc-windows-gnu
```

预期：格式、全量测试和本机 Linux 构建通过；Windows 检查通过或仅因本机未安装 target/linker 而无法执行，并保留原始输出作为证据。

- [ ] **步骤 2：逐项核对设计规格**

对 `docs/superpowers/specs/2026-07-11-json-formatter-window-design.md` 的验收标准 1–6 逐项核对：双平台顶级菜单、默认 4 空格、四种精确格式、读写剪贴板、错误时保留输入和最后输出、共享逻辑的成功/失败单测及构建测试。

- [ ] **步骤 3：审查工作树状态并提交验证修复（如有）**

运行：

```bash
git status --short
git diff --check
```

若验证阶段有未提交修复：

```bash
git add src/json_formatter.rs src/platform/linux.rs src/platform/windows.rs
git commit -m "fix: verify JSON formatter integration"
```

若无修复，不创建空提交。
