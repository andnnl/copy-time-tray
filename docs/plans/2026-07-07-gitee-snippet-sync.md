# Gitee Snippet Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace Git-based snippet sync with bidirectional Gitee Codes (gist) API, add multi-level folder support, tray status indicators, and Gitee config UI.

**Architecture:**
1. Create `src/snippet/gitee.rs` - pure HTTP client for Gitee gist API (reqwest + serde), no GTK/ui dependency.
2. Rewrite `src/snippet/sync.rs` - replace Git tree/contents push/pull with gist CRUD; fold existing tests in.
3. Thread `gitee_gist_id` field and `GiteeConfig` struct through `model.rs`/`config.rs`/`store.rs` with `#[serde(default)]` for backward compat.
4. Rebuild `refresh_tree_store()` in `main.rs` recursively; add `_default` virtual grouping; add tray status icon and last-sync status bar.
5. Extend `config_dialog.rs` with Gitee fields and "Test Connection" button.

**Tech Stack:** Rust 2021, GTK 0.18 (Linux), muda 0.19 + tray-icon 0.24 (Windows), reqwest 0.12, serde 1, notify-rust 4, thiserror 2.

---

## Context

User wants bidirectional sync between local snippet store and **Gitee Codes** (`gitee.com/dashboard/codes`), replacing the existing GitHub/Gitee **Git repository** sync. Key design decisions from `.sisyphus/plans/gitee-snippet-sync-design.md`:

| Decision | Rationale |
|----------|-----------|
| Gitee gist API (`/api/v5/gists`) | First-class multi-file groups matching folders |
| `gitee_gist_id` on Folder only | Maps each local folder → one gist |
| Timestamp-first conflict resolution | `update_date > remote.updated_at` → keep local |
| 2-second debounce after local save | Prevents push storms during batch edits |
| 3 retries with exponential backoff | Handles transient network failures |
| `_default` virtual group for root snippets | Root-level snippets have no parent; group into `_default` gist |
| Backward compat via `#[serde(default)]` | Old `snippets.json` without `gitee_gist_id` still loads |

**Current codebase analysis:**
- `model.rs` (183 lines): `Snippet` struct with `gitee_gist_id` field **not yet present**; `SnippetType::Folder` variant **already exists**; `new_snippet`/`new_folder` constructors present.
- `config.rs` (200 lines): `SyncConfig` has `provider`/`owner`/`repo`/`branch`/`token`; `GiteeConfig` **not yet present**.
- `sync.rs` (605 lines): Git-based sync using `GET /git/trees`, `PUT /contents`; `RealHttpClient`, `pull`, `push`, `base_url`, `auth_headers` present; 14 unit tests.
- `store.rs` (318 lines): `SnippetStore` with CRUD + `pending_sync` + `load_from_sync`; 9 unit tests.
- `mod.rs` (136 lines): `SnippetManager` with `do_pull`/`do_push`/`start_background_sync`.
- `main.rs` (1626 lines): GTK `launch_snippet_dialog_linux()` with flat `refresh_tree_store()`; drag-and-drop moves snippets between folders; Windows `muda` tray.
- `config_dialog.rs` (154 lines): Simple GTK form (provider, owner, repo, branch, token).
- `ui/mod.rs` + `ui/editor.rs` + `ui/search.rs`: Windows egui search/editor.
- `Cargo.toml`: No `env_logger` or `tracing` yet; `log = "0.4"` present; `notify-rust = "4"` present; `thiserror = "2"` present.

**Notes from user constraints:**
- Assume **no test framework** beyond what already exists (skip writing new tests unless explicitly needed).
- **Each step is one action** (2-5 minutes) with complete code examples.
- After each logical unit: run `cargo build` to verify; commit atomically.

---

## Task Dependency Graph

| Task | Depends On | Reason |
|------|------------|--------|
| Task 1: Add `gitee_gist_id` field | None | Foundation: other fields reference this |
| Task 2: Add `GiteeConfig` struct | None | Independent of model change |
| Task 3: Create `gitee.rs` module | Task 2 | Uses `GiteeConfig` for endpoint URL |
| Task 4: Add Gitee HTTP methods | Task 3 | Must have module + config first |
| Task 5: Add Gitee serde structs | Task 3 | Must have module first |
| Task 6: Update `SyncConfig` | Task 2 | Must have `GiteeConfig` type |
| Task 7: Update `Config` loading | Task 6 | Must have updated `SyncConfig` |
| Task 8: Rewrite `sync.rs` imports | Task 1, Task 3, Task 4, Task 5 | Needs model field + gitee client |
| Task 9: Rewrite `sync.rs` gist push | Task 8 | Needs imports + gitee client |
| Task 10: Rewrite `sync.rs` gist pull | Task 9 | Push must work before pull (shared conflict logic) |
| Task 11: Rewrite `sync.rs` retry + debounce | Task 9 | Needs push function |
| Task 12: Update `store.rs` load_from_sync | Task 1, Task 10 | Needs `gitee_gist_id` + gist pull |
| Task 13: Update `mod.rs` manager | Task 11, Task 12 | Needs retry + store sync |
| Task 14: Add `_default` group logic | Task 10 | Needs pull to return folders |
| Task 15: Recursive `refresh_tree_store` | Task 14 | Needs `_default` group |
| Task 16: Add `_default` grouping UI | Task 15 | Needs recursive tree |
| Task 17: Add tray status icon | None | Independent visual change |
| Task 18: Add last-sync status bar | Task 13 | Needs manager sync state |
| Task 19: Extend `config_dialog.rs` | Task 6 | Needs `GiteeConfig` type |
| Task 20: Add "Test Connection" button | Task 3, Task 19 | Needs gitee client + dialog |
| Task 21: Add sync notifications | Task 13 | Needs manager sync state |
| Task 22: Add error logging | Task 3 | Needs gitee client errors |
| Task 23: Add rate-limit backoff | Task 4 | Needs HTTP error handling |
| Task 24: Full build + test | All | Final verification |

---

## Parallel Execution Graph

```
Wave 1 (Start immediately - No dependencies):
├── Task 1:  Add `gitee_gist_id` field to Snippet struct
├── Task 2:  Add `GiteeConfig` struct to config.rs
└── Task 17: Add tray status icon (visual indicator)

Wave 2 (After Wave 1 completes):
├── Task 3:  Create `gitee.rs` module
├── Task 5:  Add Gitee serde structs in gitee.rs
└── Task 18: Add last-sync status bar (stub, depends on manager)

Wave 3 (After Wave 2 completes):
├── Task 4:  Add Gitee HTTP methods in gitee.rs
├── Task 6:  Update SyncConfig with GiteeConfig
└── Task 16: Add `_default` grouping UI (stub)

Wave 4 (After Wave 3 completes):
├── Task 7:  Update Config loading (gitee_config optional field)
├── Task 8:  Rewrite sync.rs imports
└── Task 19: Extend config_dialog.rs (Gitee fields)

Wave 5 (After Wave 4 completes):
├── Task 9:  Rewrite sync.rs gist push
├── Task 14: Add `_default` group logic
└── Task 20: Add "Test Connection" button

Wave 6 (After Wave 5 completes):
├── Task 10: Rewrite sync.rs gist pull
├── Task 15: Recursive refresh_tree_store
└── Task 21: Add sync notifications

Wave 7 (After Wave 6 completes):
├── Task 11: Rewrite sync.rs retry + debounce
├── Task 12: Update store.rs load_from_sync
├── Task 13: Update mod.rs manager
└── Task 22: Add error logging

Wave 8 (After Wave 7 completes):
├── Task 23: Add rate-limit backoff
└── Task 24: Full build + test
```

Critical Path: Task 1 → Task 3 → Task 4 → Task 8 → Task 9 → Task 10 → Task 15 → Task 24

Estimated Parallel Speedup: 60% faster than sequential

---

## Tasks

### Task 1: Add `gitee_gist_id` field to `Snippet` struct

**Files:**
- Modify: `src/snippet/model.rs:12-25`

**Step 1: Edit model.rs to add the field**

Edit `src/snippet/model.rs`, add `gitee_gist_id` field to the `Snippet` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gitee_gist_id: Option<String>,
}
```

**Step 2: Update `new_snippet` and `new_folder` constructors**

Update `new_snippet` (lines 28-42):

```rust
pub fn new_snippet(name: String, content: String, parent_id: Option<String>) -> Self {
    let now = Utc::now();
    Self {
        id: Uuid::new_v4().to_string(),
        parent_id,
        name,
        snippet_type: SnippetType::Snippet,
        content: Some(content),
        sort: 0,
        deleted: false,
        synced: false,
        create_date: now,
        update_date: now,
        gitee_gist_id: None,
    }
}
```

Update `new_folder` (lines 44-58):

```rust
pub fn new_folder(name: String, parent_id: Option<String>) -> Self {
    let now = Utc::now();
    Self {
        id: Uuid::new_v4().to_string(),
        parent_id,
        name,
        snippet_type: SnippetType::Folder,
        content: None,
        sort: 0,
        deleted: false,
        synced: false,
        create_date: now,
        update_date: now,
        gitee_gist_id: None,
    }
}
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task1_build.log`
Expected: exit code 0, no `gitee_gist_id` errors

**Step 4: Run existing tests**

Run: `cargo test 2>&1 | tee /tmp/task1_test.log`
Expected: All existing model tests pass (test_new_snippet_has_correct_type, test_new_folder_has_no_content, test_remote_id_is_deterministic, test_remote_id_differs_by_path, test_snippet_serialize_deserialize, test_folder_serialize_has_null_content, test_snippet_type_serde_tag, test_deserialize_from_json_string, test_deserialize_folder_from_json)

**Step 5: Commit**

```bash
git add src/snippet/model.rs
git commit -m "feat: add gitee_gist_id field to Snippet struct

- Add gitee_gist_id: Option<String> with serde(default) for backward compat
- Update new_snippet and new_folder constructors to set None"
```

---

### Task 2: Add `GiteeConfig` struct to `config.rs`

**Files:**
- Modify: `src/snippet/config.rs`

**Step 1: Add GiteeConfig struct and update SyncConfig**

Edit `src/snippet/config.rs`, add `GiteeConfig` after `SyncConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeConfig {
    pub api_base: String,
    pub token: String,
    pub auto_sync: bool,
    #[serde(default = "default_sync_interval")]
    pub sync_interval: u64,
}

fn default_sync_interval() -> u64 {
    300
}

impl Default for GiteeConfig {
    fn default() -> Self {
        Self {
            api_base: "https://gitee.com/api/v5".into(),
            token: String::new(),
            auto_sync: true,
            sync_interval: 300,
        }
    }
}
```

Update `SyncConfig` to include `gitee_config`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub provider: SyncProvider,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
    #[serde(default)]
    pub gitee_config: GiteeConfig,
}
```

**Step 2: Verify all existing config tests still pass**

Run: `cargo test --lib snippet::config 2>&1 | tee /tmp/task2_test.log`
Expected: All 7 config tests pass

**Step 3: Commit**

```bash
git add src/snippet/config.rs
git commit -m "feat: add GiteeConfig struct and gitee_config field

- GiteeConfig: api_base, token, auto_sync, sync_interval
- SyncConfig gains gitee_config: GiteeConfig with default"
```

---

### Task 3: Create `src/snippet/gitee.rs` module

**Files:**
- Create: `src/snippet/gitee.rs`
- Create: `src/snippet/gitee/error.rs`
- Create: `src/snippet/gitee/types.rs`
- Create: `src/snippet/gitee/client.rs`
- Modify: `src/snippet/mod.rs`

**Step 1: Create `src/snippet/gitee.rs` module file**

Create `src/snippet/gitee.rs`:

```rust
pub mod error;
pub mod client;
pub mod types;

pub use client::GiteeClient;
pub use error::{GiteeError, Result};
pub use types::{Gist, GistFile, GistCreate, GistUpdate};
```

**Step 2: Create `src/snippet/gitee/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GiteeError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Rate limited (429), retry after {0}s")]
    RateLimited(u64),

    #[error("Authentication failed ({0}): check your token")]
    Auth(u16),
}

pub type Result<T> = std::result::Result<T, GiteeError>;
```

**Step 3: Create `src/snippet/gitee/types.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Gist {
    pub id: String,
    pub description: Option<String>,
    pub files: HashMap<String, GistFile>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GistFile {
    pub filename: Option<String>,
    pub content: Option<String>,
    pub raw_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistCreate<'a> {
    pub description: Option<&'a str>,
    pub files: HashMap<String, GistFileCreate<'a>>,
    pub public: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistFileCreate<'a> {
    pub content: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistUpdate<'a> {
    pub description: Option<&'a str>,
    pub files: HashMap<String, GistFileUpdate<'a>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistFileUpdate<'a> {
    pub content: Option<&'a str>,
    pub filename: Option<&'a str>,
}

impl<'a> GistFileCreate<'a> {
    pub fn new(content: Option<&'a str>) -> Self {
        Self { content }
    }
}

impl<'a> GistUpdate<'a> {
    pub fn new(description: Option<&'a str>) -> Self {
        Self {
            description,
            files: HashMap::new(),
        }
    }

    pub fn add_file(mut self, name: &'a str, content: &'a str) -> Self {
        self.files.insert(
            name.into(),
            GistFileUpdate {
                content: Some(content),
                filename: Some(name),
            },
        );
        self
    }
}
```

**Step 4: Create `src/snippet/gitee/client.rs`**

```rust
use crate::snippet::config::GiteeConfig;
use crate::snippet::gitee::types::{Gist, GistCreate, GistUpdate};
use crate::snippet::gitee::{error, Result};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GiteeClient {
    config: GiteeConfig,
    client: reqwest::blocking::Client,
}

impl GiteeClient {
    pub fn new(config: GiteeConfig) -> Self {
        Self {
            config,
            client: reqwest::blocking::Client::builder()
                .user_agent("copy-time-tray/0.3")
                .build()
                .expect("reqwest client"),
        }
    }

    fn api_base(&self) -> &str {
        &self.config.api_base
    }

    fn auth_header(&self) -> (String, String) {
        ("access_token".into(), self.config.token.clone())
    }

    fn url_gists(&self) -> String {
        format!("{}/gists", self.api_base())
    }

    fn url_gist(&self, id: &str) -> String {
        format!("{}/gists/{}", self.api_base(), id)
    }

    pub fn list_gists(&self) -> Result<Vec<Gist>> {
        let (k, v) = self.auth_header();
        let resp = self
            .client
            .get(self.url_gists())
            .header(&k, &v)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        let status = resp.status();
        if status == 401 || status == 403 {
            return Err(error::GiteeError::Auth(status.as_u16()));
        }
        if status == 429 {
            let retry_after = resp
                .headers()
                .get("X-RateLimit-Reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(error::GiteeError::RateLimited(retry_after));
        }
        if !status.is_success() {
            return Err(error::GiteeError::Http(format!(
                "list_gists failed: {}",
                status
            )));
        }

        resp.json::<Vec<Gist>>()
            .map_err(|e| error::GiteeError::Parse(e.to_string()))
    }

    pub fn create_gist(&self, gist: &GistCreate) -> Result<Gist> {
        let (k, v) = self.auth_header();
        let resp = self
            .client
            .post(self.url_gists())
            .header(&k, &v)
            .header("Accept", "application/json")
            .json(gist)
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        self.check_status(&resp)?.json::<Gist>().map_err(|e| {
            error::GiteeError::Parse(e.to_string())
        })
    }

    pub fn update_gist(&self, id: &str, gist: &GistUpdate) -> Result<Gist> {
        let (k, v) = self.auth_header();
        let resp = self
            .client
            .patch(self.url_gist(id))
            .header(&k, &v)
            .header("Accept", "application/json")
            .json(gist)
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        self.check_status(&resp)?
            .json::<Gist>()
            .map_err(|e| error::GiteeError::Parse(e.to_string()))
    }

    pub fn delete_gist(&self, id: &str) -> Result<()> {
        let (k, v) = self.auth_header();
        let resp = self
            .client
            .delete(self.url_gist(id))
            .header(&k, &v)
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        self.check_status(&resp).map(|_| ())
    }

    fn check_status(&self, resp: &reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        let status = resp.status();
        if status == 401 || status == 403 {
            return Err(error::GiteeError::Auth(status.as_u16()));
        }
        if status == 429 {
            let retry_after = resp
                .headers()
                .get("X-RateLimit-Reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(error::GiteeError::RateLimited(retry_after));
        }
        if status.is_success() {
            Ok(resp.clone())
        } else {
            Err(error::GiteeError::Http(format!(
                "HTTP {}",
                status
            )))
        }
    }
}
```

**Step 5: Declare module in mod.rs**

Edit `src/snippet/mod.rs`:

```rust
pub mod config;
pub mod gitee;
pub mod model;
pub mod store;
pub mod sync;
pub mod ui;
```

**Step 6: Run build**

Run: `cargo build 2>&1 | tee /tmp/task3_build.log`
Expected: exit code 0

**Step 7: Commit**

```bash
git add src/snippet/gitee.rs src/snippet/gitee/error.rs src/snippet/gitee/types.rs src/snippet/gitee/client.rs src/snippet/mod.rs
git commit -m "feat: add gitee.rs module with client, types, error

- gitee/client.rs: GiteeClient with list/create/update/delete gist
- gitee/types.rs: Gist, GistFile, GistCreate, GistUpdate serde structs
- gitee/error.rs: GiteeError with Http/Parse/Auth/RateLimited variants
- mod.rs: declare gitee submodule"
```

---

### Task 4: Wire `GiteeConfig` into `SyncConfig` and fix construction sites

**Files:**
- Modify: `src/snippet/config.rs`
- Modify: `src/main.rs`

**Step 1: Add `gitee_config` to `SyncConfig`** (covered in Task 2, verify here)

Verify `SyncConfig` in `src/snippet/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub provider: SyncProvider,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
    #[serde(default)]
    pub gitee_config: GiteeConfig,
}
```

**Step 2: Fix all `SyncConfig { ... }` construction sites**

Find all construction sites:

Run: `grep -rn "SyncConfig {" src/`
Expected: at least one in `src/main.rs` (line ~208).

Update `src/main.rs:207-215`:

```rust
let config = crate::snippet::config::Config {
    sync: Some(crate::snippet::config::SyncConfig {
        provider: config_result.provider,
        owner: config_result.owner,
        repo: config_result.repo,
        branch: config_result.branch,
        token: config_result.token,
        gitee_config: crate::snippet::config::GiteeConfig::default(),
    }),
};
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task4_build.log`
Expected: exit code 0 (no missing field errors)

**Step 4: Commit**

```bash
git add src/main.rs src/snippet/config.rs
git commit -m "fix: wire GiteeConfig into SyncConfig and fix construction sites

- SyncConfig gains gitee_config: GiteeConfig with serde(default)
- main.rs config-dialog path: use GiteeConfig::default()"
```

---

### Task 5: Rewrite `sync.rs` - replace Git API with Gitee gist API

**Files:**
- Modify: `src/snippet/sync.rs`

**Context:** Current `sync.rs` (605 lines) implements Git tree/contents-based sync. Design requires gist API. Strategy: keep `HttpClient` trait + `RealHttpClient`, rewrite `pull` and `push` to use `GiteeClient`.

**Step 1: Update imports in sync.rs**

Replace top of `src/snippet/sync.rs`:

```rust
use std::collections::HashMap;

use crate::snippet::config::{GiteeConfig, SyncConfig, SyncProvider};
use crate::snippet::gitee::client::GiteeClient;
use crate::snippet::gitee::types::{Gist, GistFile};
use crate::snippet::model::{Snippet, SnippetType};
use crate::snippet::gitee::error::Result as GiteeResult;
```

**Step 2: Rewrite `pull` function (lines 154-202)**

```rust
pub fn pull(config: &SyncConfig, client: &dyn HttpClient) -> Result<Vec<Snippet>, SyncError> {
    let gitee_config = GiteeConfig {
        api_base: match config.provider {
            SyncProvider::Github => "https://api.github.com".into(),
            SyncProvider::Gitee => config.gitee_config.api_base.clone(),
        },
        token: config.token.clone(),
        auto_sync: config.gitee_config.auto_sync,
        sync_interval: config.gitee_config.sync_interval,
    };
    let gitee_client = GiteeClient::new(gitee_config);

    let remote_gists = gitee_client.list_gists().map_err(|e| {
        SyncError::Http(format!("list_gists failed: {}", e))
    })?;

    let mut snippets = Vec::new();
    for gist in remote_gists {
        let folder_name = gist.description.clone().unwrap_or_else(|| "_default".into());
        let folder_id = generate_gist_folder_id(&gist.id);

        let mut child_snippets = Vec::new();
        for (filename, gist_file) in gist.files {
            if let Some(content) = gist_file.content {
                let snippet = Snippet {
                    id: generate_file_id_from_path(&format!("{}/{}", folder_name, filename)),
                    parent_id: Some(folder_id.clone()),
                    name: filename.strip_suffix(".sh").unwrap_or(&filename).into(),
                    snippet_type: SnippetType::Snippet,
                    content: Some(content),
                    sort: 0,
                    deleted: false,
                    synced: true,
                    create_date: gist.created_at,
                    update_date: gist.updated_at,
                    gitee_gist_id: Some(gist.id.clone()),
                };
                child_snippets.push(snippet);
            }
        }

        if !child_snippets.is_empty() {
            let folder = Snippet {
                id: folder_id,
                parent_id: None,
                name: folder_name,
                snippet_type: SnippetType::Folder,
                content: None,
                sort: 0,
                deleted: false,
                synced: true,
                create_date: gist.created_at,
                update_date: gist.updated_at,
                gitee_gist_id: Some(gist.id),
            };
            snippets.push(folder);
            snippets.extend(child_snippets);
        }
    }

    Ok(snippets)
}
```

**Step 3: Rewrite `push` function (lines 251-328)**

```rust
pub fn push(
    config: &SyncConfig,
    client: &dyn HttpClient,
    snippets: &[Snippet],
) -> Result<Vec<String>, SyncError> {
    let gitee_config = GiteeConfig {
        api_base: match config.provider {
            SyncProvider::Github => "https://api.github.com".into(),
            SyncProvider::Gitee => config.gitee_config.api_base.clone(),
        },
        token: config.token.clone(),
        auto_sync: config.gitee_config.auto_sync,
        sync_interval: config.gitee_config.sync_interval,
    };
    let gitee_client = GiteeClient::new(gitee_config);

    let folders: HashMap<Option<String>, Vec<&Snippet>> =
        snippets.iter().filter(|s| !s.deleted || s.snippet_type == SnippetType::Folder).fold(
            HashMap::new(),
            |mut acc, s| {
                let pid = s.parent_id.clone();
                acc.entry(pid).or_default().push(s);
                acc
            },
        );

    let mut failed_ids = Vec::new();

    for (parent_id, group) in folders {
        let folder_name = if parent_id.is_some() {
            match snippets.iter().find(|s| s.id == parent_id.as_ref().unwrap()) {
                Some(folder) => folder.name.clone(),
                None => "_default".into(),
            }
        } else {
            "_default".into()
        };

        let existing_gist_id = match &parent_id {
            Some(pid) => snippets.iter().find(|s| s.id == *pid).and_then(|s| s.gitee_gist_id.clone()),
            None => None,
        };

        let files: HashMap<String, GistFileCreate<'_>> = group
            .iter()
            .filter(|s| s.snippet_type == SnippetType::Snippet && s.content.is_some())
            .map(|s| {
                let filename = format!("{}.sh", s.name);
                (filename.clone(), GistFileCreate::new(s.content.as_deref()))
            })
            .collect();

        if files.is_empty() {
            if let Some(ref gist_id) = existing_gist_id {
                let _ = gitee_client.delete_gist(gist_id);
            }
            continue;
        }

        let result = if let Some(ref gist_id) = existing_gist_id {
            let update = GistUpdate::new(Some(&folder_name))
                .add_file("_group_", &format!("folder: {}", folder_name));
            gitee_client.update_gist(gist_id, &update)
        } else {
            let mut gist_files = HashMap::new();
            for (name, content) in &files {
                gist_files.insert(name.clone(), GistFileCreate::new(*content));
            }
            let create = GistCreate {
                description: Some(&folder_name),
                files: gist_files,
                public: false,
            };
            gitee_client.create_gist(&create)
        };

        match result {
            Ok(gist) => {
                if let Some(fid) = parent_id {
                    if let Some(snippet) = snippets.iter().find(|s| s.id == fid) {
                        let _ = client.put(
                            &format!("{}/gists/{}", config.repo, gist.id),
                            &HashMap::new(),
                            &gist.id,
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!("push failed for folder {}: {}", folder_name, e);
                failed_ids.extend(group.iter().map(|s| s.id.clone()));
            }
        }
    }

    if failed_ids.is_empty() {
        Ok(Vec::new())
    } else {
        Err(SyncError::Store(format!(
            "Failed to push {} snippets",
            failed_ids.len()
        )))
    }
}
```

**Step 4: Add `generate_gist_folder_id` helper**

Add after `generate_file_id` (line 242):

```rust
pub fn generate_gist_folder_id(gist_id: &str) -> String {
    format!("gist:{}", gist_id)
}
```

Add `generate_file_id_from_path` alongside `generate_file_id`:

```rust
pub fn generate_file_id_from_path(relative_path: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(relative_path.as_bytes());
    let result = hasher.finalize();
    let hex = format!("{:x}", result);
    hex[..16].to_string()
}
```

**Step 5: Update `snippet_to_path` to skip gist folders**

Update `snippet_to_path` (lines 330-333):

```rust
fn snippet_to_path(snippet: &Snippet) -> Result<String, SyncError> {
    if snippet.snippet_type == SnippetType::Folder {
        return Ok(String::new());
    }
    let safe_name = sanitize_filename(&snippet.name);
    Ok(format!("snippets/{}.yaml", safe_name))
}
```

**Step 6: Run build**

Run: `cargo build 2>&1 | tee /tmp/task5_build.log`
Expected: exit code 0

**Step 7: Run existing tests**

Run: `cargo test --lib snippet::sync 2>&1 | tee /tmp/task5_test.log`
Expected: All 14 existing sync tests pass (test_base_url_github, test_base_url_gitee, test_auth_headers_github, test_auth_headers_gitee, test_generate_file_id, test_pull_retrieves_snippets, test_pull_non_200_returns_error, test_parse_yaml_snippet, test_parse_invalid_yaml_fails, test_push_new_snippet, test_push_deleted_snippet, test_snippet_to_yaml_format, test_snippet_to_path, test_snippet_to_path_sanitizes)

**Step 8: Commit**

```bash
git add src/snippet/sync.rs
git commit -m "feat: rewrite sync.rs for Gitee gist API

- Replace Git tree/contents push/pull with gist CRUD
- pull: list gists, parse files into Snippet entries
- push: group snippets by folder, create/update gist per folder
- Add generate_gist_folder_id helper
- Skip gist folders in snippet_to_path"
```

---

### Task 6: Update `store.rs` - `load_from_sync` handles `gitee_gist_id`

**Files:**
- Modify: `src/snippet/store.rs:134-151`

**Step 1: Update `load_from_sync`**

Replace the body of `load_from_sync`:

```rust
pub fn load_from_sync(&mut self, remote_snippets: Vec<Snippet>) -> Result<(), StoreError> {
    use std::collections::HashMap;

    let mut by_id: HashMap<String, usize> = self.data.snippets.iter().enumerate()
        .map(|(i, s)| (s.id.clone(), i))
        .collect();

    for remote in remote_snippets {
        if let Some(gist_id) = &remote.gitee_gist_id {
            if let Some(existing) = self.data.snippets.iter_mut().find(|s| {
                s.gitee_gist_id.as_deref() == Some(gist_id)
            }) {
                if remote.update_date > existing.update_date {
                    *existing = remote;
                }
                continue;
            }
        }

        if let Some(idx) = by_id.get(&remote.id) {
            let existing = &mut self.data.snippets[*idx];
            if remote.update_date > existing.update_date {
                *existing = remote;
            }
        } else {
            self.data.snippets.push(remote);
        }
    }
    self.save()
}
```

**Step 2: Run build**

Run: `cargo build 2>&1 | tee /tmp/task6_build.log`
Expected: exit code 0

**Step 3: Run existing tests**

Run: `cargo test --lib snippet::store 2>&1 | tee /tmp/task6_test.log`
Expected: All 9 store tests pass

**Step 4: Commit**

```bash
git add src/snippet/store.rs
git commit -m "feat: update load_from_sync to match gitee_gist_id

- Match remote snippets by gitee_gist_id (for folders)
- Fallback to UUID match for root snippets
- LWW conflict resolution preserved"
```

---

### Task 7: Update `mod.rs` - add debounce and retry to `SnippetManager`

**Files:**
- Modify: `src/snippet/mod.rs`

**Step 1: Update `do_push` with retry logic**

Replace `do_push` in `src/snippet/mod.rs:94-109`:

```rust
const MAX_RETRIES: usize = 3;
const RETRY_DELAY_MS: u64 = 1000;

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

    let mut last_error = None;
    for attempt in 0..MAX_RETRIES {
        match sync::push(sync_config, &self.client, &pending) {
            Ok(_) => return Ok(()),
            Err(e) => {
                log::warn!("push attempt {} failed: {}", attempt + 1, e);
                last_error = Some(e);
                if attempt < MAX_RETRIES - 1 {
                    std::thread::sleep(std::time::Duration::from_millis(
                        RETRY_DELAY_MS * 2u64.pow(attempt as u32),
                    ));
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "push failed after retries".into()).to_string())
}
```

**Step 2: Add `push_on_change` method**

Add after `do_push`:

```rust
pub fn push_on_change(&self) {
    let sync_config = match &self.config.sync {
        Some(c) => c.clone(),
        None => return,
    };
    let client = self.client.clone();

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let _ = push_internal(&sync_config, &client);
    });
}

fn push_internal(
    sync_config: &SyncConfig,
    client: &sync::RealHttpClient,
) -> Result<(), String> {
    let store_path = Config::config_dir()
        .expect("config_dir")
        .join("snippets.json");
    if let Ok(mut store) = SnippetStore::new(store_path) {
        let pending: Vec<Snippet> = store.pending_sync().into_iter().cloned().collect();
        if pending.is_empty() {
            return Ok(());
        }
        sync::push(sync_config, client, &pending).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task7_build.log`
Expected: exit code 0

**Step 4: Commit**

```bash
git add src/snippet/mod.rs
git commit -m "feat: add push retry and debounce to SnippetManager

- do_push: retry 3x with exponential backoff (1s/2s/4s)
- push_on_change: 2-second debounce after local changes
- push_internal: reload store for debounced push"
```

---

### Task 8: Add `_default` grouping and recursive `refresh_tree_store`

**Files:**
- Modify: `src/main.rs` (refresh_tree_store closure ~lines 957-1083, initial population ~lines 918-952)

**Step 1: Add `_default` group logic in `refresh_tree_store`**

After computing `folders_to_show`, add:

```rust
let default_snippets: Vec<_> = all_snippets
    .iter()
    .filter(|s| s.parent_id.is_none() || !folders.iter().any(|f| Some(&f.id) == s.parent_id.as_ref()))
    .filter(|s| {
        if q.is_some() {
            matching_snippet_ids.contains(&s.id)
        } else {
            true
        }
    })
    .collect();
```

**Step 2: Add recursive helper for nested folders**

Insert before the folder loop in `refresh_tree_store`:

```rust
fn insert_recursive(
    store: &gtk::TreeStore,
    folders: &[Snippet],
    all_snippets: &[Snippet],
    parent_folder_id: Option<&str>,
    parent_iter: Option<&gtk::TreeIter>,
    folders_to_show: &std::collections::HashSet<String>,
    q: &Option<String>,
    matching_snippet_ids: &std::collections::HashSet<String>,
) {
    let direct_children: Vec<_> = folders
        .iter()
        .filter(|s| s.parent_id.as_deref() == parent_folder_id)
        .filter(|f| folders_to_show.contains(&f.id))
        .collect();

    for folder in direct_children {
        let iter = store.insert_with_values(parent_iter, None, &[
            (0, &folder.id),
            (1, &folder.name),
            (2, &""),
            (3, &true),
            (4, &"folder-symbolic"),
        ]);
        for s in all_snippets.iter().filter(|s| s.parent_id.as_deref() == Some(&folder.id)) {
            let should_show = if q.is_some() {
                matching_snippet_ids.contains(&s.id)
            } else {
                true
            };
            if should_show {
                store.insert_with_values(Some(iter), None, &[
                    (0, &s.id),
                    (1, &s.name),
                    (2, &s.content.clone().unwrap_or_default()),
                    (3, &false),
                    (4, &"text-x-script-symbolic"),
                ]);
            }
        }
        insert_recursive(store, folders, all_snippets, Some(&folder.id), Some(iter), folders_to_show, q, matching_snippet_ids);
    }
}
```

Replace the flat folder+snippet loop (lines ~1015-1063) with:

```rust
for folder in &folders {
    if !folders_to_show.contains(&folder.id) {
        continue;
    }
    if folder.parent_id.is_some() {
        continue;
    }
    let parent_iter = store.insert_with_values(None, None, &[
        (0, &folder.id),
        (1, &folder.name),
        (2, &""),
        (3, &true),
        (4, &"folder-symbolic"),
    ]);
    for s in &all_snippets {
        if s.parent_id.as_deref() == Some(&folder.id) {
            let should_show = if q.is_some() {
                matching_snippet_ids.contains(&s.id)
            } else {
                true
            };
            if should_show {
                store.insert_with_values(Some(parent_iter), None, &[
                    (0, &s.id),
                    (1, &s.name),
                    (2, &s.content.clone().unwrap_or_default()),
                    (3, &false),
                    (4, &"text-x-script-symbolic"),
                ]);
            }
        }
    }
    insert_recursive(store, folders, all_snippets, Some(&folder.id), Some(parent_iter), folders_to_show, &q, &matching_snippet_ids);
}
```

**Step 3: Add `_default` rendering at end of `refresh_tree_store`**

After recursive folder rendering, before `tree_view.expand_all()`:

```rust
if !default_snippets.is_empty() {
    let default_iter = store.insert_with_values(None, None, &[
        (0, &"_default_group".into()),
        (1, &"_default"),
        (2, &""),
        (3, &true),
        (4, &"folder-symbolic"),
    ]);
    for s in &default_snippets {
        store.insert_with_values(Some(default_iter), None, &[
            (0, &s.id),
            (1, &s.name),
            (2, &s.content.clone().unwrap_or_default()),
            (3, &false),
            (4, &"text-x-script-symbolic"),
        ]);
    }
}
```

**Step 4: Update initial population (lines 918-952)**

Apply `_default` group to initial tree:

```rust
let default_snippets: Vec<_> = all_snippets
    .iter()
    .filter(|s| s.parent_id.is_none() || !folders.iter().any(|f| Some(&f.id) == s.parent_id.as_ref()))
    .collect();

for s in &default_snippets {
    store.insert_with_values(None, None, &[
        (0, &s.id),
        (1, &s.name),
        (2, &s.content.clone().unwrap_or_default()),
        (3, &false),
        (4, &"text-x-script-symbolic"),
    ]);
}
```

**Step 5: Run build**

Run: `cargo build 2>&1 | tee /tmp/task8_build.log`
Expected: exit code 0

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: recursive refresh_tree_store for multi-level folders

- Add insert_recursive helper for arbitrary nesting depth
- Group root-level snippets into _default virtual folder
- Apply _default logic to both initial tree and refresh"
```

---

### Task 9: Extend `config_dialog.rs` with Gitee fields

**Files:**
- Modify: `src/snippet/ui/config_dialog.rs`
- Modify: `src/main.rs` (config construction)

**Step 1: Update `ConfigDialogResult` struct**

```rust
#[derive(Debug, Clone)]
pub struct ConfigDialogResult {
    pub provider: SyncProvider,
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
    pub gitee_api_base: String,
    pub gitee_auto_sync: bool,
}
```

**Step 2: Add Gitee fields to dialog UI**

After the `token_box` block in `show_config_dialog`, add:

```rust
let api_base_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
let api_base_label = gtk::Label::new(Some("Gitee API Base:"));
api_base_label.set_xalign(0.0);
let api_base_entry = Entry::new();
api_base_entry.set_placeholder_text(Some("https://gitee.com/api/v5"));
api_base_entry.set_text("https://gitee.com/api/v5");
api_base_box.pack_start(&api_base_label, false, false, 0);
api_base_box.pack_start(&api_base_entry, true, true, 0);
content.pack_start(&api_base_box, false, false, 0);

let auto_sync_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
let auto_sync_label = gtk::Label::new(Some("Auto Sync:"));
auto_sync_label.set_xalign(0.0);
let auto_sync_switch = gtk::Switch::new();
auto_sync_switch.set_active(true);
auto_sync_box.pack_start(&auto_sync_label, false, false, 0);
auto_sync_box.pack_start(&auto_sync_switch, true, true, 0);
content.pack_start(&auto_sync_box, false, false, 0);
```

**Step 3: Include new fields in result**

Update the `Ok` branch:

```rust
Some(ConfigDialogResult {
    provider,
    owner,
    repo,
    branch,
    token,
    gitee_api_base: api_base_entry.text().to_string(),
    gitee_auto_sync: auto_sync_switch.is_active(),
})
```

**Step 4: Update `src/main.rs` config construction (line ~207-215)**

```rust
let config = crate::snippet::config::Config {
    sync: Some(crate::snippet::config::SyncConfig {
        provider: config_result.provider,
        owner: config_result.owner,
        repo: config_result.repo,
        branch: config_result.branch,
        token: config_result.token,
        gitee_config: crate::snippet::config::GiteeConfig {
            api_base: config_result.gitee_api_base,
            token: config_result.token.clone(),
            auto_sync: config_result.gitee_auto_sync,
            sync_interval: 300,
        },
    }),
};
```

**Step 5: Run build**

Run: `cargo build 2>&1 | tee /tmp/task9_build.log`
Expected: exit code 0

**Step 6: Commit**

```bash
git add src/snippet/ui/config_dialog.rs src/main.rs
git commit -m "feat: extend config dialog with Gitee API fields

- Add api_base and auto_sync to ConfigDialogResult
- Show Gitee API Base entry and Auto Sync switch
- Populate gitee_config from dialog result in main.rs"
```

---

### Task 10: Add "Test Connection" button to config dialog

**Files:**
- Modify: `src/snippet/ui/config_dialog.rs`

**Step 1: Add `test_connection` public function**

Add at end of `config_dialog.rs`:

```rust
pub fn test_connection(config: &crate::snippet::config::GiteeConfig) -> Result<(), String> {
    let client = crate::snippet::gitee::client::GiteeClient::new(config.clone());
    client.list_gists().map(|_| ()).map_err(|e| e.to_string())
}
```

**Step 2: Add button to dialog UI**

After `auto_sync_box`, add:

```rust
let test_button = gtk::Button::with_label("Test Connection");
test_button.style_context().add_class("suggested-action");
content.pack_start(&test_button, false, false, 0);

let test_status = gtk::Label::new(Some(""));
test_status.set_xalign(0.0);
content.pack_start(&test_status, false, false, 0);

{
    let api_base_entry = api_base_entry.clone();
    let token_entry = token_entry.clone();
    let auto_sync_switch = auto_sync_switch.clone();
    let test_status = test_status.clone();
    test_button.connect_clicked(move |_| {
        test_status.set_text("Testing...");
        let config = crate::snippet::config::GiteeConfig {
            api_base: api_base_entry.text().to_string(),
            token: token_entry.text().to_string(),
            auto_sync: auto_sync_switch.is_active(),
            sync_interval: 300,
        };
        match test_connection(&config) {
            Ok(_) => test_status.set_text("Connection OK"),
            Err(e) => test_status.set_text(&format!("Failed: {}", e)),
        }
    });
}
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task10_build.log`
Expected: exit code 0

**Step 4: Commit**

```bash
git add src/snippet/ui/config_dialog.rs
git commit -m "feat: add Test Connection button to config dialog

- test_connection function uses GiteeClient.list_gists
- Show OK/Failed status label below button
- Auto-populate config from dialog fields on click"
```

---

### Task 11: Add tray status icon and last-sync status bar

**Files:**
- Modify: `src/main.rs`

**Step 1: Add status icon management for Linux**

Near indicator creation (`src/main.rs:115-118`), add:

```rust
let mut sync_status: Option<String> = None;

fn set_indicator_icon(indicator: &AppIndicator, status: Option<&str>) {
    let icon = status.unwrap_or("copy-time");
    let path = format!("{}/icons/{}.svg", env!("CARGO_MANIFEST_DIR"), icon);
    let _ = indicator.set_icon_full(&path, "sync status");
}
```

In sync menu handler, before/after sync:

```rust
// Before sync
set_indicator_icon(&indicator, Some("sync-syncing"));

// After sync
match result {
    Ok(_) => set_indicator_icon(&indicator, Some("copy-time")),
    Err(_) => set_indicator_icon(&indicator, Some("sync-error")),
}
```

**Step 2: Add status bar to Linux snippet dialog**

In `launch_snippet_dialog_linux`, after `preview_scroll.add`:

```rust
let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
status_bar.set_margin_top(4);
status_bar.set_margin_bottom(4);
status_bar.set_margin_start(6);
let last_sync_label = gtk::Label::new(Some("上次同步: 从未"));
last_sync_label.set_xalign(0.0);
last_sync_label.style_context().add_class("dim-label");
status_bar.pack_start(&last_sync_label, true, true, 0);
right_box.pack_start(&status_bar, false, false, 0);
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task11_build.log`
Expected: exit code 0

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add tray status icon and last-sync status bar

- Linux: track sync-syncing/sync-error/copy-time icons
- Linux snippet dialog: show last-sync label in status bar"
```

---

### Task 12: Add sync notifications (success/partial/failure)

**Files:**
- Modify: `src/main.rs` (sync handler on both platforms)

**Step 1: Update Linux sync notification**

In the Linux sync handler (`src/main.rs:241-261`):

```rust
let sync_result = manager.do_pull().and_then(|_| manager.do_push());

glib::idle_add(move || {
    match sync_result {
        Ok(_) => {
            let _ = notify_rust::Notification::new()
                .summary("Snippet Sync")
                .body("已同步到 Gitee")
                .show();
        }
        Err(ref e) => {
            let _ = notify_rust::Notification::new()
                .summary("Snippet Sync Failed")
                .body(&format!("{}", e))
                .show();
        }
    }
    glib::ControlFlow::Break
});
```

**Step 2: Update Windows sync notification**

In Windows sync handler (`src/main.rs:413-427`):

```rust
let sync_result = manager.do_pull().and_then(|_| manager.do_push());
match sync_result {
    Ok(_) => {
        let _ = notify_rust::Notification::new()
            .summary("同步成功")
            .body("Snippets 已同步到 Gitee")
            .show();
    }
    Err(ref e) => {
        let _ = notify_rust::Notification::new()
            .summary("同步失败")
            .body(&format!("{}", e))
            .show();
    }
}
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task12_build.log`
Expected: exit code 0

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add granular sync notifications

- Linux: idle_add to show success/failure after async sync
- Windows: notify-rust with success/failure summary"
```

---

### Task 13: Add error logging

**Files:**
- Modify: `src/main.rs`
- Modify: `src/snippet/gitee/client.rs`

**Step 1: Add `log` crate initialization**

Add at top of `src/main.rs`:

```rust
fn init_logging() {
    if let Ok(dir) = crate::snippet::config::Config::config_dir() {
        let log_path = dir.join("sync.log");
        if let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = env_logger::Builder::new()
                .target(env_logger::Target::Pipe { writer: Box::new(file) })
                .filter_level(log::LevelFilter::Warn)
                .try_init();
        }
    }
}
```

**Step 2: Add `env_logger` dependency to Cargo.toml**

Add to `Cargo.toml`:

```toml
env_logger = "0.11"
```

**Step 3: Call `init_logging` in `main()`**

Before `SnippetManager::new()`:

```rust
init_logging();
```

**Step 4: Add `log::error!` in `gitee/client.rs`**

In each method:

```rust
log::error!("gitee list_gists failed: {}", e);
```

**Step 5: Run build**

Run: `cargo build 2>&1 | tee /tmp/task13_build.log`
Expected: exit code 0

**Step 6: Commit**

```bash
git add src/main.rs src/snippet/gitee/client.rs Cargo.toml
git commit -m "feat: add error logging via env_logger

- init_logging writes warnings to ~/.config/copy-time-tray/sync.log
- Add log::error in gitee client on HTTP failures"
```

---

### Task 14: Add rate-limit backoff

**Files:**
- Modify: `src/snippet/gitee/client.rs`

**Step 1: Add `handle_rate_limit` helper**

Add to `GiteeClient` impl:

```rust
fn handle_rate_limit(&self, resp: &reqwest::blocking::Response) -> Result<()> {
    let status = resp.status();
    if status == 429 {
        let retry_after = resp
            .headers()
            .get("X-RateLimit-Reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);
        log::warn!("rate limited, retry after {}s", retry_after);
        std::thread::sleep(std::time::Duration::from_secs(retry_after.min(300)));
    }
    Ok(())
}
```

**Step 2: Call `handle_rate_limit` before `check_status` in all methods**

In `list_gists`, `create_gist`, `update_gist`, `delete_gist`:

```rust
self.handle_rate_limit(&resp)?;
```

**Step 3: Run build**

Run: `cargo build 2>&1 | tee /tmp/task14_build.log`
Expected: exit code 0

**Step 4: Commit**

```bash
git add src/snippet/gitee/client.rs
git commit -m "fix: add rate-limit backoff in GiteeClient

- Read X-RateLimit-Reset header on 429 responses
- Sleep for retry_after seconds (capped at 300s)
- Log rate limit events"
```

---

### Task 15: Final build verification

**Files:**
- All files

**Step 1: Full cargo build**

Run: `cargo build --release 2>&1 | tee /tmp/task15_build.log`
Expected: exit code 0

**Step 2: Run all tests**

Run: `cargo test 2>&1 | tee /tmp/task15_test.log`
Expected: All tests pass:
- model.rs: 9 tests
- sync.rs: 14 tests
- config.rs: 7 tests
- store.rs: 9 tests

**Step 3: Lint (if `cargo clippy` available)**

Run: `cargo clippy --all-targets --all-features 2>&1 | tee /tmp/task15_clippy.log`
Expected: No errors

**Step 4: Commit (final)**

```bash
git add -A
git commit -m "feat: gitee snippet sync - final build verification

- All unit tests pass
- Release build succeeds
- Ready for integration testing"
```

---

## Commit Strategy

Each task above ends with an atomic commit covering only the files changed in that step. Commit messages follow conventional commits:

```
feat: <description>
fix: <description>
```

Branch: `feat/gitee-snippet-sync` (create at start, PR at end).

---

## Success Criteria

- [ ] `cargo build --release` exits 0
- [ ] `cargo test` exits 0 (all tests pass)
- [ ] `cargo clippy` reports no errors
- [ ] `gitee_gist_id` field serialized with `#[serde(default)]`
- [ ] `GiteeConfig` struct present with `api_base`/`token`/`auto_sync`/`sync_interval`
- [ ] `src/snippet/gitee.rs` with client/types/error modules present
- [ ] `sync.rs` push/pull use `GiteeClient` for gist CRUD
- [ ] `refresh_tree_store()` renders multi-level folders recursively
- [ ] `_default` virtual group shown for root-level snippets
- [ ] Config dialog shows Gitee API Base and Auto Sync fields
- [ ] Tray status icon tracks `copy-time`/`sync-syncing`/`sync-error`
- [ ] Last-sync label visible in snippet dialog status bar
- [ ] Notifications shown for sync success/failure
- [ ] Rate-limit 429 triggers backoff sleep
- [ ] `do_push` retries 3x with 1s/2s/4s backoff
- [ ] `push_on_change` debounces 2 seconds
