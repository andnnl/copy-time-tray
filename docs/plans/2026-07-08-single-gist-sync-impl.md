# Single-Gist Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor sync to use a single "CopyTimeTrayConfig" gist (tree.json + N content files) instead of one gist per folder, with LWW conflict resolution.

**Architecture:**
- Add `GiteeClient::get_gist(id)` for direct gist lookup by stored ID
- Rewrite push to serialize tree.json + `{id}_{name}.txt` files into one gist; create new gist on first push, save ID to config
- Rewrite pull to GET gist by stored ID, parse tree + content files, merge with LWW (newer `update_date` wins, tie → local wins)
- Simplify `load_from_sync` to match only by `id`, removing deprecated `gitee_gist_id` matching

**Tech Stack:** Rust, reqwest (blocking), serde/serde_json, chrono, sha2, uuid, tempfile (tests)

---

## Context

### Codebase Findings (from exploration)

| Finding | File | Status |
|---------|------|--------|
| `GiteeConfig.gist_id: Option<String>` | `src/snippet/config.rs:33` | **Already exists** — no change needed |
| `Snippet.gitee_gist_id` with `skip_serializing_if` | `src/snippet/model.rs:25-27` | **Already exists** — keep for backward compat |
| `load_from_sync` LWW logic | `src/snippet/store.rs:134-165` | **Partially done** — still matches by `gitee_gist_id` first, needs simplification |
| `push_gitee` / `pull_gitee` (one-gist-per-folder) | `src/snippet/sync.rs:175-471` | **Must be rewritten** |
| `GiteeClient::get_gist(id)` | `src/snippet/gitee/client.rs` | **Missing — must add** |
| `SnippetManager::do_push/do_pull` | `src/snippet/mod.rs:97-228` | **Must update** to call new single-gist functions |

### Key Behavioral Rules
1. Push: serialize all non-deleted snippets → tree.json + `{id}_{name}.txt` files → POST/PATCH gist
2. Pull: GET gist by stored ID → parse tree.json → read content files → merge via LWW
3. LWW: `update_date` newer wins; tie → local wins (preserve user intent)
4. First push: POST new gist, save returned `id` to `config.gist_id`
5. Subsequent pushes: PATCH existing gist by `gist_id`
6. gist not found (404): treat as missing → create new gist, reset `gist_id`

### Gitee API Endpoints Used
- `POST /gists` — create new single gist
- `GET /gists/{id}` — fetch single gist directly (new)
- `PATCH /gists/{id}` — update existing gist
- `GET /gists` — list gists (only used for 404 recovery fallback)


## Task Dependency Graph

| Task | Depends On | Reason |
|------|------------|--------|
| Task 1: GiteeClient::get_gist | None | Foundation — all push/pull needs direct gist lookup |
| Task 2: Serialization + push_to_single_gist | Task 1 | Needs `get_gist` to check for existing gist before push |
| Task 3: Deserialization + pull_from_single_gist | Task 1 | Needs `get_gist` to fetch remote tree directly |
| Task 4: Simplify load_from_sync (LWW only) | Task 3 | Must match pull behavior — removes deprecated gist_id matching |
| Task 5: SnippetManager wiring | Task 2, Task 3, Task 4 | Calls push/pull, persists gist_id after first sync |
| Task 6: Remove old push_gitee/pull_gitee | Task 5 | Old code only removed after new path is confirmed working |

Critical path: Task 1 → Task 2 → Task 5
Parallel opportunity: Tasks 2 and 3 can run in parallel after Task 1 completes.

## Parallel Execution Graph

```
Wave 1 (Start immediately — no dependencies):
└── Task 1: GiteeClient::get_gist + unit test

Wave 2 (After Wave 1 — Tasks 2 & 3 run in parallel):
├── Task 2: Serialization + push_to_single_gist
└── Task 3: Deserialization + pull_from_single_gist

Wave 3 (After Wave 2 — all three run in parallel):
├── Task 4: Simplify load_from_sync
└── Task 5: SnippetManager wiring (needs Tasks 2+3 done)

Wave 4 (After Wave 3):
└── Task 6: Remove old push_gitee/pull_gitee
```

Critical Path: Task 1 → Task 2 → Task 5 → Task 6  
Estimated Speedup: ~40% faster than sequential (Tasks 2 & 3 parallelized)

## Tasks

---

### Task 1: Add `GiteeClient::get_gist(id)` + Unit Test

**Files:**
- Modify: `src/snippet/gitee/client.rs:1-155` (add method after `url_gist`)
- Test: `src/snippet/gitee/client.rs` (inline `#[cfg(test)]`)

**Step 1: Write the failing test**

Add to the existing `#[cfg(test)]` module at the bottom of `client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_gist_requests_correct_url() {
        // Build a client with a dummy config
        let config = GiteeConfig {
            api_base: "https://gitee.com/api/v5".into(),
            token: "test-token".into(),
            auto_sync: true,
            sync_interval: 300,
            gist_id: None,
            gist_description: "CopyTimeTrayConfig".into(),
        };
        let client = GiteeClient::new(config);

        // Verify url_gist helper produces correct URL
        let url = client.url_gist("abc123");
        assert_eq!(url, "https://gitee.com/api/v5/gists/abc123");
    }

    #[test]
    fn test_get_gist_returns_gist_on_200() {
        use std::sync::Mutex;
        thread_local! {
            static CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());
        }

        // Use a mock server via wiremock or a simple local server
        // For now verify the method signature and URL construction:
        let config = GiteeConfig {
            api_base: "http://localhost:9999".into(),
            token: "tok".into(),
            auto_sync: true,
            sync_interval: 300,
            gist_id: None,
            gist_description: "CopyTimeTrayConfig".into(),
        };
        let client = GiteeClient::new(config);
        let url = client.url_gist("xyz789");
        assert!(url.contains("/gists/xyz789"));
    }
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test --lib gitee::client -- --nocapture
```
Expected: FAIL — `no function or associated item named \`get_gist\` found`

**Step 3: Write minimal implementation**

Add this method to `GiteeClient` in `client.rs` (after the `url_gist` method):

```rust
    /// Fetch a single gist by ID via direct GET.
    pub fn get_gist(&self, id: &str) -> Result<Gist> {
        let url = self.url_gist(id);
        eprintln!("[DEBUG] get_gist: GET {}", url);
        let resp = self
            .with_auth(self.client.get(&url))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        let status = resp.status();
        eprintln!("[DEBUG] get_gist: HTTP {}", status);
        if status == 404 {
            return Err(error::GiteeError::Http(format!(
                "Gist {} not found (404)",
                id
            )));
        }
        self.check_status(resp)?
            .json::<Gist>()
            .map_err(|e| {
                eprintln!("[DEBUG] get_gist: JSON parse error: {}", e);
                error::GiteeError::Parse(e.to_string())
            })
    }
```

**Step 4: Run test to verify it passes**

```bash
cargo test --lib gitee::client::tests -- --nocapture
```
Expected: PASS

**Step 5: Commit**

```bash
git add src/snippet/gitee/client.rs
git commit -m "feat(gitee): add GiteeClient::get_gist(id) for direct gist lookup"
```

---

### Task 2: Serialization Helper + `push_to_single_gist()`

**Files:**
- Modify: `src/snippet/sync.rs:1-750` (add `TreeEntry` struct, `build_tree_entries`, `push_to_single_gist`)
- Modify: `src/snippet/sync.rs:345-471` (replace `push_gitee` body)

**Step 1: Write the failing test**

Add these tests to `sync.rs` `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn test_tree_entry_serialization() {
        use crate::snippet::model::SnippetType;
        let entry = TreeEntry {
            id: "uuid-1".into(),
            parent_id: None,
            name: "网络工具".into(),
            entry_type: SnippetType::Folder,
            sort: 0,
            deleted: false,
            create_date: chrono::Utc::now(),
            update_date: chrono::Utc::now(),
            content_file: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(""id":"uuid-1""));
        assert!(json.contains(""type":"folder""));
        assert!(json.contains(""content_file""), "content_file should be present for folder as null");
    }

    #[test]
    fn test_tree_entry_snippet_has_content_file() {
        use crate::snippet::model::SnippetType;
        let entry = TreeEntry {
            id: "uuid-2".into(),
            parent_id: Some("uuid-1".into()),
            name: "ping".into(),
            entry_type: SnippetType::Snippet,
            sort: 0,
            deleted: false,
            create_date: chrono::Utc::now(),
            update_date: chrono::Utc::now(),
            content_file: Some("uuid-2_ping.txt".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(""type":"snippet""));
        assert!(json.contains(""content_file":"uuid-2_ping.txt""));
    }

    #[test]
    fn test_build_tree_entries_includes_folders_and_snippets() {
        let folder = Snippet::new_folder("网络工具".into(), None);
        let snippet = Snippet::new_snippet("ping".into(), "echo ping".into(), Some(folder.id.clone()));

        let entries = build_tree_entries(&[folder, snippet]);
        assert_eq!(entries.len(), 2);
        // Snippet entry should have content_file set
        let snip_entry = entries.iter().find(|e| e.entry_type == crate::snippet::model::SnippetType::Snippet).unwrap();
        assert!(snip_entry.content_file.is_some());
        assert!(snip_entry.content_file.as_ref().unwrap().ends_with("_ping.txt"));
    }
```

**Step 2: Run tests to verify they fail**

```bash
cargo test --lib sync::tests::test_tree_entry -- --nocapture
```
Expected: FAIL — `TreeEntry` not found

**Step 3: Write minimal implementation**

Add `TreeEntry` struct and helpers near the top of `sync.rs` (after existing `GitTreeEntry`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: crate::snippet::model::SnippetType,
    pub sort: i32,
    pub deleted: bool,
    pub create_date: chrono::DateTime<chrono::Utc>,
    pub update_date: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_file: Option<String>,
}

pub fn build_tree_entries(snippets: &[Snippet]) -> Vec<TreeEntry> {
    snippets
        .iter()
        .map(|s| TreeEntry {
            id: s.id.clone(),
            parent_id: s.parent_id.clone(),
            name: s.name.clone(),
            entry_type: s.snippet_type.clone(),
            sort: s.sort,
            deleted: s.deleted,
            create_date: s.create_date,
            update_date: s.update_date,
            content_file: match s.snippet_type {
                crate::snippet::model::SnippetType::Snippet => {
                    Some(format!("{}_{}.txt", s.id, s.name))
                }
                crate::snippet::model::SnippetType::Folder => None,
            },
        })
        .collect()
}
```

Add `push_to_single_gist` function (replace `push_gitee`):

```rust
fn push_to_single_gist(
    config: &SyncConfig,
    client: &dyn HttpClient,
    snippets: &[Snippet],
) -> Result<Vec<String>, SyncError> {
    let gitee_config = &config.gitee_config;

    // Filter out deleted snippets; only push non-deleted
    let active: Vec<&Snippet> = snippets
        .iter()
        .filter(|s| !s.deleted)
        .collect();

    if active.is_empty() {
        return Ok(Vec::new());
    }

    // Build tree.json content
    let tree_entries = build_tree_entries(&active);
    let tree_json = serde_json::to_string_pretty(&tree_entries)
        .map_err(|e| SyncError::Store(e.to_string()))?;

    // Build files map: tree.json + content files
    let mut files: HashMap<String, String> = HashMap::new();
    files.insert("tree.json".into(), tree_json);
    for s in &active {
        if let Some(ref content) = s.content {
            let filename = format!("{}_{}.txt", s.id, s.name);
            files.insert(filename, content.clone());
        }
    }

    // Determine gist_id: check config first, then try to find existing gist
    let gist_id = match &gitee_config.gist_id {
        Some(id) => {
            // Verify gist still exists
            let gitee_client = GiteeClient::new(gitee_config.clone());
            match gitee_client.get_gist(id) {
                Ok(_) => id.clone(),
                Err(_) => {
                    // Gist gone, create new
                    create_new_gist(gitee_config, &files)?
                }
            }
        }
        None => {
            // No existing gist, create new
            create_new_gist(gitee_config, &files)?
        }
    };

    // Update gist with current files
    let mut update = GistUpdate::new(Some(&gitee_config.gist_description));
    for (filename, content) in &files {
        update = update.add_file(filename, content);
    }

    let gitee_client = GiteeClient::new(gitee_config.clone());
    gitee_client
        .update_gist(&gist_id, &update)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    Ok(Vec::new())
}

fn create_new_gist(
    gitee_config: &GiteeConfig,
    files: &HashMap<String, String>,
) -> Result<String, SyncError> {
    let mut gist_files = HashMap::new();
    for (name, content) in files {
        gist_files.insert(name.clone(), GistFileCreate::new(Some(content)));
    }

    let create = GistCreate {
        description: Some(&gitee_config.gist_description),
        files: gist_files,
        public: false,
    };

    let gitee_client = GiteeClient::new(gitee_config.clone());
    let gist = gitee_client
        .create_gist(&create)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    Ok(gist.id)
}
```

Replace `push_gitee` call in the public `push` function:
```rust
    SyncProvider::Gitee => {
        push_to_single_gist(config, client, snippets)
    }
```

**Step 4: Run tests to verify they pass**

```bash
cargo test --lib sync::tests::test_tree_entry -- --nocapture
cargo test --lib sync::tests::test_build_tree_entries -- --nocapture
```
Expected: PASS

**Step 5: Commit**

```bash
git add src/snippet/sync.rs
git commit -m "feat(sync): add TreeEntry serialization + push_to_single_gist"
```

---

### Task 3: Deserialization + `pull_from_single_gist()`

**Files:**
- Modify: `src/snippet/sync.rs:1-750` (add `pull_from_single_gist`, replace `pull_gitee`)

**Step 1: Write the failing test**

Add these tests to `sync.rs` `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn test_pull_from_single_gist_reconstructs_snippets() {
        // Mock gist response with tree.json + content files
        let gist_json = r#"{
            "id": "gist123",
            "description": "CopyTimeTrayConfig",
            "files": {
                "tree.json": {
                    "content": "[{\"id\":\"f1\",\"parent_id\":null,\"name\":\"网络工具\",\"type\":\"folder\",\"sort\":0,\"deleted\":false,\"create_date\":\"2026-01-01T00:00:00Z\",\"update_date\":\"2026-01-01T00:00:00Z\"},{\"id\":\"s1\",\"parent_id\":\"f1\",\"name\":\"ping\",\"type\":\"snippet\",\"sort\":0,\"deleted\":false,\"create_date\":\"2026-01-01T00:00:00Z\",\"update_date\":\"2026-01-02T00:00:00Z\",\"content_file\":\"s1_ping.txt\"}]"
                },
                "s1_ping.txt": {
                    "content": "echo ping\n"
                }
            },
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z"
        }"#;

        // We verify the tree parsing directly
        let tree_content = "[{"id":"f1","parent_id":null,"name":"网络工具","type":"folder","sort":0,"deleted":false,"create_date":"2026-01-01T00:00:00Z","update_date":"2026-01-01T00:00:00Z"},{"id":"s1","parent_id":"f1","name":"ping","type":"snippet","sort":0,"deleted":false,"create_date":"2026-01-01T00:00:00Z","update_date":"2026-01-02T00:00:00Z","content_file":"s1_ping.txt"}]";
        let entries: Vec<TreeEntry> = serde_json::from_str(tree_content).unwrap();
        assert_eq!(entries.len(), 2);
        let snip = entries.iter().find(|e| e.id == "s1").unwrap();
        assert_eq!(snip.content_file, Some("s1_ping.txt".into()));
        assert_eq!(snip.parent_id, Some("f1".into()));
    }
```

**Step 2: Run test to verify it fails**

```bash
cargo test --lib sync::tests::test_pull_from_single_gist -- --nocapture
```
Expected: FAIL — `TreeEntry` or `pull_from_single_gist` not found

**Step 3: Write minimal implementation**

Replace `pull_gitee` with:

```rust
fn pull_from_single_gist(
    config: &SyncConfig,
    client: &dyn HttpClient,
) -> Result<Vec<Snippet>, SyncError> {
    let gitee_config = &config.gitee_config;
    let gist_id = match &gitee_config.gist_id {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let gitee_client = GiteeClient::new(gitee_config.clone());
    let gist = gitee_client
        .get_gist(gist_id)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    // Parse tree.json
    let tree_file = gist.files.get("tree.json").ok_or_else(|| {
        SyncError::Parse("tree.json not found in gist".into())
    })?;

    let tree_content = tree_file.content.as_deref().unwrap_or("");
    if tree_content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let tree_entries: Vec<TreeEntry> =
        serde_json::from_str(tree_content).map_err(|e| {
            SyncError::Parse(format!("Failed to parse tree.json: {}", e))
        })?;

    // Reconstruct snippets from tree + content files
    let mut snippets = Vec::new();
    for entry in tree_entries {
        if entry.deleted {
            continue;
        }

        let content = match entry.entry_type {
            crate::snippet::model::SnippetType::Snippet => {
                if let Some(ref content_file) = entry.content_file {
                    gist.files.get(content_file)
                        .and_then(|f| f.content.clone())
                } else {
                    None
                }
            }
            crate::snippet::model::SnippetType::Folder => None,
        };

        snippets.push(Snippet {
            id: entry.id,
            parent_id: entry.parent_id,
            name: entry.name,
            snippet_type: entry.entry_type,
            content,
            sort: entry.sort,
            deleted: entry.deleted,
            synced: true,
            create_date: entry.create_date,
            update_date: entry.update_date,
            gitee_gist_id: None,
        });
    }

    Ok(snippets)
}
```

Replace `pull_gitee` call in the public `pull` function:
```rust
    SyncProvider::Gitee => {
        pull_from_single_gist(config, client)
    }
```

**Step 4: Run tests to verify they pass**

```bash
cargo test --lib sync::tests::test_pull_from_single_gist -- --nocapture
cargo test --lib sync::tests::test_tree_entry_serialization -- --nocapture
```
Expected: PASS

**Step 5: Commit**

```bash
git add src/snippet/sync.rs
git commit -m "feat(sync): add pull_from_single_gist with tree.json parsing"
```

---

### Task 4: Simplify `load_from_sync` — Remove `gitee_gist_id` Matching

**Files:**
- Modify: `src/snippet/store.rs:134-165` (simplify `load_from_sync`)

**Step 1: Write the failing test**

Add this test to `store.rs` `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn test_load_from_sync_no_gist_id_matching() {
        let (mut store, _dir) = temp_store();

        // Local snippet with gitee_gist_id set (old-style)
        let mut local = Snippet::new_snippet("old".into(), "local-content".into(), None);
        local.id = "s1".into();
        local.gitee_gist_id = Some("gist-xxx".into());
        local.update_date = chrono::Utc::now() - chrono::Duration::minutes(10);
        store.add(local).unwrap();

        // Remote snippet with same ID but different content, newer update_date
        let mut remote = Snippet::new_snippet("new".into(), "remote-content".into(), None);
        remote.id = "s1".into();
        remote.gitee_gist_id = Some("gist-yyy".into()); // Different gist_id — should be ignored
        remote.update_date = chrono::Utc::now();
        remote.synced = true;

        store.load_from_sync(vec![remote]).unwrap();

        // Should match by id, not by gitee_gist_id
        let result = store.get("s1").unwrap();
        assert_eq!(result.content, Some("remote-content".into()));
        assert_eq!(result.name, "new");
    }
```

**Step 2: Run test to verify it fails**

```bash
cargo test --lib store::tests::test_load_from_sync_no_gist_id_matching -- --nocapture
```
Expected: FAIL — currently matches by `gitee_gist_id` first, so `gist-xxx` matches local and old content is preserved

**Step 3: Write minimal implementation**

Modify `load_from_sync` in `store.rs`:

```rust
    pub fn load_from_sync(&mut self, remote_snippets: Vec<Snippet>) -> Result<(), StoreError> {
        use std::collections::HashMap;

        let by_id: HashMap<String, usize> = self.data.snippets.iter().enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();

        for remote in remote_snippets {
            if let Some(idx) = by_id.get(&remote.id) {
                let existing = &mut self.data.snippets[*idx];
                if remote.update_date > existing.update_date {
                    *existing = remote;
                }
                // Same timestamp → keep local (user intent preserved)
            } else {
                self.data.snippets.push(remote);
            }
        }
        self.save()
    }
```

**Step 4: Run test to verify it passes**

```bash
cargo test --lib store::tests::test_load_from_sync_no_gist_id_matching -- --nocapture
cargo test --lib store::tests::test_load_from_sync_lww_newer_wins -- --nocapture
```
Expected: PASS

**Step 5: Commit**

```bash
git add src/snippet/store.rs
git commit -m "refactor(store): simplify load_from_sync — match by id only (remove gist_id matching)"
```

---

### Task 5: SnippetManager Wiring — Call New Push/Pull + Save gist_id

**Files:**
- Modify: `src/snippet/mod.rs:97-228` (`do_push`, `do_pull`, `start_background_sync`, `schedule_push`)

**Step 1: Write the failing test**

Add this integration-style unit test to `mod.rs` (or to `sync.rs` tests):

In `sync.rs` `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn test_push_to_single_gist_creates_gist_and_returns_id() {
        // This test verifies the full push flow via a mock HTTP client
        use std::sync::Mutex;
        thread_local! {
            static MOCK_RECORDS: Mutex<Vec<(String, String, String)>> = Mutex::new(Vec::new());
        }

        struct MockGistClient;
        impl HttpClient for MockGistClient {
            fn get(&self, url: &str, _headers: &HashMap<String, String>) -> Result<HttpResponse, SyncError> {
                MOCK_RECORDS.with(|r| r.lock().unwrap().push(("GET".into(), url.into(), "".into())));
                Ok(HttpResponse { status: 404, body: "{}".into() })
            }
            fn put(&self, url: &str, _headers: &HashMap<String, String>, body: &str) -> Result<HttpResponse, SyncError> {
                MOCK_RECORDS.with(|r| r.lock().unwrap().push(("PUT".into(), url.into(), body.into())));
                // Return a fake gist with ID
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"id":"new-gist-id","files":{}}"#.into(),
                })
            }
            fn delete(&self, _url: &str, _headers: &HashMap<String, String>) -> Result<HttpResponse, SyncError> {
                unimplemented!()
            }
        }

        let config = SyncConfig {
            provider: SyncProvider::Gitee,
            owner: "u".into(),
            repo: "r".into(),
            branch: "master".into(),
            token: "t".into(),
            gitee_config: GiteeConfig {
                api_base: "https://gitee.com/api/v5".into(),
                token: "t".into(),
                auto_sync: true,
                sync_interval: 300,
                gist_id: None,
                gist_description: "CopyTimeTrayConfig".into(),
            },
        };

        let snippets = vec![Snippet::new_snippet("hello".into(), "echo hi".into(), None)];
        let result = push_to_single_gist(&config, &MockGistClient, &snippets);
        assert!(result.is_ok());

        // Verify that a PUT was made to /gists/ (create returns id, then PATCH updates)
        MOCK_RECORDS.with(|r| {
            let records = r.lock().unwrap();
            let urls: Vec<&str> = records.iter().map(|(_, url, _)| url.as_str()).collect();
            assert!(urls.iter().any(|u| u.contains("/gists")), "Expected gist URL, got: {:?}", urls);
        });
    }
```

**Step 2: Run test to verify it fails**

```bash
cargo test --lib sync::tests::test_push_to_single_gist_creates_gist -- --nocapture
```
Expected: FAIL — `push_to_single_gist` not found (or `MockGistClient` type mismatch)

**Step 3: Write minimal implementation**

Modify `do_push` and `do_pull` in `mod.rs`:

For `do_push`:
```rust
    pub fn do_push(&self) -> Result<(), String> {
        let sync_config = self
            .config
            .sync
            .as_ref()
            .ok_or_else(|| "未配置同步".to_string())?;
        let pending: Vec<Snippet> = {
            let store = self.store.lock().map_err(|e| e.to_string())?;
            store.pending_sync().into_iter().cloned().collect()
        };
        if pending.is_empty() {
            return Ok(());
        }
        for attempt in 0..MAX_RETRIES {
            match sync::push_to_single_gist(sync_config, &self.client, &pending) {
                Ok(_) => {
                    // Mark all pending as synced
                    if let Ok(mut store) = self.store.lock() {
                        for s in &pending {
                            let _ = store.update(&s.id, |snip| {
                                snip.synced = true;
                            });
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    let backoff = Duration::from_secs(INITIAL_BACKOFF_SECS * (1 << attempt));
                    log::warn!(
                        "do_push attempt {}/{} failed: {}, retrying in {:?}",
                        attempt + 1,
                        MAX_RETRIES,
                        e,
                        backoff
                    );
                    if attempt + 1 < MAX_RETRIES {
                        thread::sleep(backoff);
                    }
                }
            }
        }
        Err(format!("push failed after {} attempts", MAX_RETRIES))
    }
```

For `do_pull`:
```rust
    pub fn do_pull(&self) -> Result<(), String> {
        let sync_config = self
            .config
            .sync
            .as_ref()
            .ok_or_else(|| "未配置同步".to_string())?;
        let remote = sync::pull_from_single_gist(sync_config, &self.client)
            .map_err(|e| e.to_string())?;
        if !remote.is_empty() {
            if let Ok(mut store) = self.store.lock() {
                store.load_from_sync(remote).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
```

Add helper to save `gist_id` to config after push creates a new gist. Add this to `sync.rs`:

```rust
pub fn get_or_create_gist_id(
    config: &SyncConfig,
    client: &dyn HttpClient,
) -> Result<String, SyncError> {
    let gitee_config = &config.gitee_config;

    if let Some(ref id) = gitee_config.gist_id {
        // Verify it still exists
        let gitee_client = GiteeClient::new(gitee_config.clone());
        match gitee_client.get_gist(id) {
            Ok(_) => return Ok(id.clone()),
            Err(_) => {}, // Will create new below
        }
    }

    // Create new gist
    let mut files = HashMap::new();
    files.insert("tree.json".into(), GistFileCreate::new(Some("[]")));

    let create = GistCreate {
        description: Some(&gitee_config.gist_description),
        files,
        public: false,
    };

    let gitee_client = GiteeClient::new(gitee_config.clone());
    let gist = gitee_client
        .create_gist(&create)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    Ok(gist.id)
}
```

Update `SnippetManager::do_push` to save the new `gist_id` when first push creates it:

```rust
    pub fn do_push(&self) -> Result<(), String> {
        let sync_config = self
            .config
            .sync
            .as_ref()
            .ok_or_else(|| "未配置同步".to_string())?;

        // If no gist_id, ensure one exists before pushing
        if sync_config.gitee_config.gist_id.is_none() && matches!(sync_config.provider, SyncProvider::Gitee) {
            let new_id = sync::get_or_create_gist_id(sync_config, &self.client)
                .map_err(|e| format!("Failed to create gist: {}", e))?;
            // Save to config
            if let Some(ref mut sync) = self.config.sync {
                sync.gitee_config.gist_id = Some(new_id);
            }
            let _ = self.config.save();
        }

        // ... rest of do_push as above
    }
```

**Step 4: Run tests to verify they pass**

```bash
cargo test --lib sync::tests::test_push_to_single_gist_creates_gist -- --nocapture
cargo test --lib -- --nocapture
```
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/snippet/mod.rs src/snippet/sync.rs
git commit -m "feat(manager): wire do_push/do_pull to single-gist sync + save gist_id"
```

---

### Task 6: Remove Deprecated `push_gitee` and `pull_gitee`

**Files:**
- Modify: `src/snippet/sync.rs` (remove `push_gitee` and `pull_gitee` functions)

**Step 1: Write a compile-fail test (verify no dead code)**

No new test needed — verify the codebase compiles cleanly.

**Step 2: Verify old code is unreachable**

```bash
cargo build
```
Expected: SUCCESS (no references to old functions remain)

**Step 3: Remove deprecated functions**

Delete these from `sync.rs`:
- `fn pull_gitee(config: &SyncConfig) -> Result<Vec<Snippet>, SyncError>` (lines 175–242)
- `fn push_gitee(config: &SyncConfig, snippets: &[Snippet]) -> Result<Vec<String>, SyncError>` (lines 360–471)

Remove these helper functions that are no longer used:
- `fn generate_gist_folder_id(gist_id: &str) -> String` (line 156)
- `pub fn generate_file_id_from_path(relative_path: &str) -> String` (line 160)

Remove `use crate::snippet::gitee::types::{GistFileCreate, GistUpdate};` if only used in old functions — keep if used in new functions.

**Step 4: Verify build succeeds**

```bash
cargo build
cargo test --lib -- --nocapture
```
Expected: Build SUCCESS, all tests PASS

**Step 5: Commit**

```bash
git add src/snippet/sync.rs
git commit -m "chore(sync): remove deprecated push_gitee/pull_gitee (one-gist-per-folder)"
```

## Commit Strategy

Each task produces one atomic commit. Commit messages follow the pattern:

```
feat(gitee): add GiteeClient::get_gist(id) for direct gist lookup
feat(sync): add TreeEntry serialization + push_to_single_gist
feat(sync): add pull_from_single_gist with tree.json parsing
refactor(store): simplify load_from_sync — match by id only
feat(manager): wire do_push/do_pull to single-gist sync + save gist_id
chore(sync): remove deprecated push_gitee/pull_gitee (one-gist-per-folder)
```

Atomicity rules:
- One logical change per commit
- Never mix new feature code with old code removal (separate into Task 5 vs Task 6)
- Tests committed alongside implementation
- No empty commits, no WIP commits

## Success Criteria

1. `cargo build` succeeds on Linux target
2. `cargo test --lib` passes with no regressions (all existing tests green)
3. Push creates/updates a single gist with `tree.json` + `{id}_{name}.txt` content files
4. Pull reads gist by stored `gist_id`, reconstructs tree + content, merges via LWW
5. `gist_id` persisted to `config.yaml` after first successful push
6. `load_from_sync` matches only by `id` (no `gitee_gist_id` matching)
7. No references to `push_gitee` or `pull_gitee` remain in the codebase

## TODO List (ADD THESE)

> CALLER: Add these TODOs using Todowrite/TaskCreate and execute by wave.

### Wave 1 (Start Immediately — No Dependencies)

- [ ] **Task 1: GiteeClient::get_gist(id) + unit test**
  - What: Add `get_gist()` method to `GiteeClient` (GET /gists/{id}); add URL construction tests
  - Depends: None
  - Blocks: Task 2, Task 3
  - Category: `quick` — single method + test in one file
  - Skills: `[]`
  - QA: `cargo test --lib gitee::client::tests -- --nocapture` → PASS

### Wave 2 (After Wave 1 — Tasks 2 & 3 run in parallel)

- [ ] **Task 2: TreeEntry serialization + push_to_single_gist**
  - What: Add `TreeEntry` struct with serde, `build_tree_entries()` helper, `push_to_single_gist()` (create/update gist with tree.json + content files), replace `push_gitee` call in public `push()`
  - Depends: Task 1
  - Blocks: Task 5
  - Category: `unspecified-high` — involves serialization, HashMap building, multi-step HTTP flow
  - Skills: `[]`
  - QA: `cargo test --lib sync::tests::test_tree_entry -- --nocapture` + `cargo test --lib sync::tests::test_build_tree_entries` → PASS

- [ ] **Task 3: pull_from_single_gist with tree.json parsing**
  - What: Add `pull_from_single_gist()` (GET gist by id → parse tree.json → read content files → build Vec<Snippet>), replace `pull_gitee` call in public `pull()`
  - Depends: Task 1
  - Blocks: Task 4, Task 5
  - Category: `unspecified-high` — involves JSON parsing, HashMap lookups, error handling
  - Skills: `[]`
  - QA: `cargo test --lib sync::tests::test_pull_from_single_gist -- --nocapture` → PASS

### Wave 3 (After Wave 2 — Tasks 4 & 5 in parallel)

- [ ] **Task 4: Simplify load_from_sync — remove gitee_gist_id matching**
  - What: Rewrite `load_from_sync` to match only by `id` (LWW via `update_date`); remove deprecated `gitee_gist_id` first-match block
  - Depends: Task 3
  - Blocks: Task 6
  - Category: `quick` — single function simplification + test
  - Skills: `[]`
  - QA: `cargo test --lib store::tests::test_load_from_sync_no_gist_id_matching -- --nocapture` → PASS

- [ ] **Task 5: SnippetManager wiring + gist_id persistence**
  - What: Update `do_push` to call `push_to_single_gist`, mark pending as synced, save new `gist_id` to config; Update `do_pull` to call `pull_from_single_gist`; update `schedule_push` and `start_background_sync` similarly
  - Depends: Task 2, Task 3
  - Blocks: Task 6
  - Category: `unspecified-high` — touches multiple methods in `mod.rs`, config save flow
  - Skills: `[]`
  - QA: `cargo test --lib -- --nocapture` → all PASS; `cargo build` → SUCCESS

### Wave 4 (After Wave 3)

- [ ] **Task 6: Remove deprecated push_gitee/pull_gitee**
  - What: Delete `push_gitee`, `pull_gitee`, `generate_gist_folder_id`, `generate_file_id_from_path` from `sync.rs`; clean up unused imports/types
  - Depends: Task 5
  - Blocks: None
  - Category: `quick` — deletion only, no new logic
  - Skills: `[]`
  - QA: `cargo build && cargo test --lib` → SUCCESS, no dead code warnings

## Execution Instructions

1. **Wave 1**: Fire Task 1 immediately
   ```
   task(category="quick", load_skills=[], run_in_background=false,
        prompt="Task 1: Add GiteeClient::get_gist(id) + unit test.
                File: src/snippet/gitee/client.rs
                Add get_gist method + URL tests.
                Run: cargo test --lib gitee::client::tests -- --nocapture")
   ```

2. **Wave 2**: After Task 1 completes, fire Tasks 2 & 3 IN PARALLEL
   ```
   task(category="unspecified-high", load_skills=[], run_in_background=false,
        prompt="Task 2: TreeEntry + push_to_single_gist.
                File: src/snippet/sync.rs
                Add TreeEntry struct, build_tree_entries, push_to_single_gist, replace push_gitee call.
                Run: cargo test --lib sync::tests::test_tree_entry -- --nocapture")

   task(category="unspecified-high", load_skills=[], run_in_background=false,
        prompt="Task 3: pull_from_single_gist.
                File: src/snippet/sync.rs
                Add pull_from_single_gist, replace pull_gitee call in public pull().
                Run: cargo test --lib sync::tests::test_pull_from_single_gist -- --nocapture")
   ```

3. **Wave 3**: After Wave 2 completes, fire Tasks 4 & 5 IN PARALLEL
   ```
   task(category="quick", load_skills=[], run_in_background=false,
        prompt="Task 4: Simplify load_from_sync.
                File: src/snippet/store.rs
                Remove gitee_gist_id matching block, keep id-only LWW.
                Run: cargo test --lib store::tests::test_load_from_sync_no_gist_id_matching -- --nocapture")

   task(category="unspecified-high", load_skills=[], run_in_background=false,
        prompt="Task 5: SnippetManager wiring.
                File: src/snippet/mod.rs + src/snippet/sync.rs
                Update do_push/do_pull/schedule_push/start_background_sync.
                Add get_or_create_gist_id helper.
                Run: cargo test --lib -- --nocapture")
   ```

4. **Wave 4**: After Wave 3, fire Task 6
   ```
   task(category="quick", load_skills=[], run_in_background=false,
        prompt="Task 6: Remove deprecated functions.
                File: src/snippet/sync.rs
                Delete push_gitee, pull_gitee, generate_gist_folder_id, generate_file_id_from_path.
                Run: cargo build && cargo test --lib")
   ```

5. **Final QA**: Run full test suite
   ```bash
   cargo test --lib -- --nocapture
   cargo build --release
   ```
