use std::collections::HashMap;

use serde::Deserialize;

use crate::snippet::config::{SyncConfig, SyncProvider};
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

pub fn pull(config: &SyncConfig, client: &dyn HttpClient) -> Result<Vec<Snippet>, SyncError> {
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

// Push 协议骨架（Task 7 中完善）
pub fn push(
    _config: &SyncConfig,
    _client: &dyn HttpClient,
    _snippets: &[Snippet],
) -> Result<(), SyncError> {
    Ok(())
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
    fn test_pull_retrieves_snippets() {
        let config = test_config();
        let client = MockHttpClient;
        let result = pull(&config, &client);
        assert!(result.is_ok());
        let snippets = result.unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "test");
        assert_eq!(snippets[0].content, Some("echo hello\n".into()));
        assert!(snippets[0].synced);
    }

    #[test]
    fn test_pull_non_200_returns_error() {
        struct FailingClient;
        impl HttpClient for FailingClient {
            fn get(
                &self,
                _url: &str,
                _headers: &HashMap<String, String>,
            ) -> Result<HttpResponse, SyncError> {
                Ok(HttpResponse {
                    status: 401,
                    body: "Unauthorized".into(),
                })
            }
            fn put(
                &self,
                _url: &str,
                _headers: &HashMap<String, String>,
                _body: &str,
            ) -> Result<HttpResponse, SyncError> {
                Ok(HttpResponse {
                    status: 200,
                    body: "{}".into(),
                })
            }
            fn delete(
                &self,
                _url: &str,
                _headers: &HashMap<String, String>,
            ) -> Result<HttpResponse, SyncError> {
                Ok(HttpResponse {
                    status: 200,
                    body: "{}".into(),
                })
            }
        }
        let config = test_config();
        let result = pull(&config, &FailingClient);
        assert!(result.is_err());
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
}
