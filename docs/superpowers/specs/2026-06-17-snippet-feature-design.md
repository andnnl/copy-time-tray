# Snippet Feature Design — copy-time-tray

## 概述

为 copy-time-tray 托盘应用添加 Snippet（代码片段/命令片段）管理功能。用户可预存常用命令文本，通过全局快捷键唤起搜索弹窗，快速复制到剪贴板。支持通过 GitHub/Gitee 私有仓库实现云同步。

**核心定位**：本地优先的片段管理 + 可选云同步，而非终端注入工具。

---

## 1. 架构总览

```
┌─────────────────────────────────────────────────────────┐
│                    UI Layer (egui)                        │
│  ┌──────────────────┐    ┌──────────────────────────┐   │
│  │  搜索弹窗         │    │  编辑弹窗 (modal)         │   │
│  │  (全局快捷键唤起)  │    │  (新建/编辑/删除确认)     │   │
│  └────────┬─────────┘    └────────────┬─────────────┘   │
│           │                           │                  │
├───────────┼───────────────────────────┼──────────────────┤
│           ▼                           ▼                  │
│  ┌─────────────────────────────────────────────────┐    │
│  │           Store (本地 JSON)                       │    │
│  │  ~/.../copy-time-tray/snippets.json             │    │
│  └────────────────────┬────────────────────────────┘    │
│                       │                                  │
│           ┌───────────┴───────────┐                      │
│           ▼                       ▼                      │
│  ┌────────────────┐     ┌──────────────────┐            │
│  │ Sync Manager   │     │ Config Manager   │            │
│  │ (reqwest HTTP) │     │ (config.yaml)    │            │
│  └────────┬───────┘     └──────────────────┘            │
└───────────┼─────────────────────────────────────────────┘
            │
            ▼
┌───────────────────────────────────────┐
│  GitHub/Gitee Contents API            │
│  私有仓库, YAML 文件                   │
└───────────────────────────────────────┘
```

---

## 2. 数据模型

### 2.1 本地存储结构 (snippets.json)

```json
{
  "version": 1,
  "snippets": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "parentId": null,
      "name": "常用命令",
      "type": "Folder",
      "content": null,
      "sort": 0,
      "deleted": false,
      "synced": true,
      "createDate": "2026-06-17T10:00:00Z",
      "updateDate": "2026-06-17T10:00:00Z"
    },
    {
      "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
      "parentId": "550e8400-e29b-41d4-a716-446655440000",
      "name": "查看日志",
      "type": "Snippet",
      "content": "tail -f /var/log/syslog | grep error",
      "sort": 0,
      "deleted": false,
      "synced": true,
      "createDate": "2026-06-17T10:01:00Z",
      "updateDate": "2026-06-17T10:01:00Z"
    }
  ]
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| id | UUID v4 | 唯一标识 |
| parentId | Option\<String\> | 父文件夹 ID，null 表示根目录 |
| name | String | 显示名称 |
| type | Enum | Folder / Snippet |
| content | Option\<String\> | 片段内容，仅 Snippet 有值 |
| sort | i32 | 排序权重，越小越靠前 |
| deleted | bool | 软删除标记 |
| synced | bool | 是否已同步到云端 |
| createDate | DateTime | 创建时间 (RFC3339) |
| updateDate | DateTime | 更新时间 (RFC3339) |

### 2.2 远端存储结构 (YAML 文件)

```
snippets/
├── 常用命令/                    # 文件夹 (目录)
│   ├── 查看日志.yaml            # 1 snippet = 1 file
│   └── 重启服务.yaml
└── docker-重启.yaml             # 根目录 snippet
```

**YAML 文件格式**：

```yaml
# snippets/常用命令/查看日志.yaml
name: 查看日志
content: |
  tail -f /var/log/syslog | grep error
sort: 0
created: 2026-06-17T10:01:00Z
updated: 2026-06-17T10:01:00Z
```

**ID 生成规则**：`SHA-256(相对路径)` 的前 16 位十六进制字符串。

---

## 3. 同步协议

### 3.1 认证配置 (config.yaml)

```yaml
# ~/.../copy-time-tray/config.yaml
sync:
  provider: github       # github | gitee
  owner: username
  repo: my-snippets
  branch: main
  token: ghp_xxxxxxxxxxxx
```

### 3.2 Pull 流程

```
1. HTTP GET /repos/{owner}/{repo}/git/trees/{branch}?recursive=1
2. 过滤 snippets/ 目录下的 .yaml 文件
3. 对每个文件 HTTP GET /contents/{path} (Accept: application/vnd.github.raw)
4. 解析 YAML → 生成 Snippet 记录 (ID = SHA-256 前 16 位)
5. 与本地合并 (Last Write Wins，比较 updateDate)
6. 写入 snippets.json
```

### 3.3 Push 流程

```
1. 查询 synced=false 的本地记录
2. 对每条未同步记录:
   - deleted=true  → HTTP DELETE /contents/{path}
   - deleted=false → HTTP PUT /contents/{path} (body=YAML, base64)
3. 成功响应后，置 synced=true
4. 失败记录保留，下次重试
```

### 3.4 触发策略

| 触发时机 | 操作 |
|---------|------|
| 打开搜索弹窗 | Pull |
| 本地修改后 | Debounce 3 秒 → Push |
| 托盘菜单"同步" | Pull + Push |
| 应用启动 | Pull (静默) |

---

## 4. UI 设计

### 4.1 搜索弹窗

**外观**：
- 尺寸：600×400 px
- 位置：屏幕中央
- 样式：无边框圆角窗口，半透明背景

**布局**：
```
┌─────────────────────────────────────────┐
│ 🔍 [搜索框..........................]    │
├─────────────────────────────────────────┤
│ 📁 常用命令                              │
│   📄 查看日志         tail -f /var/...   │
│   📄 重启服务         systemctl rest...  │
│ 📁 Docker                               │
│   📄 docker-重启      docker compos...   │
├─────────────────────────────────────────┤
│ ↑↓ 导航  Enter 展开  Ctrl+Enter 复制     │
└─────────────────────────────────────────┘
```

**交互**：
- 全局快捷键 `Ctrl+Shift+S` 唤起/隐藏
- 搜索：debounce 200ms，模糊匹配 name + content
- 键盘导航：↑↓ 移动选中，Enter 展开/折叠文件夹，Ctrl+Enter 复制内容
- Esc 关闭弹窗

**快捷键**：
| 快捷键 | 功能 |
|--------|------|
| Ctrl+Shift+S | 唤起/隐藏弹窗 |
| Esc | 关闭弹窗 |
| ↑ / ↓ | 移动选中 |
| Enter | 展开/折叠文件夹 |
| Ctrl+Enter | 复制选中 snippet |
| Ctrl+N | 新建 snippet |
| Ctrl+E | 编辑选中 snippet |
| Ctrl+D | 删除选中 (弹窗确认) |

### 4.2 编辑弹窗

**外观**：独立 modal 窗口，400×300 px

**布局**：
```
┌──────────────────────────────────────┐
│ 新建 Snippet                        │
├──────────────────────────────────────┤
│ 名称:  [查看日志___________]          │
│ 文件夹: [常用命令 ▼]                 │
│ 内容:                               │
│ ┌──────────────────────────────────┐ │
│ │ tail -f /var/log/syslog          │ │
│ │ | grep error                     │ │
│ └──────────────────────────────────┘ │
│                                      │
│            [取消]  [保存]             │
└──────────────────────────────────────┘
```

---

## 5. 代码结构

```
src/
├── main.rs              # 入口，托盘初始化
├── platform/            # 平台抽象层 (已有)
│   ├── linux.rs
│   └── windows.rs
└── snippet/             # 新功能模块
    ├── mod.rs           # 模块导出
    ├── model.rs         # Snippet 数据结构
    ├── store.rs         # 本地 JSON 读写
    ├── sync.rs          # HTTP 同步逻辑
    ├── config.rs        # 配置文件管理
    └── ui/
        ├── mod.rs       # UI 模块导出
        ├── search.rs    # 搜索弹窗
        └── editor.rs    # 编辑弹窗
```

---

## 6. 依赖清单

```toml
[dependencies]
# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# HTTP
reqwest = { version = "0.11", features = ["blocking", "json"] }

# 工具
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
base64 = "0.21"

# GUI
egui = "0.27"
eframe = "0.27"
global-hotkey = "0.4"

# 已有
arboard = "3"          # 剪贴板
notify-rust = "4"      # 桌面通知
```

---

## 7. 非目标 (Out of Scope)

- **终端注入**：仅复制到剪贴板，不自动发送到终端
- **加密**：依赖私有仓库自身安全，不在应用层加密
- **Git CLI 集成**：纯 HTTP API，不依赖本地 git
- **冲突解决 UI**：采用 Last Write Wins，不提示冲突
- **富文本/Markdown 预览**：纯文本编辑器

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| API Rate Limit | 频繁同步被封 | Debounce + 缓存 ETag |
| Token 泄露 | 安全风险 | 配置文件权限 600，文档提醒 |
| 网络离线 | 同步失败 | 本地优先，失败队列重试 |
| 大文件传输 | 首拉慢 | 仅传输元数据，内容按需拉取 (可选优化) |

---

## 9. 验收标准

- [ ] 全局快捷键唤起搜索弹窗
- [ ] 搜索框输入 200ms 后实时过滤
- [ ] 键盘导航：↑↓ 移动，Enter 展开，Ctrl+Enter 复制
- [ ] 新建/编辑/删除 snippet，操作即时保存到本地 JSON
- [ ] 配置 GitHub/Gitee token 后，开弹窗自动 pull
- [ ] 本地修改后 3 秒内自动 push
- [ ] 托盘菜单有"同步"手动入口
- [ ] 无配置时离线可用，同步按钮提示"请先配置"

---

## 10. 里程碑

| 阶段 | 内容 | 状态 |
|------|------|------|
| M1 | 数据模型 + 本地存储 | 待实现 |
| M2 | 搜索弹窗 UI + 键盘交互 | 待实现 |
| M3 | 编辑弹窗 + CRUD 操作 | 待实现 |
| M4 | 同步协议 (Pull/Push) | 待实现 |
| M5 | 全局快捷键 + 托盘菜单集成 | 待实现 |

---

*Created: 2026-06-17*
*Author: Sisyphus (AI Agent)*
*Status: Draft — Pending User Review*
