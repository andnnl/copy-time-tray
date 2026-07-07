# Gitee Codes 双向同步设计规格

**日期**: 2026-07-07
**状态**: 设计阶段
**目标**: 实现 copy-time-tray 与 Gitee 代码片段（gitee.com/dashboard/codes）的双向同步

---

## 1. 概述

本规格描述 copy-time-tray 的下一代同步机制：**双向同步**到 Gitee 代码片段服务。取代原有的 Git 仓库同步方案（使用 Gitee 代码片段 API 替代）。

### 1.1 设计原则

- **双向同步**:本地和 Gitee 均可修改，自动同步
- **多层文件夹**: 支持任意层级的文件夹嵌套（当前仅支持两层）
- **时间戳优先**:冲突解决策略，较新修改胜出
- **全自动触发**:启动时拉取、本地变更时推送（2秒防抖）
- **弱网友好**: 网络故障时重试3次后取消

---

## 2. 数据模型变更

### 2.1 Snippet 结构体新增字段

**文件**: `src/snippet/model.rs`

```rust
pub struct Snippet {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub snippet_type: SnippetType,
    pub content: Option<String>,
    pub sort: i32,
    pub deleted: bool,
    pub synced: bool,
    pub create_date: DateTime<Utc>,
    pub update_date: DateTime<Utc>,
    
    // 新增字段
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gitee_gist_id: Option<String>,  // 文件夹 -> Gitee 代码片段的映射 ID
}
```

**说明**:
- 仅文件夹类型使用 `gitee_gist_id` 字段
- 对于 snippet 类型，该字段始终为 None
- serde 注解确保向后兼容（旧数据可加载）

### 2.2 根级 snippet 默认分组

所有 `parent_id = None` 且 `snippet_type = Snippet` 的条目在同步时归入虚拟的 `_default` 文件夹：

```
本地结构                    Gitee 代码片段
─────────                  ──────────────
根级 snippet "hello"     ----> _default snippet
根级 snippet "world"          (文件: hello.sh, world.sh)
```

### 2.3 多层文件夹同步映射

支持任意层级的文件夹嵌套。每个文件夹映射为一个 Gitee 代码片段。

**示例**:
```
本地结构                          Gitee 代码片段
─────────                        ──────────────
📁 网络工具/                     -----> "网络工具" (gist_id: abc123)
  ├─ ping                           (文件: ping.sh, curl.sh)
  ├─ curl
  └─ 📁 高级/                   -----> "网络工具/高级" (gist_id: def456)
      ├─ tcpdump                    (文件: tcpdump.sh, nmap.sh)
      └─ nmap

📁 开发/                         -----> "开发" (gist_id: ghi789)
  ├─ git status                   (文件: git_status.sh, ...)
  └─ ...

📁 _default (虚拟)              -----> "_default" (gist_id: jkl012)
  ├─ hello                        (文件: hello.sh, world.sh)
  └─ world
```

---

## 3. 同步逻辑

### 3.1 双向同步流程

#### 3.1.1 推送（Push）流程

1. **检测本地变更**: 查找 `deleted=true` 或 `update_date > last_sync_time` 的条目
2. **按文件夹分组**: 每个文件夹及其所有子 snippet 打包为一个 Gitee 代码片段
3. **调用 Gitee API**:
   - 新建文件夹: `POST /api/v5/gists`
   - 更新文件夹: `PATCH /api/v5/gists/{gist_id}`
   - 删除文件夹: `DELETE /api/v5/gists/{gist_id}`
4. **更新本地状态**: 设置 `gitee_gist_id` 和 `synced=true`

#### 3.1.2 拉取（Pull）流程

1. **获取远程列表**: `GET /api/v5/gists` 获取所有代码片段
2. **匹配本地文件夹**: 按 `gitee_gist_id` 进行匹配
3. **冲突解决策略**:
   - 本地有、远程有: 比较 `update_date`，较新者胜出
   - 本地无、远程有: 创建本地文件夹和子 snippet
   - 本地有、远程无: 标记本地为 `deleted=true`

#### 3.1.3 冲突解决实现

```rust
fn resolve_conflict(local: &Snippet, remote: &GistFile) -> SyncDecision {
    if local.update_date > remote.updated_at {
        SyncDecision::KeepLocal
    } else if local.update_date < remote.updated_at {
        SyncDecision::KeepRemote
    } else {
        SyncDecision::KeepLocal  // 时间戳相同时本地优先
    }
}
```

### 3.2 Gitee API 数据结构

**Gitee 代码片段响应示例**:

```json
{
  "id": "gist_id_123",
  "description": "网络工具",
  "files": {
    "ping.sh": {
      "filename": "ping.sh",
      "content": "ping -c 4 8.8.8.8"
    },
    "curl.sh": {
      "filename": "curl.sh",
      "content": "curl -I https://example.com"
    }
  },
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-02T00:00:00Z"
}
```

### 3.3 自动触发机制

**触发时机**:
- **启动时**: 调用 `pull_all()` 拉取远程变更
- **本地变更后**: 每次 `save()` 后触发 `push_changed()`（2秒防抖）

**防抖实现**:
```rust
// 使用 tokio 延迟执行
tokio::spawn(async move {
    tokio::time::sleep(Duration::from_secs(2)).await;
    push_changed().await;
});
```

### 3.4 弱网重试机制

**重试策略**: 指数退避，最多重试3次

```rust
const MAX_RETRIES: usize = 3;
const RETRY_DELAY_MS: u64 = 1000;

async fn push_with_retry(folder: &Folder) -> Result<(), SyncError> {
    for attempt in 0..MAX_RETRIES {
        match push_folder(folder).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if attempt == MAX_RETRIES - 1 {
                    return Err(e);  // 最后一次失败，取消同步
                }
                tokio::time::sleep(
                    Duration::from_millis(RETRY_DELAY_MS * 2u64.pow(attempt as u32))
                ).await;
            }
        }
    }
    Err(SyncError::RetryExhausted)
}
```

---

## 4. 错误处理

### 4.1 错误分类与处理

| 错误类型 | 处理方式 | 用户提示 |
|---------|---------|---------|
| 网络超时/弱网 | 重试3次后取消 | 托盘气泡: "同步失败: 网络问题，已重试3次" |
| 认证失败 (401/403) | 立即停止，不重试 | 托盘气泡: "同步失败: 请检查Token配置" |
| 解析失败 | 跳过该条目，记录日志 | 托盘气泡: "同步部分失败: N个条目解析错误" |
| API 限流 (429) | 退避等待后重试 | 托盘气泡: "同步暂停: API调用频率过高" |
| 服务器错误 (5xx) | 重试3次后取消 | 托盘气泡: "同步失败: Gitee 服务异常" |

### 4.2 部分同步语义

- 多文件夹同步时，单个文件夹失败不影响其他文件夹
- 失败条目记录到日志，下次同步时重试
- 成功条目立即更新 `synced=true` 和 `gitee_gist_id`
- 用户可通过托盘气泡和状态栏了解同步结果

---

## 5. UI 集成

### 5.1 托盘图标状态

**Linux 实现**:
- 正常: 默认图标
- 同步中: 叠加旋转动画或附加小圆点
- 同步失败: 红色感叹号叠加

### 5.2 状态栏消息

**管理器对话框底部**:
```
上次同步: 2024-01-15 14:30:22 | 同步状态: ✓ 正常
上次同步: 2024-01-15 14:30:22 | 同步状态: ✗ 网络错误
同步中...
```

### 5.3 配置对话框改造

**文件**: `src/snippet/ui/config_dialog.rs`

**新增 UI 元素**:
- Gitee API 地址输入框 (默认 `https://gitee.com/api/v5`)
- Token 输入框 (密码类型，保存到 config.yaml)
- "测试连接" 按钮 (验证 Token 有效性)
- "立即同步" 按钮 (手动触发一次)
- 同步状态显示 (上次同步时间、成功/失败数)

### 5.4 通知气泡

**使用 notify-rust 库**:
- 同步成功: "已同步 X 个文件夹到 Gitee"
- 同步部分成功: "已同步 8/10 个文件夹，2 个失败"
- 同步失败: "同步失败: 网络问题"
- 认证失败: "同步失败: 请检查Token配置"

---

## 6. 代码模块重构

### 6.1 文件变更范围

| 文件 | 变更类型 | 内容 |
|-----|---------|-----|
| `snippet/model.rs` | 修改 | 新增 `gitee_gist_id` 字段 |
| `snippet/sync.rs` | **完全重写** | 替换 Git 仓库同步为 Gitee 代码片段 API |
| `snippet/config.rs` | 修改 | 新增 Gitee API 相关配置字段 |
| `snippet/mod.rs` | 修改 | 同步调度逻辑、防抖、重试机制 |
| `main.rs` (refresh_tree_store) | **重写** | 支持多层文件夹递归渲染 |
| `main.rs` (托盘菜单) | 修改 | 添加同步状态指示器 |
| `snippet/ui/config_dialog.rs` | 修改 | 配置对话框 UI 改造 |

### 6.2 核心模块职责

#### 6.2.1 `snippet/sync.rs` (新增)

- `GiteeClient`: Gitee API 客户端封装
- `push_folders()`: 推送本地文件夹到 Gitee
- `pull_folders()`: 从 Gitee 拉取远程文件夹
- `resolve_conflict()`: 时间戳优先冲突解决
- `push_with_retry()`: 带重试的推送逻辑

#### 6.2.2 `snippet/mod.rs` (修改)

- `start_background_sync()`: 启动时自动拉取
- `push_on_change()`: 本地变更后防抖推送
- `sync_status()`: 同步状态查询
- `test_connection()`: 测试 Token 有效性

#### 6.2.3 `main.rs` (修改)

- `refresh_tree_store()`: 递归构建多层文件夹树
- 托盘图标状态管理
- 配置对话框入口

---

## 7. 配置字段

### 7.1 config.yaml 新增字段

```yaml
sync:
  provider: gitee           # 固定为 gitee (代码片段)
  gitee_config:
    api_base: https://gitee.com/api/v5
    token: xxxxxxxxxxxxxxxx
    auto_sync: true         # 是否启用自动同步
    sync_interval: 300      # 同步间隔 (秒，可选，用于定时轮询)
```

### 7.2 向后兼容

- 旧配置格式仍然可以加载
- `gitee_config` 为可选字段
- 如果 `gitee_config` 不存在，显示配置对话框引导用户配置

---

## 8. 实现计划

### 8.1 阶段一: 数据模型与配置

**任务**:
1. 修改 `snippet/model.rs`，添加 `gitee_gist_id` 字段
2. 修改 `snippet/config.rs`，添加 Gitee API 配置结构
3. 更新 `config.yaml` 读写逻辑

### 8.2 阶段二: Gitee API 客户端

**任务**:
1. 实现 `GiteeClient` 结构
2. 实现 gist CRUD API 调用 (GET/POST/PATCH/DELETE)
3. 实现列表获取、文件上传、文件删除

### 8.3 阶段三: 同步逻辑

**任务**:
1. 实现 `push_folders()`: 本地 -> Gitee
2. 实现 `pull_folders()`: Gitee -> 本地
3. 实现冲突解决逻辑
4. 实现重试机制和防抖

### 8.4 阶段四: UI 集成

**任务**:
1. 重构 `refresh_tree_store()` 支持多层文件夹
2. 修改配置对话框，添加 Gitee 配置表单
3. 添加托盘图标状态指示
4. 添加通知气泡

### 8.5 阶段五: 测试与调试

**任务**:
1. 单元测试: GiteeClient API 调用
2. 集成测试: 双向同步流程
3. 手动测试: 多层文件夹、弱网重试
4. 文档: 用户指南和配置说明

---

## 9. 验收标准

- [ ] 支持多层文件夹嵌套（任意深度）
- [ ] 本地修改后自动推送到 Gitee（2秒防抖）
- [ ] 启动时自动拉取 Gitee 最新数据
- [ ] 冲突解决采用时间戳优先策略
- [ ] 网络故障时重试3次后取消，并提示用户
- [ ] 配置对话框支持 Token 输入和测试连接
- [ ] 托盘图标显示同步状态
- [ ] 根级 snippet 自动归入 `_default` 分组
- [ ] 每个文件夹映射为一个 Gitee 代码片段（多文件形式）

---

## 10. 风险与缓解

### 10.1 API 限流

**风险**: Gitee API 有调用频率限制
**缓解**: 实现退避重试，记录限流日志，提示用户

### 10.2 数据丢失

**风险**: 同步冲突可能导致数据丢失
**缓解**: 时间戳优先策略，本地优先（时间戳相同时），日志记录所有变更

### 10.3 向后兼容

**风险**: 升级后旧配置可能不兼容
**缓解**: serde 默认值机制，迁移脚本（如有必要），用户引导配置

---

## 11. 未来扩展

1. **支持其他平台**: GitHub Gist、GitLab Snippets
2. **版本历史**: 保留 Gitee gist 的历史版本
3. **选择性同步**: 用户可以标记某些文件夹为 "不同步"
4. **离线模式**: 本地修改暂存，网络恢复后自动同步
