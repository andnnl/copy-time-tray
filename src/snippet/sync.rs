use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::snippet::config::{GiteeConfig, SyncConfig, SyncProvider};
use crate::snippet::gitee::types::{Gist, GistCreate, GistFileCreate, GistUpdate};
use crate::snippet::gitee::GiteeClient;
use crate::snippet::model::Snippet;

#[derive(Debug)]
pub enum SyncError {
    Http(String),
    Parse(String),
    Config(String),
    Store(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Http(e) => write!(f, "HTTP error: {}", e),
            SyncError::Parse(e) => write!(f, "Parse error: {}", e),
            SyncError::Config(e) => write!(f, "Config error: {}", e),
            SyncError::Store(e) => write!(f, "Store error: {}", e),
        }
    }
}

impl std::error::Error for SyncError {}

#[derive(Debug, Deserialize)]
pub struct GitTreeEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct GitTreeResponse {
    pub tree: Vec<GitTreeEntry>,
}

/// Entry in tree.json representing a snippet/folder in the tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: crate::snippet::model::SnippetType,
    pub sort: i32,
    pub deleted: bool,
    pub create_date: DateTime<Utc>,
    pub update_date: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Build tree entries from snippets for serialization to tree.json.
pub fn build_tree_entries(snippets: &[&Snippet]) -> Vec<TreeEntry> {
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
            content: s.content.clone(),
        })
        .collect()
}

const GITEE_TOMBSTONE_RETENTION_DAYS: i64 = 30;

fn snippets_for_gitee_tree(snippets: &[Snippet], now: DateTime<Utc>) -> Vec<&Snippet> {
    let cutoff = now - chrono::Duration::days(GITEE_TOMBSTONE_RETENTION_DAYS);
    snippets
        .iter()
        .filter(|s| !s.deleted || s.update_date >= cutoff)
        .collect()
}

fn tree_entries_to_snippets(tree_entries: Vec<TreeEntry>) -> Vec<Snippet> {
    tree_entries
        .into_iter()
        .map(|entry| Snippet {
            id: entry.id,
            parent_id: entry.parent_id,
            name: entry.name,
            snippet_type: entry.entry_type,
            content: entry.content,
            sort: entry.sort,
            deleted: entry.deleted,
            synced: true,
            create_date: entry.create_date,
            update_date: entry.update_date,
            gitee_gist_id: None,
        })
        .collect()
}

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

pub trait HttpClient: Send {
    fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<HttpResponse, SyncError>;
    fn put(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: &str,
    ) -> Result<HttpResponse, SyncError>;
    fn delete(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<HttpResponse, SyncError>;
}

pub struct RealHttpClient {
    client: reqwest::blocking::Client,
}

impl RealHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl HttpClient for RealHttpClient {
    fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<HttpResponse, SyncError> {
        let mut req = self.client.get(url);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req.send().map_err(|e| SyncError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp.text().map_err(|e| SyncError::Http(e.to_string()))?;
        Ok(HttpResponse { status, body })
    }

    fn put(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: &str,
    ) -> Result<HttpResponse, SyncError> {
        let mut req = self.client.put(url);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req
            .body(body.to_string())
            .send()
            .map_err(|e| SyncError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let resp_body = resp.text().map_err(|e| SyncError::Http(e.to_string()))?;
        Ok(HttpResponse {
            status,
            body: resp_body,
        })
    }

    fn delete(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<HttpResponse, SyncError> {
        let mut req = self.client.delete(url);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req.send().map_err(|e| SyncError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp.text().map_err(|e| SyncError::Http(e.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}

pub fn base_url(config: &SyncConfig) -> String {
    match config.provider {
        SyncProvider::Github => format!(
            "https://api.github.com/repos/{}/{}",
            config.owner, config.repo
        ),
        SyncProvider::Gitee => format!(
            "https://gitee.com/api/v5/repos/{}/{}",
            config.owner, config.repo
        ),
    }
}

pub fn auth_headers(config: &SyncConfig) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    match config.provider {
        SyncProvider::Github => {
            headers.insert("Authorization".into(), format!("Bearer {}", config.token));
            headers.insert("Accept".into(), "application/vnd.github+json".into());
        }
        SyncProvider::Gitee => {
            headers.insert("access_token".into(), config.token.clone());
        }
    }
    headers
}

pub fn pull(config: &SyncConfig, client: &dyn HttpClient) -> Result<Vec<Snippet>, SyncError> {
    match config.provider {
        SyncProvider::Gitee => pull_from_single_gist(config),
        SyncProvider::Github => pull_github(config, client),
    }
}

fn pull_from_single_gist(config: &SyncConfig) -> Result<Vec<Snippet>, SyncError> {
    let gitee_config = &config.gitee_config;
    let gist_id = match &gitee_config.gist_id {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };

    let gitee_client = GiteeClient::new(gitee_config.clone());
    let gist = gitee_client
        .get_gist(gist_id)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    let tree_file = gist
        .files
        .get("tree.json")
        .ok_or_else(|| SyncError::Parse("tree.json not found in gist".into()))?;

    let tree_content = tree_file.content.as_deref().unwrap_or("");
    if tree_content.trim().is_empty() {
        return Ok(Vec::new());
    }

    let tree_entries: Vec<TreeEntry> = serde_json::from_str(tree_content)
        .map_err(|e| SyncError::Parse(format!("Failed to parse tree.json: {}", e)))?;

    Ok(tree_entries_to_snippets(tree_entries))
}

fn pull_github(config: &SyncConfig, client: &dyn HttpClient) -> Result<Vec<Snippet>, SyncError> {
    let base = base_url(config);
    let headers = auth_headers(config);

    // 1. 获取树
    let tree_url = format!("{}/git/trees/{}?recursive=1", base, config.branch);
    let tree_resp = client.get(&tree_url, &headers)?;
    if tree_resp.status != 200 {
        return Err(SyncError::Http(format!(
            "Tree request failed: {}",
            tree_resp.status
        )));
    }
    let tree: GitTreeResponse = serde_json::from_str(&tree_resp.body)
        .map_err(|e| SyncError::Parse(format!("Failed to parse tree: {}", e)))?;

    // 2. 过滤 snippets/ 下的 .yaml 文件
    let yaml_files: Vec<&GitTreeEntry> = tree
        .tree
        .iter()
        .filter(|e| {
            e.entry_type == "blob" && e.path.starts_with("snippets/") && e.path.ends_with(".yaml")
        })
        .collect();

    // 3. 获取每个文件内容并解析
    let mut snippets = Vec::new();
    for entry in yaml_files {
        let content_url = format!("{}/contents/{}", base, entry.path);
        let mut content_headers = headers.clone();
        if matches!(config.provider, SyncProvider::Github) {
            content_headers.insert("Accept".into(), "application/vnd.github.raw+json".into());
        }
        let content_resp = client.get(&content_url, &content_headers)?;
        if content_resp.status != 200 {
            log::warn!("Failed to fetch {}: {}", entry.path, content_resp.status);
            continue;
        }

        let remote_snippet = parse_yaml_snippet(&entry.path, &content_resp.body)?;
        snippets.push(remote_snippet);
    }

    Ok(snippets)
}

fn parse_yaml_snippet(relative_path: &str, body: &str) -> Result<Snippet, SyncError> {
    #[derive(Deserialize)]
    struct RemoteSnippet {
        name: String,
        content: String,
        #[serde(default)]
        sort: i32,
        created: String,
        updated: String,
    }

    let remote: RemoteSnippet = serde_yaml::from_str(body)
        .map_err(|e| SyncError::Parse(format!("YAML parse failed: {}", e)))?;

    let create_date = chrono::DateTime::parse_from_rfc3339(&remote.created)
        .map_err(|e| SyncError::Parse(format!("Invalid created date: {}", e)))?
        .with_timezone(&chrono::Utc);

    let update_date = chrono::DateTime::parse_from_rfc3339(&remote.updated)
        .map_err(|e| SyncError::Parse(format!("Invalid updated date: {}", e)))?
        .with_timezone(&chrono::Utc);

    let id = generate_file_id(relative_path);

    Ok(Snippet {
        id,
        parent_id: None,
        name: remote.name,
        snippet_type: crate::snippet::model::SnippetType::Snippet,
        content: Some(remote.content),
        sort: remote.sort,
        deleted: false,
        synced: true,
        create_date,
        update_date,
        gitee_gist_id: None,
    })
}

pub fn generate_file_id(relative_path: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(relative_path.as_bytes());
    let result = hasher.finalize();
    let hex = format!("{:x}", result);
    hex[..16].to_string()
}

pub fn push(
    config: &SyncConfig,
    client: &dyn HttpClient,
    snippets: &[Snippet],
) -> Result<(Vec<String>, Option<String>), SyncError> {
    match config.provider {
        SyncProvider::Gitee => {
            let gist_id = push_to_single_gist(config, snippets)?;
            Ok((Vec::new(), Some(gist_id)))
        }
        SyncProvider::Github => push_github(config, client, snippets).map(|failed| (failed, None)),
    }
}

fn find_reusable_gitee_config_gist_id(gists: &[Gist], description: &str) -> Option<String> {
    gists
        .iter()
        .filter(|gist| gist.description.as_deref() == Some(description))
        .filter(|gist| gist.files.contains_key("tree.json"))
        .max_by_key(|gist| gist.updated_at)
        .map(|gist| gist.id.clone())
}

/// Push all snippets to a single Gitee gist with tree.json + content files.
fn push_to_single_gist(config: &SyncConfig, snippets: &[Snippet]) -> Result<String, SyncError> {
    let gitee_config = &config.gitee_config;
    let active = snippets_for_gitee_tree(snippets, Utc::now());

    eprintln!(
        "[DEBUG] push_to_single_gist: {} snippets after tombstone retention",
        active.len()
    );

    let tree_entries = build_tree_entries(&active);
    let tree_json =
        serde_json::to_string_pretty(&tree_entries).map_err(|e| SyncError::Store(e.to_string()))?;

    eprintln!(
        "[DEBUG] push_to_single_gist: tree.json size {} bytes",
        tree_json.len()
    );

    let gist_id = match &gitee_config.gist_id {
        Some(id) => {
            let gitee_client = GiteeClient::new(gitee_config.clone());
            match gitee_client.get_gist(id) {
                Ok(_) => id.clone(),
                Err(_) => {
                    let mut files: HashMap<String, String> = HashMap::new();
                    files.insert("tree.json".into(), tree_json.clone());
                    create_new_gist(gitee_config, &files)?
                }
            }
        }
        None => {
            if active.is_empty() {
                return Ok(String::new());
            }
            let mut files: HashMap<String, String> = HashMap::new();
            files.insert("tree.json".into(), tree_json.clone());
            create_new_gist(gitee_config, &files)?
        }
    };

    let mut update = GistUpdate::new(Some(&gitee_config.gist_description));
    update = update.add_file("tree.json", &tree_json);

    let gitee_client = GiteeClient::new(gitee_config.clone());
    gitee_client
        .update_gist(&gist_id, &update)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    Ok(gist_id)
}

fn create_new_gist(
    gitee_config: &GiteeConfig,
    files: &HashMap<String, String>,
) -> Result<String, SyncError> {
    let gitee_client = GiteeClient::new(gitee_config.clone());
    if let Ok(gists) = gitee_client.list_gists() {
        if let Some(existing_id) =
            find_reusable_gitee_config_gist_id(&gists, &gitee_config.gist_description)
        {
            eprintln!(
                "[SYNC] Reusing existing Gitee gist {} for description {}",
                existing_id, gitee_config.gist_description
            );
            return Ok(existing_id);
        }
    }

    let mut gist_files = HashMap::new();
    for (name, content) in files {
        gist_files.insert(name.clone(), GistFileCreate::new(Some(content)));
    }

    let create = GistCreate {
        description: Some(&gitee_config.gist_description),
        files: gist_files,
        public: false,
    };

    let gist = gitee_client
        .create_gist(&create)
        .map_err(|e| SyncError::Http(e.to_string()))?;

    Ok(gist.id)
}

fn push_github(
    config: &SyncConfig,
    client: &dyn HttpClient,
    snippets: &[Snippet],
) -> Result<Vec<String>, SyncError> {
    let base = base_url(config);
    let headers = auth_headers(config);
    let mut failed_ids = Vec::new();

    for snippet in snippets {
        if snippet.snippet_type == crate::snippet::model::SnippetType::Folder {
            continue;
        }

        let relative_path = snippet_to_path(snippet)?;
        let yaml_content = snippet_to_yaml(snippet)?;

        if snippet.deleted {
            let delete_url = format!("{}/contents/{}", base, relative_path);
            let _delete_body = serde_json::json!({
                "message": format!("Delete snippet: {}", snippet.name),
                "branch": config.branch,
            });
            let resp = client.delete(&delete_url, &headers);
            match resp {
                Ok(r) if r.status == 200 || r.status == 404 => {
                    // 404 = 文件已被删除，视为成功
                }
                Ok(r) => {
                    log::warn!("Delete failed for {}: {}", snippet.name, r.status);
                    failed_ids.push(snippet.id.clone());
                    continue;
                }
                Err(e) => {
                    log::warn!("Delete error for {}: {}", snippet.name, e);
                    failed_ids.push(snippet.id.clone());
                    continue;
                }
            }
        } else {
            let put_url = format!("{}/contents/{}", base, relative_path);
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                yaml_content.as_bytes(),
            );

            let body = serde_json::json!({
                "message": format!("Update snippet: {}", snippet.name),
                "content": encoded,
                "branch": config.branch,
            });
            let body_str = serde_json::to_string(&body).unwrap();
            let resp = client.put(&put_url, &headers, &body_str);
            match resp {
                Ok(r) if r.status == 200 || r.status == 201 => {}
                Ok(r) => {
                    log::warn!("Push failed for {}: {}", snippet.name, r.status);
                    failed_ids.push(snippet.id.clone());
                    continue;
                }
                Err(e) => {
                    log::warn!("Push error for {}: {}", snippet.name, e);
                    failed_ids.push(snippet.id.clone());
                    continue;
                }
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

fn snippet_to_path(snippet: &Snippet) -> Result<String, SyncError> {
    let safe_name = sanitize_filename(&snippet.name);
    Ok(format!("snippets/{}.yaml", safe_name))
}

fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
}

fn snippet_to_yaml(snippet: &Snippet) -> Result<String, SyncError> {
    let content = snippet.content.as_deref().unwrap_or("");
    let yaml = format!(
        r#"name: {}
content: |
{}
sort: {}
created: "{}"
updated: "{}"
"#,
        snippet.name,
        content
            .lines()
            .map(|l| format!("  {}", l))
            .collect::<Vec<_>>()
            .join("\n"),
        snippet.sort,
        snippet.create_date.to_rfc3339(),
        snippet.update_date.to_rfc3339(),
    );
    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippet::config::GithubConfig;
    use std::sync::Mutex;

    thread_local! {
        static MOCK_REQUESTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    }

    struct MockHttpClient;

    impl HttpClient for MockHttpClient {
        fn get(
            &self,
            url: &str,
            _headers: &HashMap<String, String>,
        ) -> Result<HttpResponse, SyncError> {
            MOCK_REQUESTS.with(|r| r.lock().unwrap().push(("GET".into(), url.into())));
            if url.contains("git/trees") {
                Ok(HttpResponse {
                    status: 200,
                    body:
                        r#"{"tree":[{"path":"snippets/test.yaml","type":"blob","sha":"abc123"}]}"#
                            .into(),
                })
            } else if url.contains("contents/snippets/test.yaml") {
                Ok(HttpResponse {
                    status: 200,
                    body: "name: test\ncontent: |\n  echo hello\nsort: 0\ncreated: \"2026-06-17T10:00:00Z\"\nupdated: \"2026-06-17T10:00:00Z\"\n".into(),
                })
            } else {
                Ok(HttpResponse {
                    status: 404,
                    body: "Not found".into(),
                })
            }
        }

        fn put(
            &self,
            url: &str,
            _headers: &HashMap<String, String>,
            _body: &str,
        ) -> Result<HttpResponse, SyncError> {
            MOCK_REQUESTS.with(|r| r.lock().unwrap().push(("PUT".into(), url.into())));
            Ok(HttpResponse {
                status: 200,
                body: r#"{}"#.into(),
            })
        }

        fn delete(
            &self,
            url: &str,
            _headers: &HashMap<String, String>,
        ) -> Result<HttpResponse, SyncError> {
            MOCK_REQUESTS.with(|r| r.lock().unwrap().push(("DELETE".into(), url.into())));
            Ok(HttpResponse {
                status: 200,
                body: r#"{}"#.into(),
            })
        }
    }

    fn test_config() -> SyncConfig {
        SyncConfig {
            provider: SyncProvider::Github,
            owner: "testuser".into(),
            repo: "test-repo".into(),
            branch: "main".into(),
            token: "test-token".into(),
            github_config: GithubConfig::default(),
            gitee_config: GiteeConfig::default(),
        }
    }

    #[test]
    fn test_base_url_github() {
        let config = test_config();
        let url = base_url(&config);
        assert_eq!(url, "https://api.github.com/repos/testuser/test-repo");
    }

    #[test]
    fn test_base_url_gitee() {
        let config = SyncConfig {
            provider: SyncProvider::Gitee,
            owner: "giteeuser".into(),
            repo: "gitee-repo".into(),
            branch: "master".into(),
            token: "token".into(),
            github_config: GithubConfig::default(),
            gitee_config: GiteeConfig::default(),
        };
        let url = base_url(&config);
        assert_eq!(url, "https://gitee.com/api/v5/repos/giteeuser/gitee-repo");
    }

    #[test]
    fn test_auth_headers_github() {
        let config = test_config();
        let headers = auth_headers(&config);
        assert_eq!(headers["Authorization"], "Bearer test-token");
        assert_eq!(headers["Accept"], "application/vnd.github+json");
    }

    #[test]
    fn test_auth_headers_gitee() {
        let config = SyncConfig {
            provider: SyncProvider::Gitee,
            owner: "user".into(),
            repo: "repo".into(),
            branch: "main".into(),
            token: "gitee-token".into(),
            github_config: GithubConfig::default(),
            gitee_config: GiteeConfig::default(),
        };
        let headers = auth_headers(&config);
        assert_eq!(headers["access_token"], "gitee-token");
    }

    #[test]
    fn test_generate_file_id() {
        let id1 = generate_file_id("snippets/test.yaml");
        let id2 = generate_file_id("snippets/test.yaml");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);

        let id3 = generate_file_id("snippets/other.yaml");
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_parse_yaml_snippet() {
        let body = "name: 查看日志\ncontent: |\n  tail -f /var/log/syslog\nsort: 5\ncreated: \"2026-06-17T10:01:00Z\"\nupdated: \"2026-06-17T10:30:00Z\"\n";
        let snippet = parse_yaml_snippet("snippets/查看日志.yaml", body).unwrap();
        assert_eq!(snippet.name, "查看日志");
        assert_eq!(snippet.sort, 5);
        assert!(snippet.synced);
    }

    #[test]
    fn test_parse_invalid_yaml_fails() {
        let result = parse_yaml_snippet("test.yaml", "invalid: yaml: :");
        assert!(result.is_err());
    }

    #[test]
    fn test_snippet_to_yaml_format() {
        let snippet = Snippet::new_snippet("test".into(), "line1\nline2".into(), None);
        let yaml = snippet_to_yaml(&snippet).unwrap();
        assert!(yaml.contains("name: test"));
        assert!(yaml.contains("content: |"));
        assert!(yaml.contains("  line1"));
        assert!(yaml.contains("  line2"));
    }

    #[test]
    fn test_snippet_to_path() {
        let snippet = Snippet::new_snippet("正常文件名".into(), "content".into(), None);
        let path = snippet_to_path(&snippet).unwrap();
        assert_eq!(path, "snippets/正常文件名.yaml");
    }

    #[test]
    fn test_snippet_to_path_sanitizes() {
        let snippet = Snippet::new_snippet("bad/name".into(), "content".into(), None);
        let path = snippet_to_path(&snippet).unwrap();
        assert_eq!(path, "snippets/bad_name.yaml");
    }

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
            content: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"id\":\"uuid-1\""));
        assert!(json.contains("\"type\":\"folder\""));
        assert!(
            !json.contains("\"content\""),
            "content should be absent for folder"
        );
    }

    #[test]
    fn test_tree_entry_snippet_has_content() {
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
            content: Some("echo ping".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"snippet\""));
        assert!(json.contains("\"content\":\"echo ping\""));
    }

    #[test]
    fn test_build_tree_entries_includes_content() {
        let folder = Snippet::new_folder("网络工具".into(), None);
        let snippet =
            Snippet::new_snippet("ping".into(), "echo ping".into(), Some(folder.id.clone()));

        let entries = build_tree_entries(&[&folder, &snippet]);
        assert_eq!(entries.len(), 2);

        let snip_entry = entries
            .iter()
            .find(|e| e.entry_type == crate::snippet::model::SnippetType::Snippet)
            .unwrap();
        assert_eq!(snip_entry.content, Some("echo ping".into()));
    }

    #[test]
    fn test_find_reusable_gitee_config_gist_prefers_existing_tree_json_gist() {
        let older = Gist {
            id: "older".into(),
            description: Some("CopyTimeTrayConfig".into()),
            files: HashMap::from([(
                "tree.json".into(),
                crate::snippet::gitee::types::GistFile {
                    filename: Some("tree.json".into()),
                    content: None,
                    raw_url: None,
                },
            )]),
            created_at: "2026-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2026-01-01T00:00:00Z".parse().unwrap(),
        };
        let newer = Gist {
            id: "newer".into(),
            description: Some("CopyTimeTrayConfig".into()),
            files: HashMap::from([(
                "tree.json".into(),
                crate::snippet::gitee::types::GistFile {
                    filename: Some("tree.json".into()),
                    content: None,
                    raw_url: None,
                },
            )]),
            created_at: "2026-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2026-01-02T00:00:00Z".parse().unwrap(),
        };
        let old_layout = Gist {
            id: "old-layout".into(),
            description: Some("CopyTimeTrayConfig".into()),
            files: HashMap::new(),
            created_at: "2026-01-03T00:00:00Z".parse().unwrap(),
            updated_at: "2026-01-03T00:00:00Z".parse().unwrap(),
        };

        let reusable =
            find_reusable_gitee_config_gist_id(&[older, newer, old_layout], "CopyTimeTrayConfig");

        assert_eq!(reusable, Some("newer".into()));
    }

    #[test]
    fn test_gitee_tree_keeps_recent_deleted_tombstones_for_30_days() {
        let now = chrono::Utc::now();
        let mut active = Snippet::new_snippet("active".into(), "active content".into(), None);
        active.id = "active".into();

        let mut recent_deleted =
            Snippet::new_snippet("recent deleted".into(), "deleted content".into(), None);
        recent_deleted.id = "recent-deleted".into();
        recent_deleted.deleted = true;
        recent_deleted.update_date = now - chrono::Duration::days(29);

        let mut expired_deleted =
            Snippet::new_snippet("expired deleted".into(), "old deleted content".into(), None);
        expired_deleted.id = "expired-deleted".into();
        expired_deleted.deleted = true;
        expired_deleted.update_date = now - chrono::Duration::days(31);

        let snippets = vec![active, recent_deleted, expired_deleted];
        let kept = snippets_for_gitee_tree(&snippets, now);

        let kept_ids: Vec<&str> = kept.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(kept_ids, vec!["active", "recent-deleted"]);
    }

    #[test]
    fn test_tree_entries_to_snippets_preserves_deleted_tombstones() {
        use crate::snippet::model::SnippetType;
        let deleted_at = chrono::Utc::now();
        let entries = vec![TreeEntry {
            id: "deleted-id".into(),
            parent_id: None,
            name: "deleted remote".into(),
            entry_type: SnippetType::Snippet,
            sort: 0,
            deleted: true,
            create_date: deleted_at - chrono::Duration::days(1),
            update_date: deleted_at,
            content: Some("old content".into()),
        }];

        let snippets = tree_entries_to_snippets(entries);

        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].id, "deleted-id");
        assert!(snippets[0].deleted);
        assert!(snippets[0].synced);
    }

    #[test]
    fn test_pull_from_single_gist_reconstructs_snippets() {
        let tree_content = "[{\"id\":\"f1\",\"parent_id\":null,\"name\":\"网络工具\",\"type\":\"folder\",\"sort\":0,\"deleted\":false,\"create_date\":\"2026-01-01T00:00:00Z\",\"update_date\":\"2026-01-01T00:00:00Z\"},{\"id\":\"s1\",\"parent_id\":\"f1\",\"name\":\"ping\",\"type\":\"snippet\",\"sort\":0,\"deleted\":false,\"create_date\":\"2026-01-01T00:00:00Z\",\"update_date\":\"2026-01-02T00:00:00Z\",\"content\":\"echo ping\"}]";
        let entries: Vec<TreeEntry> = serde_json::from_str(tree_content).unwrap();
        assert_eq!(entries.len(), 2);
        let snip = entries.iter().find(|e| e.id == "s1").unwrap();
        assert_eq!(snip.content, Some("echo ping".into()));
        assert_eq!(snip.parent_id, Some("f1".into()));
    }
}
