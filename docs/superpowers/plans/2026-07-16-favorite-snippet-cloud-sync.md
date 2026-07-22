# 收藏片段标签栏与云同步实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让普通代码片段的收藏状态在本地、Gitee tree.json 和当前 GitHub 仓库 YAML 同步协议中保持一致，并在 Windows 与 Linux 搜索框下提供沿用现有按钮风格的可换行收藏标签栏和右键收藏入口。

**架构：** 在 `Snippet` 中增加可向后兼容的收藏及 GitHub 远端路径元数据；收藏写入统一经过 Store，以复用时间戳、待同步标记和持久化。Gitee 将收藏作为 `TreeEntry` 字段随整棵树同步；当前 GitHub 仓库 YAML 写入稳定 ID 和收藏状态，并通过本地记录的旧路径完成“先写新文件、后删旧文件”的重命名迁移。平台 UI 只产生收藏目标状态或直接调用共享的应用层 helper，成功后刷新并调度已有的防抖同步。

**技术栈：** Rust edition 2021、serde / serde_json / serde_yaml、chrono、reqwest blocking、GTK 3（Linux）、egui 0.29（Windows）、tempfile。

**协议边界：** 本计划以已确认的 `docs/superpowers/specs/2026-07-16-favorite-sync-design.md` 和当前源码为准：Gitee 继续使用单 Gist `tree.json`，GitHub 继续使用仓库 Contents API 中的 `snippets/*.yaml`。不把较早的 GitHub 私有 Gist 迁移规格混入本次变更。

---

## 文件结构

### 修改

- `src/snippet/model.rs`：为本地片段定义 `favorite` 和 `github_remote_path`，并验证历史 JSON 兼容性。
- `src/snippet/store.rs`：提供受类型约束的收藏切换、按稳定 ID 去重的远端 LWW 合并，以及不触发更新时间的 GitHub 同步确认。
- `src/snippet/import_export.rs`：使 Termora 导入显式产生未收藏、无 GitHub 路径的片段。
- `src/snippet/sync.rs`：扩展 Gitee tree 与 GitHub YAML 协议，记录 GitHub 路径，按稳定 ID 折叠拉取结果，并实现可验证的 GitHub 重命名 PUT→DELETE 顺序。
- `src/snippet/mod.rs`：以结构化 GitHub 推送成功结果更新已同步状态和远端路径；让两条推送入口都安全确认成功项。
- `src/snippet/ui/mod.rs`：声明携带 ID 与目标状态的收藏 UI 响应。
- `src/app.rs`：实现可供 Windows 响应处理与 Linux GTK 回调共用的收藏应用服务，成功时调度同步并刷新 Windows 快照。
- `src/snippet/ui/search.rs`：添加无 GUI 依赖的收藏过滤/菜单判定，并渲染 Windows 收藏标签栏和搜索结果右键菜单。
- `src/snippet/ui/linux_manager/mod.rs`：添加 GTK 收藏栏、TreeView 与标签右键菜单，并使 Linux 所有 Store 写入路径调度自动同步。

### 不创建

- 不创建新的同步协议或 GitHub Gist 客户端。
- 不向 Termora 的导入/导出结构添加 `favorite` 或 `github_remote_path`。
- 不为标签增加关闭按钮、拖拽排序、独立 CSS 或独立配色。

---

### 任务 1：建立向后兼容的模型与收藏 Store API

**文件：**
- 修改：`src/snippet/model.rs:12-62,74-189`
- 修改：`src/snippet/store.rs:13-30,153-212,215-587`
- 修改：`src/snippet/import_export.rs:107-119,176-380`

- [ ] **步骤 1：为模型默认值和历史 JSON 编写失败测试**

在 `src/snippet/model.rs` 的测试模块增加以下测试。JSON 特意不携带新字段，证明 serde 默认值而非测试辅助函数在负责兼容。

```rust
#[test]
fn new_items_start_unfavorited_without_github_path() {
    let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
    let folder = Snippet::new_folder("folder".into(), None);

    assert!(!snippet.favorite);
    assert_eq!(snippet.github_remote_path, None);
    assert!(!folder.favorite);
    assert_eq!(folder.github_remote_path, None);
}

#[test]
fn legacy_json_defaults_favorite_and_github_path() {
    let json = r#"{
        "id":"legacy","parent_id":null,"name":"legacy","type":"snippet",
        "content":"echo legacy","sort":0,"deleted":false,"synced":true,
        "create_date":"2026-07-16T00:00:00Z","update_date":"2026-07-16T00:00:00Z"
    }"#;

    let snippet: Snippet = serde_json::from_str(json).unwrap();

    assert!(!snippet.favorite);
    assert_eq!(snippet.github_remote_path, None);
}

#[test]
fn model_round_trip_keeps_favorite_and_github_path() {
    let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
    snippet.favorite = true;
    snippet.github_remote_path = Some("snippets/test.yaml".into());

    let restored: Snippet = serde_json::from_str(&serde_json::to_string(&snippet).unwrap()).unwrap();

    assert!(restored.favorite);
    assert_eq!(restored.github_remote_path.as_deref(), Some("snippets/test.yaml"));
}
```

- [ ] **步骤 2：运行模型测试，确认因字段和构造器尚不存在而失败**

运行：`cargo test snippet::model`

预期：编译失败，包含 `no field 'favorite' on type 'Snippet'` 和 `no field 'github_remote_path' on type 'Snippet'`；不得因测试语法错误失败。

- [ ] **步骤 3：为收藏 Store 行为和第三方导入默认值编写失败测试**

在 `src/snippet/store.rs` 测试模块添加：

```rust
#[test]
fn set_favorite_persists_marks_pending_and_updates_timestamp() {
    let (mut store, dir) = temp_store();
    let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
    snippet.synced = true;
    snippet.update_date = chrono::Utc::now() - chrono::Duration::minutes(1);
    let before = snippet.update_date;
    let id = snippet.id.clone();
    store.add(snippet).unwrap();

    store.set_favorite(&id, true).unwrap();

    let updated = store.get(&id).unwrap();
    assert!(updated.favorite);
    assert!(!updated.synced);
    assert!(updated.update_date > before);
    drop(store);

    let reloaded = SnippetStore::new(dir.path().join("snippets.json")).unwrap();
    assert!(reloaded.get(&id).unwrap().favorite);
}

#[test]
fn set_favorite_rejects_folder_and_deleted_snippet() {
    let (mut store, _dir) = temp_store();
    let folder = Snippet::new_folder("folder".into(), None);
    let folder_id = folder.id.clone();
    let mut deleted = Snippet::new_snippet("deleted".into(), "content".into(), None);
    deleted.deleted = true;
    let deleted_id = deleted.id.clone();
    store.add(folder).unwrap();
    store.add(deleted).unwrap();

    assert!(matches!(store.set_favorite(&folder_id, true), Err(StoreError::NotFavoriteable(_))));
    assert!(matches!(store.set_favorite(&deleted_id, true), Err(StoreError::NotFavoriteable(_))));
}
```

在 `src/snippet/import_export.rs` 测试模块添加：

```rust
#[test]
fn termora_import_defaults_to_unfavorited_without_github_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("termora.json");
    fs::write(&path, sample_termora_json()).unwrap();

    for snippet in import_from_termora(&path).unwrap() {
        assert!(!snippet.favorite);
        assert_eq!(snippet.github_remote_path, None);
    }
}
```

- [ ] **步骤 4：运行定向测试，确认缺少 API 和错误变体**

运行：`cargo test snippet::store snippet::import_export`

预期：编译失败，包含 `no method named 'set_favorite'` 和 `no variant or associated item named 'NotFavoriteable'`。

- [ ] **步骤 5：实现模型字段、Store API 与 Termora 初始化的最小代码**

在 `src/snippet/model.rs` 的 `Snippet` 中，在 `gitee_gist_id` 前添加：

```rust
#[serde(default)]
pub favorite: bool,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub github_remote_path: Option<String>,
```

在 `new_snippet` 与 `new_folder` 的结构体字面量中均添加：

```rust
favorite: false,
github_remote_path: None,
```

在 `src/snippet/store.rs` 中扩展 `StoreError` 和 `Display`：

```rust
NotFavoriteable(String),
```

```rust
StoreError::NotFavoriteable(id) => write!(f, "Snippet cannot be favorited: {}", id),
```

在 `delete` 前增加受保护的 API；它必须复用 `update`，以保留更新时间、`synced = false` 与保存逻辑：

```rust
pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<(), StoreError> {
    match self.get(id) {
        None => Err(StoreError::NotFound(id.to_string())),
        Some(snippet) if snippet.deleted || snippet.snippet_type != SnippetType::Snippet => {
            Err(StoreError::NotFavoriteable(id.to_string()))
        }
        Some(_) => self.update(id, |snippet| snippet.favorite = favorite),
    }
}
```

在 `src/snippet/import_export.rs` 的 Termora `Snippet { ... }` 字面量中添加：

```rust
favorite: false,
github_remote_path: None,
```

- [ ] **步骤 6：运行模型、Store 与导入测试，确认绿灯**

运行：`cargo test snippet::model && cargo test snippet::store && cargo test snippet::import_export`

预期：三个命令均通过；新测试确认历史本地 JSON 与 Termora 导入默认未收藏。

- [ ] **步骤 7：提交模型与 Store 基础变更**

```bash
git add src/snippet/model.rs src/snippet/store.rs src/snippet/import_export.rs
git commit -m "feat: persist snippet favorites"
```

---

### 任务 2：让 Gitee tree 与 GitHub YAML 读取和写入收藏及稳定 ID

**文件：**
- 修改：`src/snippet/sync.rs:45-106,252-335,544-565,736-894`
- 修改：`src/snippet/store.rs:190-212,215-587`

- [ ] **步骤 1：为 Gitee tree、GitHub 新旧 YAML 和 LWW 收藏合并编写失败测试**

在 `src/snippet/sync.rs` 测试模块增加：

```rust
#[test]
fn tree_entry_round_trip_keeps_favorite_and_legacy_tree_defaults_false() {
    let mut snippet = Snippet::new_snippet("ping".into(), "ping localhost".into(), None);
    snippet.favorite = true;
    let entries = build_tree_entries(&[&snippet]);
    let json = serde_json::to_string(&entries).unwrap();
    assert!(json.contains("\"favorite\":true"));
    assert!(tree_entries_to_snippets(entries)[0].favorite);

    let legacy: Vec<TreeEntry> = serde_json::from_str(
        r#"[{"id":"old","parent_id":null,"name":"old","type":"snippet","sort":0,"deleted":false,"create_date":"2026-07-16T00:00:00Z","update_date":"2026-07-16T00:00:00Z","content":"echo old"}]"#,
    ).unwrap();
    assert!(!legacy[0].favorite);
}

#[test]
fn github_yaml_writes_and_reads_stable_id_favorite_and_remote_path() {
    let mut snippet = Snippet::new_snippet("new name".into(), "echo new".into(), None);
    snippet.id = "stable-id".into();
    snippet.favorite = true;

    let yaml = snippet_to_yaml(&snippet).unwrap();
    assert!(yaml.contains("id: stable-id"));
    assert!(yaml.contains("favorite: true"));

    let parsed = parse_yaml_snippet("snippets/new-name.yaml", &yaml).unwrap();
    assert_eq!(parsed.id, "stable-id");
    assert!(parsed.favorite);
    assert_eq!(parsed.github_remote_path.as_deref(), Some("snippets/new-name.yaml"));
}

#[test]
fn legacy_github_yaml_keeps_path_derived_id_and_defaults_favorite() {
    let yaml = "name: old\ncontent: |\n  echo old\nsort: 0\ncreated: \"2026-07-16T00:00:00Z\"\nupdated: \"2026-07-16T00:00:00Z\"\n";
    let parsed = parse_yaml_snippet("snippets/old.yaml", yaml).unwrap();

    assert_eq!(parsed.id, generate_file_id("snippets/old.yaml"));
    assert!(!parsed.favorite);
    assert_eq!(parsed.github_remote_path.as_deref(), Some("snippets/old.yaml"));
}
```

在 `src/snippet/store.rs` 测试模块增加：

```rust
#[test]
fn newer_remote_favorite_wins_under_lww() {
    let (mut store, _dir) = temp_store();
    let base = chrono::Utc::now() - chrono::Duration::hours(1);
    let mut local = Snippet::new_snippet("same".into(), "content".into(), None);
    local.id = "same-id".into();
    local.favorite = false;
    local.update_date = base;
    store.add(local).unwrap();

    let mut remote = Snippet::new_snippet("same".into(), "content".into(), None);
    remote.id = "same-id".into();
    remote.favorite = true;
    remote.synced = true;
    remote.update_date = base + chrono::Duration::minutes(1);
    store.load_from_sync(vec![remote]).unwrap();

    assert!(store.get("same-id").unwrap().favorite);
}

#[test]
fn load_from_sync_collapses_duplicate_remote_stable_ids_before_append() {
    let (mut store, _dir) = temp_store();
    let base = chrono::Utc::now() - chrono::Duration::hours(1);
    let mut older = Snippet::new_snippet("old path".into(), "old".into(), None);
    older.id = "stable-id".into();
    older.update_date = base;
    let mut newer = Snippet::new_snippet("new path".into(), "new".into(), None);
    newer.id = "stable-id".into();
    newer.update_date = base + chrono::Duration::minutes(1);

    store.load_from_sync(vec![older, newer]).unwrap();

    assert_eq!(store.list().iter().filter(|item| item.id == "stable-id").count(), 1);
    assert_eq!(store.get("stable-id").unwrap().name, "new path");
}
```

- [ ] **步骤 2：运行定向协议与 Store 测试，确认正确失败**

运行：`cargo test snippet::sync && cargo test snippet::store`

预期：编译失败，指出 `TreeEntry`、`Snippet` 或 YAML DTO 尚无 `favorite`、稳定 `id` 和路径字段；重复 ID 测试在实现前也可以暴露当前 `load_from_sync` 的重复追加问题。

- [ ] **步骤 3：实现 Gitee tree 与 GitHub YAML 的兼容 DTO**

在 `TreeEntry` 中新增：

```rust
#[serde(default)]
pub favorite: bool,
```

在 `build_tree_entries` 复制业务字段：

```rust
favorite: s.favorite,
```

在 `tree_entries_to_snippets` 的 `Snippet` 字面量中添加：

```rust
favorite: entry.favorite,
github_remote_path: None,
```

将 `parse_yaml_snippet` 内部 DTO 改为：

```rust
#[derive(Deserialize)]
struct RemoteSnippet {
    #[serde(default)]
    id: Option<String>,
    name: String,
    content: String,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    sort: i32,
    created: String,
    updated: String,
}
```

并用以下规则创建 ID 和片段字段：

```rust
let id = remote.id.unwrap_or_else(|| generate_file_id(relative_path));
```

```rust
favorite: remote.favorite,
github_remote_path: Some(relative_path.to_string()),
```

在 `snippet_to_yaml` 的 `name:` 之前输出稳定身份和收藏状态：

```rust
r#"id: {}
favorite: {}
name: {}
content: |
{}
sort: {}
created: "{}"
updated: "{}"
"#
```

格式化参数顺序必须为：`snippet.id`、`snippet.favorite`、`snippet.name`、缩进内容、`snippet.sort`、创建时间、更新时间。

同时补齐本文件所有 `TreeEntry { ... }` 测试字面量的 `favorite` 字段，以让编译器强制每一处明确业务默认值。

- [ ] **步骤 4：在 Store 中按 ID 归一化远端批次，再做现有 LWW 合并**

将 `load_from_sync` 的开头替换为先折叠远端批次的逻辑；同一 `id` 仅保留更新时间更大的记录，同一时间保留先出现的记录，从而与“相同时间不覆盖”的 LWW 语义一致：

```rust
let mut newest_remote_by_id: HashMap<String, Snippet> = HashMap::new();
for remote in remote_snippets {
    match newest_remote_by_id.get(&remote.id) {
        Some(current) if current.update_date >= remote.update_date => {}
        _ => {
            newest_remote_by_id.insert(remote.id.clone(), remote);
        }
    }
}
```

随后建立 `by_id`，并遍历 `newest_remote_by_id.into_values()` 执行现有的 `remote.update_date > existing.update_date` 覆盖规则。不要把 `github_remote_path` 特殊合并；胜出的完整远端记录应完整覆盖。

- [ ] **步骤 5：运行协议和 Store 测试，确认绿灯**

运行：`cargo test snippet::sync && cargo test snippet::store`

预期：通过；历史 `tree.json` 和 YAML 缺字段均默认为未收藏，远端较新收藏能覆盖本地，重复稳定 ID 只留下较新记录。

- [ ] **步骤 6：提交序列化与合并变更**

```bash
git add src/snippet/sync.rs src/snippet/store.rs
git commit -m "feat: sync snippet favorites"
```

---

### 任务 3：实现 GitHub 路径确认、删除请求与重命名迁移

**文件：**
- 修改：`src/snippet/sync.rs:108-187,252-358,456-565,567-895`
- 修改：`src/snippet/store.rs:177-188,215-587`
- 修改：`src/snippet/mod.rs:172-235,306-379,442-542`

- [ ] **步骤 1：定义推送结果和 Store 路径确认的失败测试**

先在 `src/snippet/store.rs` 增加测试，要求成功确认 GitHub 推送时不改变内容更新时间：

```rust
#[test]
fn mark_github_synced_records_path_without_bumping_update_date() {
    let (mut store, _dir) = temp_store();
    let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
    snippet.id = "s1".into();
    snippet.synced = false;
    snippet.update_date = chrono::Utc::now() - chrono::Duration::minutes(5);
    let updated_at = snippet.update_date;
    store.add(snippet).unwrap();

    store.mark_github_synced(&[crate::snippet::sync::GithubPushSuccess {
        id: "s1".to_string(),
        path: "snippets/test.yaml".to_string(),
        expected_update_date: updated_at,
    }]).unwrap();

    let saved = store.get("s1").unwrap();
    assert!(saved.synced);
    assert_eq!(saved.github_remote_path.as_deref(), Some("snippets/test.yaml"));
    assert_eq!(saved.update_date, updated_at);
}

#[test]
fn stale_github_sync_confirmation_keeps_newer_local_change_pending() {
    let (mut store, _dir) = temp_store();
    let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
    snippet.id = "s1".into();
    snippet.synced = false;
    let pushed_at = snippet.update_date;
    store.add(snippet).unwrap();
    store.update("s1", |item| item.favorite = true).unwrap();

    store.mark_github_synced(&[crate::snippet::sync::GithubPushSuccess {
        id: "s1".to_string(),
        path: "snippets/old.yaml".to_string(),
        expected_update_date: pushed_at,
    }]).unwrap();

    let saved = store.get("s1").unwrap();
    assert!(!saved.synced);
    assert_eq!(saved.github_remote_path, None);
}
```

在 `src/snippet/sync.rs` 测试模块将 mock 请求记录扩展为方法、URL、请求体，并添加一个按“方法 + URL”返回响应的 scripted mock。以该 mock 编写：

```rust
#[test]
fn renamed_github_snippet_puts_new_path_then_deletes_old_path() {
    let mut snippet = Snippet::new_snippet("new".into(), "echo new".into(), None);
    snippet.id = "stable-id".into();
    snippet.github_remote_path = Some("snippets/old.yaml".into());
    let client = ScriptedHttpClient::with_responses([
        ("GET", "contents/snippets/new.yaml?ref=main", 404, ""),
        ("PUT", "contents/snippets/new.yaml", 201, "{}"),
        ("GET", "contents/snippets/old.yaml?ref=main", 200, r#"{"sha":"old-sha"}"#),
        ("DELETE", "contents/snippets/old.yaml", 200, "{}"),
    ]);

    let result = push_github(&test_config(), &client, &[snippet]).unwrap();

    assert_eq!(result.succeeded.len(), 1);
    assert_eq!(result.succeeded[0].id, "stable-id");
    assert_eq!(result.succeeded[0].path, "snippets/new.yaml");
    assert_eq!(client.request_methods_and_urls(), vec![
        ("GET".into(), "https://api.github.com/repos/testuser/test-repo/contents/snippets/new.yaml?ref=main".into()),
        ("PUT".into(), "https://api.github.com/repos/testuser/test-repo/contents/snippets/new.yaml".into()),
        ("GET".into(), "https://api.github.com/repos/testuser/test-repo/contents/snippets/old.yaml?ref=main".into()),
        ("DELETE".into(), "https://api.github.com/repos/testuser/test-repo/contents/snippets/old.yaml".into()),
    ]);
    assert!(client.request_body("DELETE", "contents/snippets/old.yaml").contains(r#""sha":"old-sha""#));
    assert!(client.request_body("DELETE", "contents/snippets/old.yaml").contains(r#""branch":"main""#));
}

#[test]
fn github_put_uses_current_target_sha_only_when_target_exists() {
    let snippet = Snippet::new_snippet("same".into(), "echo same".into(), None);
    let existing = ScriptedHttpClient::with_responses([
        ("GET", "contents/snippets/same.yaml?ref=main", 200, r#"{"sha":"target-sha"}"#),
        ("PUT", "contents/snippets/same.yaml", 200, "{}"),
    ]);
    push_github(&test_config(), &existing, &[snippet.clone()]).unwrap();
    assert!(existing.request_body("PUT", "contents/snippets/same.yaml").contains(r#""sha":"target-sha""#));

    let missing = ScriptedHttpClient::with_responses([
        ("GET", "contents/snippets/same.yaml?ref=main", 404, ""),
        ("PUT", "contents/snippets/same.yaml", 201, "{}"),
    ]);
    push_github(&test_config(), &missing, &[snippet]).unwrap();
    assert!(!missing.request_body("PUT", "contents/snippets/same.yaml").contains("\"sha\""));
}

#[test]
fn failed_rename_delete_returns_error_without_success_path() {
    let mut snippet = Snippet::new_snippet("new".into(), "echo new".into(), None);
    snippet.id = "stable-id".into();
    snippet.github_remote_path = Some("snippets/old.yaml".into());
    let client = ScriptedHttpClient::with_responses([
        ("GET", "contents/snippets/new.yaml?ref=main", 404, ""),
        ("PUT", "contents/snippets/new.yaml", 201, "{}"),
        ("GET", "contents/snippets/old.yaml?ref=main", 200, r#"{"sha":"old-sha"}"#),
        ("DELETE", "contents/snippets/old.yaml", 500, "{}"),
    ]);

    assert!(push_github(&test_config(), &client, &[snippet]).is_err());
    assert_eq!(client.request_methods_and_urls().last().unwrap().0, "DELETE");
}
```

`ScriptedHttpClient` 必须保留 DELETE 与 PUT 请求体；每个 metadata GET 由 `{"sha":"..."}` 响应驱动，不能从 tree API 伪造 SHA。

- [ ] **步骤 2：运行定向测试，确认推送结果、删除体和路径确认 API 缺失**

运行：`cargo test snippet::sync && cargo test snippet::store`

预期：编译失败，包含缺少 `mark_github_synced`、`PushResult`/`succeeded` 或 scripted mock API；在尚未重构 `push_github` 前，不得将测试改成适应旧的 `Vec<String>` 返回值。

- [ ] **步骤 3：扩展 HTTP DELETE 契约并保留远端 blob SHA**

把 `HttpClient::delete` 改为接受 body：

```rust
fn delete(
    &self,
    url: &str,
    headers: &HashMap<String, String>,
    body: &str,
) -> Result<HttpResponse, SyncError>;
```

`RealHttpClient::delete` 必须在 `send()` 前加入：

```rust
let resp = req
    .body(body.to_string())
    .send()
    .map_err(|e| SyncError::Http(e.to_string()))?;
```

新增 `github_content_metadata(config, client, path)`：使用普通 `Accept: application/vnd.github+json` 请求 `GET {base}/contents/{path}?ref={branch}`，仅 `200` 时 JSON 解析 `{ sha }`，`404` 返回 `Ok(None)`，其他状态返回 `SyncError::Http`。不要复用 pull 所用 raw Accept header，因为 raw 内容没有 `sha`。

每次 PUT **和** DELETE 都先调用这个 helper：目标路径存在时，把其 SHA 放入 PUT JSON；目标路径不存在时，PUT JSON 不得包含 `sha`；删除路径不存在时视为删除成功。重命名场景必须分别获取 `target_path` 的 PUT SHA 和旧路径的 DELETE SHA，绝不可混用。

删除 body 使用：

```rust
let body = serde_json::json!({
    "message": format!("Delete snippet: {}", snippet.name),
    "sha": sha,
    "branch": config.branch,
});
let body = serde_json::to_string(&body).map_err(|error| SyncError::Store(error.to_string()))?;
```

若 metadata 返回 `None`，把文件已不存在视为删除成功；若获取 SHA 出错，记录该 snippet 失败且不确认同步。

同步更新所有测试 mock 的 `delete` 签名和请求记录。

- [ ] **步骤 4：用结构化成功结果实现 PUT→DELETE 的重命名事务边界**

在 `src/snippet/sync.rs` 定义：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubPushSuccess {
    pub id: String,
    pub path: String,
    pub expected_update_date: DateTime<Utc>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PushResult {
    pub succeeded: Vec<GithubPushSuccess>,
    pub gitee_gist_id: Option<String>,
}
```

把 `push` 返回值改为 `Result<PushResult, SyncError>`。Gitee 分支返回空 `succeeded` 与 `Some(gist_id)`；GitHub 分支返回 `push_github` 的结果。

令 `push_github` 返回 `Result<PushResult, SyncError>`。对每个普通片段：

1. 计算 `target_path = snippet_to_path(snippet)?` 和 YAML；先调用 metadata helper 查询 `target_path`，若返回 SHA 则将它写入 PUT JSON，若为 `None` 则不写 `sha`；
2. 若 `snippet.deleted`，选择 `snippet.github_remote_path.as_deref().unwrap_or(&target_path)` 作为删除路径，获取 SHA 后删除；成功时将 `GithubPushSuccess { id, path: delete_path, expected_update_date: snippet.update_date }` 放入 `succeeded`（已删除条目之后不会使用该路径）；
3. 若未删除，向 `target_path` PUT；PUT 非 200/201 时失败；
4. 若 `github_remote_path` 存在且与 `target_path` 不同，则仅在 PUT 成功后获取旧路径 SHA 并 DELETE 旧路径；DELETE 或取 SHA 失败时把该片段列为失败，绝不把它加入 `succeeded`；
5. 仅所有必要操作成功后追加 `GithubPushSuccess { id: snippet.id.clone(), path: target_path, expected_update_date: snippet.update_date }`。

保留“任一项失败则返回 `SyncError::Store("Failed to push N snippets")`”的现有整体重试策略。由于本轮整体错误不会确认任何成功项目，重试可能重复已成功 PUT；同一稳定 ID 的覆盖式 PUT 是安全的，且能再次尝试清理旧路径。

```rust
pub fn mark_github_synced(&mut self, successes: &[GithubPushSuccess]) -> Result<(), StoreError> {
    for success in successes {
        if let Some(snippet) = self.get_mut(&success.id) {
            if snippet.update_date == success.expected_update_date {
                snippet.synced = true;
                snippet.github_remote_path = Some(success.path.clone());
            }
        }
    }
    self.save()
}
```

同时增加失败测试：在确认前先通过 `update` 把同一片段的 `update_date` 改新，再调用 `mark_github_synced`；断言它仍是 `synced == false`，且 `github_remote_path` 不被旧推送结果覆盖。这样 UI 在后台推送期间做出的新收藏、取消收藏或编辑不会被旧确认错误标记为已同步。

在 `src/snippet/mod.rs` 的 `do_push` 与 `schedule_push` 成功分支中使用 `PushResult`：

```rust
if let Some(id) = result.gitee_gist_id { /* 保留现有写配置逻辑 */ }
if sync_config.provider == config::SyncProvider::Github {
    store.mark_github_synced(&result.succeeded)?;
} else {
    store.mark_synced(&pending_ids)?;
}
```

在 `schedule_push` 同样必须确认成功项；当前自动路径没有 `mark_synced`，此处一并修复，避免收藏每 5 秒无限重复上传。

- [ ] **步骤 5：在 GitHub 拉取阶段按稳定 ID 折叠重复远端文件**

在 `pull_github` 收集完 YAML 后、返回前按 `id` 折叠：

```rust
let mut newest_by_id: HashMap<String, Snippet> = HashMap::new();
for snippet in snippets {
    match newest_by_id.get(&snippet.id) {
        Some(existing) if existing.update_date >= snippet.update_date => {}
        _ => {
            newest_by_id.insert(snippet.id.clone(), snippet);
        }
    }
}
Ok(newest_by_id.into_values().collect())
```

这与任务 2 的 Store 归一化形成双保险：拉取 API 的消费者得到无重复列表，直接调用 Store 的其他调用方仍被保护。

- [ ] **步骤 6：运行同步与管理器测试，确认绿灯**

运行：`cargo test snippet::sync && cargo test snippet::store && cargo test snippet`

预期：通过；测试记录证明重命名顺序为新路径 PUT 后旧路径 DELETE，DELETE 包含 SHA 和 branch，任何失败不会让 Store 确认新路径或清除待同步状态。

- [ ] **步骤 7：提交 GitHub 重命名同步变更**

```bash
git add src/snippet/sync.rs src/snippet/store.rs src/snippet/mod.rs
git commit -m "fix: preserve snippet identity across github renames"
```

---

### 任务 4：增加统一的收藏应用事件、刷新和同步调度

**文件：**
- 修改：`src/snippet/ui/mod.rs:24-33`
- 修改：`src/app.rs:215-274`

- [ ] **步骤 1：为 UI 事件目标状态和应用 helper 编写失败测试**

在 `src/app.rs` 的测试模块（若不存在则新增 `#[cfg(test)] mod tests`）先提取并测试不依赖 `MANAGER` 的判定函数：

```rust
#[test]
fn favorite_change_refreshes_and_schedules_only_after_store_success() {
    assert!(favorite_change_effect(Ok(())).refresh);
    assert!(favorite_change_effect(Ok(())).schedule_push);
    assert!(!favorite_change_effect(Err("not found".to_string())).refresh);
    assert!(!favorite_change_effect(Err("not found".to_string())).schedule_push);
}
```

其中测试先期望以下内部值类型：

```rust
#[derive(Debug, PartialEq, Eq)]
struct FavoriteChangeEffect {
    refresh: bool,
    schedule_push: bool,
}
```

这只测试应用层的成功/失败边界；真实持久化已在任务 1 的 Store 测试覆盖。

- [ ] **步骤 2：运行 app 测试，确认缺少类型和函数**

运行：`cargo test app`

预期：编译失败，包含 `cannot find function 'favorite_change_effect'`。

- [ ] **步骤 3：声明响应并实现共享收藏操作**

在 `UiResponse` 中、`SnippetDeleted` 后增加：

```rust
ToggleSnippetFavorite { id: String, favorite: bool },
```

在 `app.rs` 添加纯判定与共享 helper：

```rust
#[derive(Debug, PartialEq, Eq)]
struct FavoriteChangeEffect {
    refresh: bool,
    schedule_push: bool,
}

fn favorite_change_effect(result: Result<(), String>) -> FavoriteChangeEffect {
    FavoriteChangeEffect {
        refresh: result.is_ok(),
        schedule_push: result.is_ok(),
    }
}

pub(crate) fn set_snippet_favorite(id: &str, favorite: bool) -> Result<(), String> {
    let manager_guard = MANAGER
        .get()
        .and_then(|manager| manager.lock().ok())
        .ok_or_else(|| "Snippet 管理器未初始化".to_string())?;
    let manager = &*manager_guard;
    manager
        .with_store_mut(|store| store.set_favorite(id, favorite))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    manager.schedule_push();
    Ok(())
}
```

注意：`with_store_mut` 返回嵌套 `Result`，因此必须先映射外层锁错误，再映射 `StoreError`；不要吞掉 Store 失败。

在 `handle_ui_response` 增加：

```rust
UiResponse::ToggleSnippetFavorite { id, favorite } => {
    let result = set_snippet_favorite(&id, favorite);
    if favorite_change_effect(result).refresh {
        #[cfg(target_os = "windows")]
        send_refresh_to_search();
    }
}
```

将 match 开头的强制 `MANAGER` 获取移入只需要 manager 的分支，或让 `ToggleSnippetFavorite` 分支在先调用 helper；否则 helper 未初始化时的错误语义会被提前 `return` 掩盖。既有创建、更新、删除分支必须保留当前行为。

- [ ] **步骤 4：运行 app 定向测试，确认绿灯**

运行：`cargo test app`

预期：通过；`UiResponse` 能携带明确的目标状态，而非依据可能过期 UI 快照的 toggle。

- [ ] **步骤 5：提交应用事件变更**

```bash
git add src/snippet/ui/mod.rs src/app.rs
git commit -m "feat: handle snippet favorite changes"
```

---

### 任务 5：实现 Windows 收藏标签栏和搜索结果右键收藏

**文件：**
- 修改：`src/snippet/ui/search.rs:1-325`

- [ ] **步骤 1：为无 GUI 依赖的收藏栏筛选和菜单判定写失败测试**

在 `search.rs` 测试模块导入 `Snippet`、`SnippetType` 和 `chrono`，添加：

```rust
#[test]
fn favorite_bar_excludes_deleted_folders_and_unfavorited_items_and_sorts_newest_first() {
    let base = chrono::Utc::now() - chrono::Duration::hours(1);
    let mut older = Snippet::new_snippet("older".into(), "old".into(), None);
    older.favorite = true;
    older.update_date = base;
    let mut newer = Snippet::new_snippet("newer".into(), "new".into(), None);
    newer.favorite = true;
    newer.update_date = base + chrono::Duration::minutes(1);
    let folder = Snippet::new_folder("folder".into(), None);
    let unfavorited = Snippet::new_snippet("plain".into(), "plain".into(), None);
    let mut deleted = Snippet::new_snippet("deleted".into(), "deleted".into(), None);
    deleted.favorite = true;
    deleted.deleted = true;

    let favorites = favorite_snippets(&[older, folder, unfavorited, deleted, newer]);

    assert_eq!(favorites.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(), vec!["newer", "older"]);
}

#[test]
fn favorite_action_only_targets_active_regular_snippets() {
    let plain = Snippet::new_snippet("plain".into(), "content".into(), None);
    let mut favorite = Snippet::new_snippet("favorite".into(), "content".into(), None);
    favorite.favorite = true;
    let folder = Snippet::new_folder("folder".into(), None);
    let mut deleted = Snippet::new_snippet("deleted".into(), "content".into(), None);
    deleted.deleted = true;

    assert_eq!(favorite_action_for(&plain), Some(true));
    assert_eq!(favorite_action_for(&favorite), Some(false));
    assert_eq!(favorite_action_for(&folder), None);
    assert_eq!(favorite_action_for(&deleted), None);
}
```

- [ ] **步骤 2：运行 UI 单元测试，确认纯函数缺失**

运行：`cargo test snippet::ui::search`

预期：编译失败，包含 `cannot find function 'favorite_snippets'` 和 `cannot find function 'favorite_action_for'`。

- [ ] **步骤 3：实现纯函数和收藏标签栏**

在 `SearchPopup` 前添加：

```rust
fn favorite_snippets(snippets: &[Snippet]) -> Vec<Snippet> {
    let mut favorites: Vec<Snippet> = snippets
        .iter()
        .filter(|snippet| {
            !snippet.deleted
                && snippet.snippet_type == crate::snippet::model::SnippetType::Snippet
                && snippet.favorite
        })
        .cloned()
        .collect();
    favorites.sort_by(|left, right| right.update_date.cmp(&left.update_date));
    favorites
}

fn favorite_action_for(snippet: &Snippet) -> Option<bool> {
    (!snippet.deleted
        && snippet.snippet_type == crate::snippet::model::SnippetType::Snippet)
        .then_some(!snippet.favorite)
}
```

在搜索框 `ui.horizontal` 和现有 `ui.separator()` 之间，渲染：

```rust
let favorites = favorite_snippets(&self.snippets);
let mut copied_content = None;
let mut favorite_change = None;
if !favorites.is_empty() {
    ui.horizontal_wrapped(|ui| {
        for snippet in favorites {
            ui.push_id(&snippet.id, |ui| {
                let max_width = ui.available_width().min(220.0);
                let response = ui
                    .allocate_ui_with_layout(
                        egui::vec2(max_width, 0.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| ui.add(egui::Button::new(format!("★ {}", snippet.name)).truncate()),
                    )
                    .inner;
                if response.clicked() {
                    copied_content = snippet.content.clone();
                }
                response.context_menu(|ui| {
                    if ui.button("取消收藏").clicked() {
                        favorite_change = Some((snippet.id.clone(), false));
                        ui.close_menu();
                    }
                });
            });
        }
    });
}
```

把现有 `copy_index` 改为一个贯穿该窗口绘制闭包的 `Option<String>`，使标签点击和 Ctrl+Enter 都写入同一变量；在 UI 块末尾一次性发送 `UiResponse::SnippetCopied`。在响应发送处追加：

```rust
if let Some((id, favorite)) = favorite_change {
    if let Some(sender) = &self.resp_sender {
        let _ = sender.send(UiResponse::ToggleSnippetFavorite { id, favorite });
    }
}
```

在 Windows 标签渲染中限制按钮可用宽度并启用文字裁剪：每个标签外层使用 `ui.allocate_ui_with_layout(egui::vec2(ui.available_width().min(220.0), 0.0), ...)`，并以 `ui.add(egui::Button::new(format!("★ {}", snippet.name)).truncate())` 创建标签按钮。必须保持默认 Button 的 fill/stroke/圆角；只约束标签宽度，不引入自定义视觉。人工验收加入超长名称：在窄窗口中不撑破布局，标签仍能换行、左键复制与右键取消收藏。

- [ ] **步骤 4：为搜索结果行取得精确 Response，并绑定右键菜单**

保留行的选中背景、文字、键盘导航和内容预览。不要再对 `ui.available_rect_before_wrap()` 整块区域作交互；在每行开始处使用明确高度：

```rust
let row_height = ui.spacing().interact_size.y;
let (row_rect, row_response) = ui.allocate_exact_size(
    egui::vec2(ui.available_width(), row_height),
    egui::Sense::click(),
);
```

`row_rect` 内必须承载文字和预览，而不是在分配 rect 后继续使用外层 `ui` 排版。使用：

```rust
ui.painter().rect_filled(row_rect, 2.0, bg);
ui.allocate_ui_at_rect(row_rect, |row_ui| {
    row_ui.horizontal(|row_ui| {
        row_ui.colored_label(foreground, format!("{} {}", prefix, snippet.name));
        if !content_preview.is_empty() {
            row_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |row_ui| {
                row_ui.label(egui::RichText::new(&content_preview).small());
            });
        }
    });
});
```

将 `foreground` 按当前选中状态从既有 visuals 计算。所有左键选中和右键菜单都绑定同一个 `row_response`，人工验收时须确认右键文字所在位置命中本条目而不是前一空白行。

```rust
row_response.context_menu(|ui| {
    let label = if target { "收藏" } else { "取消收藏" };
    if ui.button(label).clicked() {
        favorite_change = Some((snippet.id.clone(), target));
        ui.close_menu();
    }
});
```

文件夹和删除项不挂载菜单。右键本身不复制、不改变搜索文本，也不触发编辑或删除。

- [ ] **步骤 5：运行 Windows UI 逻辑测试**

运行：`cargo test snippet::ui::search`

预期：通过；测试表明收藏栏仅显示有效普通片段、按更新时间倒序，菜单只为可收藏项目产生目标状态。

- [ ] **步骤 6：提交 Windows UI 变更**

```bash
git add src/snippet/ui/search.rs
git commit -m "feat: add favorite snippet tabs to search"
```

---

### 任务 6：实现 Linux 收藏栏、右键菜单并补齐 Linux 写入同步调度

**文件：**
- 修改：`src/snippet/ui/linux_manager/mod.rs:1-1064`

- [ ] **步骤 1：为 Linux 可复用收藏逻辑写失败测试**

在该文件测试模块增加：

```rust
#[test]
fn linux_favorite_bar_uses_only_active_favorited_snippets_sorted_by_latest_update() {
    let base = chrono::Utc::now() - chrono::Duration::hours(1);
    let mut old = crate::snippet::model::Snippet::new_snippet("old".into(), "old".into(), None);
    old.favorite = true;
    old.update_date = base;
    let mut new = crate::snippet::model::Snippet::new_snippet("new".into(), "new".into(), None);
    new.favorite = true;
    new.update_date = base + chrono::Duration::minutes(1);
    let folder = crate::snippet::model::Snippet::new_folder("folder".into(), None);

    assert_eq!(favorite_bar_snippets(&[old, folder, new])
        .iter().map(|snippet| snippet.name.as_str()).collect::<Vec<_>>(), vec!["new", "old"]);
}

#[test]
fn linux_context_menu_label_is_absent_for_folder_or_deleted_item() {
    let plain = crate::snippet::model::Snippet::new_snippet("plain".into(), "content".into(), None);
    let mut favorite = crate::snippet::model::Snippet::new_snippet("favorite".into(), "content".into(), None);
    favorite.favorite = true;
    let folder = crate::snippet::model::Snippet::new_folder("folder".into(), None);

    assert_eq!(favorite_menu_label(&plain), Some("收藏"));
    assert_eq!(favorite_menu_label(&favorite), Some("取消收藏"));
    assert_eq!(favorite_menu_label(&folder), None);
}
```

- [ ] **步骤 2：运行 Linux UI 测试，确认 helper 缺失**

运行：`cargo test snippet::ui::linux_manager`

预期：编译失败，包含 `cannot find function 'favorite_bar_snippets'` 和 `cannot find function 'favorite_menu_label'`。

- [ ] **步骤 3：实现收藏栏纯函数及 GTK FlowBox 刷新函数**

在文件顶层添加：

```rust
fn favorite_bar_snippets(snippets: &[crate::snippet::model::Snippet]) -> Vec<crate::snippet::model::Snippet> {
    let mut favorites: Vec<_> = snippets
        .iter()
        .filter(|snippet| {
            !snippet.deleted
                && snippet.snippet_type == crate::snippet::model::SnippetType::Snippet
                && snippet.favorite
        })
        .cloned()
        .collect();
    favorites.sort_by(|left, right| right.update_date.cmp(&left.update_date));
    favorites
}

fn favorite_menu_label(snippet: &crate::snippet::model::Snippet) -> Option<&'static str> {
    if snippet.deleted || snippet.snippet_type != crate::snippet::model::SnippetType::Snippet {
        None
    } else if snippet.favorite {
        Some("取消收藏")
    } else {
        Some("收藏")
    }
}
```

在 `SearchEntry` 后、`Paned` 前创建 `gtk::FlowBox`：

```rust
let favorite_box = gtk::FlowBox::new();
favorite_box.set_selection_mode(gtk::SelectionMode::None);
favorite_box.set_max_children_per_line(0);
favorite_box.set_margin_bottom(6);
content.pack_start(&favorite_box, false, true, 0);
```

提取具名函数而非自引用闭包，例如：

```rust
fn refresh_favorite_bar(
    favorite_box: &gtk::FlowBox,
    dialog: &gtk::Dialog,
    target_window: Option<u64>,
    refresh_tree: &std::rc::Rc<dyn Fn(Option<String>, Option<String>)>,
    current_query: &gtk::SearchEntry,
) {
    // 清空旧 child，从 Store 读取快照，按 favorite_bar_snippets 重建按钮。
}
```

将 `refresh_tree_store` 放入 `Rc<dyn Fn(Option<String>, Option<String>)>`，以便标签和 TreeView 的 `'static` 信号回调都捕获 clone；不要让刷新闭包在初始化时捕获自身。每次先移除旧 child；读 Store 快照并执行 `favorite_bar_snippets`；每条使用普通 `gtk::Button`，内部改放一个带椭圆裁剪的 `gtk::Label`：

```rust
let label = gtk::Label::new(Some(&format!("★ {}", snippet.name)));
label.set_ellipsize(gtk::pango::EllipsizeMode::End);
label.set_max_width_chars(24);
label.set_xalign(0.0);
let button = gtk::Button::new();
button.add(&label);
```

按钮点击必须调用现有：

```rust
copy_and_paste_snippet(&dialog, content, &snippet.name, target_window);
```

标签按钮通过 `add_events(BUTTON_PRESS_MASK)` 与 `connect_button_press_event` 在右键时显示只含“取消收藏”的 `gtk::Menu`/`gtk::MenuItem`。菜单回调调用：

```rust
crate::app::set_snippet_favorite(&snippet_id, false)
```

成功后调用 `refresh_tree_store(current_query, Some(snippet_id))` 与 `refresh_favorite_bar()`；失败时不刷新。收藏栏无项目时 `favorite_box.hide()`，否则 `show_all()`。不要添加 `suggested-action`、`destructive-action` 或任何 CSS class：保留系统 GTK 默认 Button 风格。

在初始 `content.show_all()` 前调用一次 `refresh_favorite_bar()`。

- [ ] **步骤 4：让 TreeView 右键按点击行操作当前片段**

在已有 `tree_view.add_events(BUTTON_PRESS_MASK | ...)` 后连接 button press。实现必须：

1. 只处理 `event.button() == 3`；
2. 使用 `tree_view.path_at_pos(event.position().0 as i32, event.position().1 as i32)` 找命中 path；
3. 选择 path，防止右键 A 却修改之前选中的 B；
4. 从 TreeStore 读取第 0 列 ID 和第 3 列 `is_folder`；文件夹直接 `gtk::Inhibit(false)`；
5. 通过 Store 的 `get(&id)` 获取当前片段并用 `favorite_menu_label` 判定；
6. 创建仅有一个普通 `gtk::MenuItem` 的 `gtk::Menu`，标签为“收藏”或“取消收藏”；激活时将 `target = !snippet.favorite` 传给 `set_snippet_favorite`；
7. 成功后按当前搜索框文字刷新树与收藏栏，并选回该 ID；调用 `menu.popup_easy(event.button(), event.time())`，返回 `gtk::Inhibit(true)`。

不得把复制、编辑、删除混入此菜单。

- [ ] **步骤 5：在 Linux 所有成功 Store 写入后调度同步并刷新收藏栏**

每个下列路径都必须只在 Store 调用返回 `Ok` 后执行 `manager.schedule_push()`，并在界面刷新时同时刷新 `FlowBox`：

- 拖动移动到文件夹或根目录：`src/snippet/ui/linux_manager/mod.rs:466-485`；
- 文件夹重命名：`640-654`；
- 普通片段编辑：`679-704`；
- 删除：`709-735`；
- 导入完成且 `summary.added > 0`：`769-781`；
- 新建文件夹：`916-935`；
- 新建普通片段：`953-977`。

将每处原先忽略的嵌套结果改为显式成功判定，例如：

```rust
let updated = manager
    .with_store_mut(|store| store.update(&id, |snippet| { /* existing mutation */ }))
    .ok()
    .and_then(Result::ok)
    .is_some();
if updated {
    manager.schedule_push();
    refresh_tree_store(...);
    refresh_favorite_bar();
}
```

保持既有通知文案，但只在成功写入后显示“已更新”或“已删除”。这修复了现有 Linux 修改不会调度自动同步的缺口，确保收藏和编辑的跨设备行为一致。

- [ ] **步骤 6：运行 Linux UI 测试并构建默认目标**

运行：`cargo test snippet::ui::linux_manager && cargo build`

预期：均通过；纯函数测试确认文件夹和删除项不会出现在收藏栏或菜单中，Linux GTK 代码可编译。

- [ ] **步骤 7：提交 Linux UI 与调度变更**

```bash
git add src/snippet/ui/linux_manager/mod.rs
git commit -m "feat: add linux snippet favorite controls"
```

---

### 任务 7：格式化、跨模块回归与人工验收

**文件：**
- 修改：仅格式化工具实际改动的 Rust 文件

- [ ] **步骤 1：格式化并检查工作区状态**

运行：`cargo fmt --check`

若失败，运行：`cargo fmt`

然后运行：`cargo fmt --check && git status --short`

预期：格式检查通过；状态只包含本功能相关文件及实施前已存在的未跟踪文件，不得删除或纳入无关的 `.learnings/`、`.superpowers/`、脚本和其他计划/规格文档。

- [ ] **步骤 2：运行完整自动化测试**

运行：`cargo test`

预期：退出码 0；覆盖模型默认值、Store 收藏操作、Gitee tree、GitHub YAML 新旧兼容、重命名顺序/失败路径、UI 纯逻辑和现有回归测试。

- [ ] **步骤 3：构建 Linux 默认目标并尽力检查 Windows 目标**

运行：`cargo build && cargo check --target x86_64-pc-windows-gnu`

预期：`cargo build` 必须通过。若 Windows target 未安装或环境缺少其系统链接依赖，记录确切工具链错误，不修改代码或安装宿主依赖；若可用则两个命令均通过。

- [ ] **步骤 4：执行人工 UI 与同步回归检查**

在 Linux：

1. 打开代码片段管理器；对普通片段右键选择“收藏”，确认搜索框下出现 `★ 名称` 的普通 GTK 按钮，文件夹没有该菜单；
2. 窄化或增加多个收藏，确认 `FlowBox` 自动换行；
3. 左键标签，确认沿用现有剪贴板通知、窗口隐藏和自动粘贴；右键标签仅见“取消收藏”；
4. 取消收藏，确认标签立刻消失，树仍保持原有排序；
5. 修改、移动、新建、删除一个片段，确认原有 UI 行为不变且日志显示 `schedule_push called`（已配置自动同步时）。

在 Windows（如果可运行）：

1. 打开搜索弹窗，确认标签栏仅在有收藏时显示在搜索框下方；窗口较窄时标签换行，使用默认 egui Button 而非独立色板；
2. 左键标签确认复制；右键标签只显示“取消收藏”；右键搜索结果可收藏/取消收藏，文件夹无操作；
3. 确认 Ctrl+Enter、编辑、删除和上下键选择仍正常。

对 GitHub 测试仓库：

1. 同步一个收藏片段，检查 YAML 包含稳定 `id:` 与 `favorite: true`；
2. 重命名后同步，确认新路径先出现、旧路径被删除，两个文件不会长期共存；
3. 另一设备拉取，确认稳定 ID 对应同一片段且收藏状态恢复。

对 Gitee 测试 Gist：确认 `tree.json` 中写入 `favorite`，另一设备拉取后收藏栏恢复。

- [ ] **步骤 5：提交最终格式化或修复（若有）**

```bash
git add src/snippet/model.rs src/snippet/store.rs src/snippet/import_export.rs src/snippet/sync.rs src/snippet/mod.rs src/snippet/ui/mod.rs src/app.rs src/snippet/ui/search.rs src/snippet/ui/linux_manager/mod.rs
git commit -m "test: verify favorite snippet sync"
```

若 `git status --short` 显示上述文件均无未提交修改，则跳过此空提交；不得把无关未跟踪文件加入暂存区。
