# GitHub 私有 Gist 全树同步设计规格

**日期**：2026-07-11  
**状态**：已确认，待书面规格审查  
**目标**：将 GitHub 同步从按仓库逐文件同步直接替换为私有 Gist 全树同步，使其数据模型、冲突处理与 Gitee 单 Gist 同步保持一致。

## 1. 范围与决策

### 1.1 已确认决策

- GitHub 使用私有 Gist，不再使用 GitHub 仓库 Contents API。
- 每个 GitHub 配置对应一个 Gist，保存完整片段树的 `tree.json`。
- 首次同步按描述复用已有且包含 `tree.json` 的 Gist；没有可复用 Gist 时创建私有 Gist。
- 配置界面中 GitHub 不再收集 Owner、Repo、Branch；只收集 Token 与 Gist 名称。
- 旧 YAML 中的 `owner`、`repo`、`branch` 字段必须能继续反序列化，但运行时不再读取或写入它们。
- 不保留 GitHub 仓库模式，也不添加模式选择。

### 1.2 不在本次范围内

- 不改变 Gitee 的 API、配置字段或其同步行为。
- 不迁移、删除或修改用户现有 GitHub 仓库中的历史 YAML 文件。
- 不实现 macOS 平台支持。
- 不改变已有 LWW 合并规则和 30 天删除墓碑保留期。

## 2. 配置与迁移

`SyncConfig` 继续保存通用 `provider` 与旧仓库字段，但旧字段添加 serde 默认值并在新配置序列化时跳过空值。新增独立的 `github_config`，避免继续借用 Gitee 专属字段。

```rust
pub struct SyncConfig {
    pub provider: SyncProvider,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub github_config: GithubConfig,
    #[serde(default)]
    pub gitee_config: GiteeConfig,
}

pub struct GithubConfig {
    pub api_base: String,                 // 默认 https://api.github.com
    pub token: String,
    pub gist_id: Option<String>,
    pub gist_description: String,         // 默认 CopyTimeTrayConfig
}
```

为兼容旧 GitHub 配置，`GithubConfig::token` 的默认值为空；读取配置后，GitHub 同步读取 `github_config.token`，若它为空则回退至旧的顶层 `token`。保存新的 GitHub 配置时，仅写入 `github_config.token`，并将旧顶层 `token` 置为空。这样旧配置无需手动迁移，首次通过设置窗口保存即可完成规范化。

`gist_id` 与 Gitee 一样在首次成功创建或复用 Gist 后持久化。GitHub 和 Gitee 的 ID 分别写入自己的 provider 配置，绝不互相覆盖。

## 3. GitHub Gist API 适配

GitHub 请求使用：

- 基础地址：`https://api.github.com`
- 认证：`Authorization: Bearer <token>`
- Accept：`application/vnd.github+json`
- User-Agent：`copy-time-tray/0.3`

接口：

| 用途 | 方法与路径 | 成功状态 |
|---|---|---|
| 读取已绑定 Gist | `GET /gists/{gist_id}` | 200 |
| 查找可复用 Gist | `GET /gists?per_page=100&page=N` | 200 |
| 创建私有 Gist | `POST /gists`，请求体带 `public: false` | 201 |
| 写回完整树 | `PATCH /gists/{gist_id}` | 200 |
| 拉取被截断的文件 | `GET <raw_url>` | 200 |

新增 GitHub 专属的轻量客户端和响应模型。Gitee 客户端及其模型不复用，以免将两家 API 的认证、分页和响应差异混在一起。两者通过同步层共享的树序列化、反序列化和墓碑过滤函数获得一致行为。

GitHub Gist API 在文件内容过大时会将文件标为 `truncated: true`，并返回 `raw_url`。同步层读取 `tree.json` 时：

1. 内容未截断且存在 `content`：直接解析；
2. 标为截断：使用认证头请求 `raw_url`，再解析完整内容；
3. 无内容或缺少 `raw_url`：返回明确的解析错误，不覆盖本地数据。

## 4. 同步数据与流程

### 4.1 数据格式

GitHub 与 Gitee 都以现有 `TreeEntry` 数组序列化为单个 `tree.json`。条目包含：

- 稳定 ID、父级 ID、名称、类型与排序；
- 删除状态；
- 创建及更新时间；
- 片段文本内容（文件夹没有内容）。

该格式已由 Gitee 使用，GitHub 不引入另一种格式。所有未删除条目，以及更新日期距当前不超过 30 天的删除墓碑，都会被写入完整树。

### 4.2 Pull（GitHub → 本地）

1. 如果未配置 GitHub `gist_id`，返回空列表，不创建远程数据。
2. 获取指定 Gist，读取完整 `tree.json`（处理截断回退）。
3. 反序列化为远程片段列表；远程条目设置 `synced = true`。
4. 调用既有 `SnippetStore::load_from_sync()` 合并：相同 ID 比较 `update_date`，远程较新才覆盖；相同时间保留本地；仅远程存在的条目追加到本地。
5. 删除墓碑与正常条目同样进入 LWW 合并，确保跨设备删除可传播。

### 4.3 Push（本地 → GitHub）

1. 从 Store 取得完整片段树，而不是仅取得待同步条目。
2. 过滤超过 30 天的删除墓碑，并构建 `tree.json`。
3. 若已有 `gist_id`，先读取确认其存在；找不到时按“复用或创建”流程获取新 ID。
4. 若没有 ID，分页列出当前账户的 Gist，以描述相同且包含 `tree.json` 为条件，选取最近更新的一条；没有匹配项则 `POST` 创建 `public: false` 的 Gist。
5. `PATCH` 该 Gist，使 `tree.json` 替换为完整本地树；提交成功才标记本地的已推送条目为 `synced = true`。
6. 将新建或复用的 ID 写入 `github_config.gist_id` 和配置文件。

调用入口维持现有行为：手动同步、启动后后台拉取、编辑后的防抖同步均先拉取再合并、再推送完整树，从而保留跨设备的 LWW 语义。

## 5. UI 与连接验证

GitHub 设置区域调整为：

- GitHub Personal Access Token；
- Gist 名称（默认 `CopyTimeTrayConfig`）；
- 测试连接按钮。

移除 Owner、Repo、Branch 的控件、校验和状态读取。帮助文案明确要求经典 PAT 的 `gist` scope，或细粒度 PAT 的账户级 `Gists: write` 权限。

“测试连接”对 GitHub 调用列举 Gist 的接口，能够鉴权且返回成功即提示连接成功；不创建、更新或删除任何 Gist。Gitee 的现有测试连接逻辑保持不变。

同步成功、失败提示统一使用中性“远程”措辞，不再将同步结果写死为 Gitee。

## 6. 错误处理

| 场景 | 行为 |
|---|---|
| Token 缺失 | 阻止保存/同步，提示填写 Token |
| 401 或 403 | 不重试，提示检查 Gist 权限或 Token |
| 404 的已绑定 Gist | push 时重新查找或创建并保存新 ID；pull 报错且不修改本地数据 |
| 网络错误或 5xx | 保留现有 3 次指数退避重试 |
| `tree.json` 缺失、为空或 JSON 非法 | 返回解析错误，不合并、不标记同步 |
| 截断文件的 raw 下载失败 | 返回 HTTP 错误，不合并、不标记同步 |
| 创建、复用或更新失败 | 不写入 gist_id；本地条目保持未同步 |

## 7. 测试与验收

新增或调整单元测试覆盖：

1. 旧 GitHub YAML（顶层 token 和 owner/repo/branch）可解析；新 GitHub 配置序列化时不输出空旧字段。
2. GitHub 请求使用 Gist 基础地址、Bearer 认证、Accept 与 User-Agent。
3. GitHub pull 从 `tree.json` 还原文件夹、片段和删除墓碑。
4. 截断的 GitHub `tree.json` 会请求 `raw_url` 并使用响应内容。
5. GitHub push 写入完整树并保留近期删除墓碑，不再产生仓库树或 Contents API 请求。
6. 缺少 ID 时按描述和 `tree.json` 复用最近更新的 Gist；无匹配项创建 `public: false` Gist。
7. 首次成功同步保存 GitHub `gist_id`；不得改写 Gitee `gist_id`。
8. GitHub 与 Gitee 都从 Store 取得完整树进行推送。
9. 配置 UI 不再引用 GitHub Owner/Repo/Branch，GitHub token 与 Gist 名称校验生效。
10. `cargo fmt --check`、`cargo test`、`cargo build` 及 `git diff --check` 均通过。

## 8. 实现边界

改动预期集中在：

- `src/snippet/config.rs`：GitHub Gist 配置、兼容反序列化与测试；
- `src/snippet/github.rs`、`src/snippet/github/*`：GitHub Gist HTTP 客户端与模型；
- `src/snippet/sync.rs`：移除 GitHub 仓库同步、接入 GitHub Gist 同步及共享树逻辑；
- `src/snippet/mod.rs`：两端完整树推送和 provider 专属 ID 持久化；
- `src/snippet/ui/*`：GitHub 配置表单、连接测试、即时同步配置；
- `src/platform/linux.rs`：中性同步提示；
- 相关单元测试。

不重构与同步无关的片段编辑、导入导出或托盘功能。
