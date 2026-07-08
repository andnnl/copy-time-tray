# 单一 Gist 同步设计规格

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to create implementation plan.

**日期**: 2026-07-08
**状态**: 设计完成，待实现
**目标**: 将整个 snippet 树同步到一个 "CopyTimeTrayConfig" Gitee gist，替代当前的"一个文件夹一个 gist"方案

---

## 1. 概述

### 1.1 设计原则

- **单一 gist**: 所有 snippets（文件夹 + snippet）打包到一个 gist
- **多文件存储**: tree.json 描述树结构 + N 个内容文件
- **时间戳优先**: 冲突解决策略，较新修改胜出
- **直接查询**: 存储 gist_id，通过 GET `/gists/{id}` 直接查询（避免遍历）
- **TDD 先行**: 先测试 sync.rs 核心函数，再集成 UI

### 1.2 与旧设计对比

| 维度 | 旧设计 | 新设计 |
|-----|-------|-------|
| Gist 数量 | N 个（每个文件夹一个） | 1 个（整棵树） |
| gitee_gist_id | 每个文件夹记录自己的 | deprecated（保留向后兼容） |
| Gist 查询 | 遍历所有 gist 过滤 | 直接 GET `/gists/{id}` |
| 文件命名 | `{name}.sh` | `{id}_{name}.txt` |
| 树结构保存 | 每个文件夹独立 gist | tree.json + 内容文件 |

---

## 2. 数据模型变更

### 2.1 GiteeConfig 新增字段

**文件**: `src/snippet/config.rs`

```rust
pub struct GiteeConfig {
    pub api_base: String,
    pub token: String,
    pub auto_sync: bool,
    pub sync_interval: i32,
    
    // 新增
    pub gist_id: Option<String>,  // CopyTimeTrayConfig gist ID
}
```

**配置文件 (config.yaml)**:
```yaml
sync:
  provider: gitee
  gitee_config:
    api_base: https://gitee.com/api/v5
    token: xxxxxx
    auto_sync: true
    sync_interval: 300
    gist_id: abc123def456  # 首次同步后自动保存
```

### 2.2 Snippet 结构体调整

**文件**: `src/snippet/model.rs`

`gitee_gist_id` 字段保留但不再使用（向后兼容）：
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub gitee_gist_id: Option<String>,  // deprecated
```

### 2.3 TreeEntry 序列化格式

**tree.json 格式**:
```json
[
  {
    "id": "uuid1",
    "parent_id": null,
    "name": "网络工具",
    "type": "folder",
    "sort": 0,
    "deleted": false,
    "synced": true,
    "create_date": "2024-01-01T00:00:00Z",
    "update_date": "2024-01-02T00:00:00Z"
  },
  {
    "id": "uuid2",
    "parent_id": "uuid1",
    "name": "ping",
    "type": "snippet",
    "content_file": "uuid2_ping.txt",
    "sort": 0,
    ...
  }
]
```

**文件命名规则**:
- Snippet 类型: `{id}_{name}.txt`（UUID 保证唯一，name 提供可读性）
- Folder 类型: 无内容文件（只在 tree.json 中记录）

---

## 3. 同步流程

### 3.1 Push 流程（本地 → Gitee）

**触发时机**:
- 本地 snippets.json 有变更后（create/update/delete）
- 2 秒防抖后触发
- 失败时 3 次重试（指数退避）

**Push 步骤**:

```
1. 构建 tree.json
   └─ 序列化所有 snippets 为 TreeEntry 数组
   
2. 构建文件 map
   └─ HashMap<String, String>
   └─ "tree.json" → JSON 字符串
   └─ "{id}_{name}.txt" → snippet.content
   
3. 更新或创建 gist
   └─ 有 gist_id → PATCH /gists/{id}
   └─ 无 gist_id → POST /gists
   └─ 保存 gist_id 到 config
   
4. 更新本地状态
   └─ synced = true
```

**关键代码位置**: `src/snippet/sync.rs::push_to_single_gist()`

### 3.2 Pull 流程（Gitee → 本地）

**触发时机**:
- 应用启动时自动 pull
- 用户手动点击"同步"

**Pull 步骤**:

```
1. 获取 gist（直接查询）
   └─ GET /gists/{gist_id}
   
2. 解析 tree.json
   └─ 从 gist.files["tree.json"] 解析
   
3. 填充 content
   └─ 对于 snippet 类型，读取 "{id}_{name}.txt"
   
4. 合并到本地（冲突解决）
   └─ 时间戳优先（update_date 较新者胜出）
   └─ 时间戳相同 → 本地优先
   
5. 保存本地
   └─ store.save()
```

**关键代码位置**: `src/snippet/sync.rs::pull_from_single_gist()`

---

## 4. 冲突解决策略

### 4.1 时间戳优先（LWW）

**规则**:
- 本地有、远程有 → 比较 `update_date`，较新者胜出
- 时间戳相同 → 本地优先（保留用户意图）
- 本地无、远程有 → 添加到本地
- 本地有、远程无 → 删除本地（或标记 deleted）

**实现位置**: `src/snippet/store.rs::load_from_sync()`

```rust
for remote in remote_snippets {
    if let Some(local) = find_by_id(&remote.id) {
        if remote.update_date > local.update_date {
            *local = remote;  // 远程较新，覆盖本地
        }
        // 时间戳相同 → 保留本地
    } else {
        self.snippets.push(remote);  // 本地无，添加
    }
}
```

---

## 5. 错误处理

| 错误类型 | 处理方式 | 用户提示 |
|---------|---------|---------|
| 网络超时 | 重试 3 次后取消 | "同步失败: 网络问题" |
| 认证失败 (401/403) | 立即停止 | "同步失败: 请检查 Token" |
| gist_id 无效 | 重新创建 gist | "同步失败: gist 不存在，已重新创建" |
| tree.json 解析失败 | 跳过本次同步 | "同步失败: 数据格式错误" |

---

## 6. 实现计划（按 TDD）

### 6.1 Wave 1: GiteeClient 新增方法

**任务**:
- Add `get_gist(gist_id: &str) -> Result<Gist>` to `gitee/client.rs`
- Unit test: mock HTTP, verify GET `/gists/{id}` call

### 6.2 Wave 2: sync.rs 核心函数

**任务**:
- Add `push_to_single_gist()` with tree.json + content files
- Add `pull_from_single_gist()` with gist_id direct query
- Remove old `push_gitee()` / `pull_gitee()` (one-gist-per-folder)

**测试**:
- Mock snippets array → verify tree.json structure
- Mock gist response → verify Snippet reconstruction

### 6.3 Wave 3: store.rs load_from_sync 调整

**任务**:
- Update `load_from_sync()` for LWW timestamp comparison
- Simplify: remove `gitee_gist_id` matching (no longer needed)

### 6.4 Wave 4: mod.rs SnippetManager 调整

**任务**:
- Update `do_push()` to call `push_to_single_gist()`
- Update `do_pull()` to call `pull_from_single_gist()`
- Save `gist_id` to config after first sync

### 6.5 Wave 5: config.rs GiteeConfig

**任务**:
- Add `gist_id: Option<String>` field
- Update config.yaml read/write to include gist_id

### 6.6 Wave 6: UI 集成

**任务**:
- Update config dialog to show gist_id (readonly or hidden)
- Update sync status notification

---

## 7. 验收标准

- [ ] 所有 snippets 打包到一个 gist（tree.json + N 个文件）
- [ ] 文件命名 `{id}_{name}.txt`，无冲突
- [ ] Pull 直接通过 gist_id 查询（GET `/gists/{id}`）
- [ ] Push 创建 gist 后保存 ID 到 config
- [ ] 冲突解决采用时间戳优先（LWW）
- [ ] 单元测试覆盖 sync.rs 核心函数
- [ ] 向后兼容（旧 gitee_gist_id 字段保留）

---

## 8. 关键代码位置

| 模块 | 文件 | 关键变更 |
|-----|-----|---------|
| GiteeClient | `gitee/client.rs` | 新增 `get_gist()` |
| sync.rs | `sync.rs` | 重写 push/pull |
| store.rs | `store.rs` | 调整 load_from_sync |
| config.rs | `config.rs` | 新增 gist_id 字段 |
| mod.rs | `mod.rs` | 调整 do_push/do_pull |

---

## 9. 风险与缓解

### 9.1 gist_id 丢失

**风险**: config.yaml gist_id 字段被删除或损坏
**缓解**: Pull 时检测 gist 不存在 → 重新创建，通知用户

### 9.2 tree.json 过大

**风险**: 大量 snippets 时 tree.json 可能超出限制
**缓解**: 监控文件大小，超过 1MB 时警告用户

### 9.3 向后兼容

**风险**: 升级后旧数据 gitee_gist_id 字段存在但语义改变
**缓解**: 保留字段，注释 deprecated，新代码不使用

---

## 10. 下一步

调用 **superpowers:writing-plans** 创建详细实现计划（bite-sized tasks）。