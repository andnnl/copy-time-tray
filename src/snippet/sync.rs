use std::collections::HashMap;

use serde::Deserialize;

use crate::snippet::config::{SyncConfig, SyncProvider, GiteeConfig};
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
            headers.insert(
                "Authorization".into(),
                format!("Bearer {}", config.token),
            );
            headers.insert("Accept".into(), "application/vnd.github+json".into());
        }
        SyncProvider::Gitee => {
            headers.insert("access_token".into(), config.token.clone());
        }
    }
    headers
}

pub fn generate_gist_folder_id(gist_id: &str) -> String {
    format!("gist:{}", gist_id)
}

pub fn generate_file_id_from_path(relative_path: &str) -> String {
    generate_file_id(relative_path)
}

pub fn pull(config: &SyncConfig, client: &dyn HttpClient) -> Result<Vec<Snippet>, SyncError> {
    match config.provider {
        SyncProvider::Gitee => {
            let gitee_client = GiteeClient::new(config.gitee_config.clone());
            let gists = gitee_client
                .list_gists()
                .map_err(|e| SyncError::Http(e.to_string()))?;

            let mut snippets = Vec::new();
            for gist in gists {
                let folder_name = gist.description.as_deref().unwrap_or("_default");
                let folder_id = generate_gist_folder_id(&gist.id);
                let folder = Snippet {
                    id: folder_id.clone(),
                    parent_id: None,
                    name: folder_name.to_string(),
                    snippet_type: crate::snippet::model::SnippetType::Folder,
                    content: None,
                    sort: 0,
                    deleted: false,
                    synced: true,
                    create_date: gist.created_at,
                    update_date: gist.updated_at,
                    gitee_gist_id: Some(gist.id.clone()),
                };
                snippets.push(folder);

                for (filename, file) in gist.files {
                    let name = filename.strip_suffix(".sh").unwrap_or(&filename).to_string();
                    let content = file.content.unwrap_or_default();
                    let file_id = generate_file_id_from_path(&filename);
                    let snippet = Snippet {
                        id: file_id,
                        parent_id: Some(folder_id.clone()),
                        name,
                        snippet_type: crate::snippet::model::SnippetType::Snippet,
                        content: Some(content),
                        sort: 0,
                        deleted: false,
                        synced: true,
                        create_date: gist.created_at,
                        update_date: gist.updated_at,
                        gitee_gist_id: None,
                    };
                    snippets.push(snippet);
                }
            }

            Ok(snippets)
        }
        SyncProvider::Github => {
            pull_github(config, client)
        }
    }
}

fn pull_github(
    config: &SyncConfig,
    client: &dyn HttpClient,
) -> Result<Vec<Snippet>, SyncError> {
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
            log::warn!(
                "Failed to fetch {}: {}",
                entry.path,
                content_resp.status
            );
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

    let remote: RemoteSnippet =
        serde_yaml::from_str(body).map_err(|e| SyncError::Parse(format!("YAML parse failed: {}", e)))?;

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
) -> Result<Vec<String>, SyncError> {
    match config.provider {
        SyncProvider::Gitee => {
            push_gitee(config, snippets)
        }
        SyncProvider::Github => {
            push_github(config, client, snippets)
        }
    }
}

fn push_gitee(
    config: &SyncConfig,
    snippets: &[Snippet],
) -> Result<Vec<String>, SyncError> {
    let gitee_client = GiteeClient::new(config.gitee_config.clone());
    let mut failed_ids = Vec::new();

    let mut groups: HashMap<Option<String>, Vec<&Snippet>> = HashMap::new();
    for snippet in snippets {
        if snippet.snippet_type == crate::snippet::model::SnippetType::Folder {
            continue;
        }
        groups.entry(snippet.parent_id.clone()).or_default().push(snippet);
    }

    let folders: HashMap<String, &Snippet> = snippets
        .iter()
        .filter(|s| s.snippet_type == crate::snippet::model::SnippetType::Folder)
        .map(|s| (s.id.clone(), s))
        .collect();

    for (parent_id, group_snippets) in groups {
        let folder = parent_id
            .as_ref()
            .and_then(|pid| folders.get(pid));
        let gist_id = folder.and_then(|f| f.gitee_gist_id.clone());
        let folder_name = folder.map(|f| f.name.as_str()).unwrap_or("_default");

        if let Some(gist_id) = gist_id {
            let mut filenames = Vec::new();
            let mut contents = Vec::new();
            for snippet in &group_snippets {
                filenames.push(format!("{}.sh", snippet.name));
                contents.push(snippet.content.as_deref().unwrap_or(""));
            }
            let mut update = GistUpdate::new(Some(folder_name));
            for (filename, content) in filenames.iter().zip(contents.iter()) {
                update = update.add_file(filename, content);
            }
            match gitee_client.update_gist(&gist_id, &update) {
                Ok(_) => {}
                Err(e) => {
                    log::warn!("Update gist failed: {}", e);
                    failed_ids.extend(group_snippets.iter().map(|s| s.id.clone()));
                }
            }
        } else {
            let mut files = HashMap::new();
            for snippet in &group_snippets {
                let filename = format!("{}.sh", snippet.name);
                files.insert(
                    filename,
                    GistFileCreate::new(snippet.content.as_deref()),
                );
            }
            let create = GistCreate {
                description: Some(folder_name),
                files,
                public: false,
            };
            match gitee_client.create_gist(&create) {
                Ok(_) => {}
                Err(e) => {
                    log::warn!("Create gist failed: {}", e);
                    failed_ids.extend(group_snippets.iter().map(|s| s.id.clone()));
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
    use std::sync::Mutex;

    thread_local! {
        static MOCK_REQUESTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    }

    struct MockHttpClient;

    impl HttpClient for MockHttpClient {
        fn get(&self, url: &str, _headers: &HashMap<String, String>) -> Result<HttpResponse, SyncError> {
            MOCK_REQUESTS.with(|r| r.lock().unwrap().push(("GET".into(), url.into())));
            if url.contains("git/trees") {
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"tree":[{"path":"snippets/test.yaml","type":"blob","sha":"abc123"}]}"#
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
            gitee_config: GiteeConfig::default(),
        };
        let url = base_url(&config);
        assert_eq!(
            url,
            "https://gitee.com/api/v5/repos/giteeuser/gitee-repo"
        );
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
}
