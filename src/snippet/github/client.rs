use super::error::{GithubError, Result};
use super::types::{Gist, GistCreate, GistUpdate};
use crate::snippet::config::GithubConfig;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::Url;

const ACCEPT_VALUE: &str = "application/vnd.github+json";
const USER_AGENT_VALUE: &str = "copy-time-tray/0.3";
const PAGE_SIZE: usize = 100;
const GITHUB_API_HOST: &str = "api.github.com";
const GITHUB_RAW_HOSTS: &[&str] = &["gist.githubusercontent.com", "raw.githubusercontent.com"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawAuthentication {
    Bearer,
    Anonymous,
}

#[derive(Debug, Clone)]
pub struct GithubClient {
    api_base: String,
    headers: HeaderMap,
    client: Client,
    raw_client: Client,
}

impl GithubClient {
    pub fn new(config: GithubConfig) -> Self {
        let api_base = if config.api_base.is_empty() {
            "https://api.github.com".to_owned()
        } else {
            config.api_base.trim_end_matches('/').to_owned()
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", config.token))
                .expect("GitHub token must be a valid HTTP header value"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static(ACCEPT_VALUE));
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

        Self {
            api_base,
            headers,
            client: Client::new(),
            // Raw content URLs must not redirect to another host after their
            // trust decision has been made.
            raw_client: Client::builder()
                .redirect(Policy::none())
                .build()
                .expect("reqwest client configuration must be valid"),
        }
    }

    pub fn headers(&self) -> HeaderMap {
        self.headers.clone()
    }

    pub fn get_gist(&self, id: &str) -> Result<Gist> {
        self.send_json(self.client.get(self.url_gist(id)))
    }

    pub fn list_gists(&self) -> Result<Vec<Gist>> {
        let mut gists = Vec::new();
        let mut page = 1;

        loop {
            let mut current_page: Vec<Gist> =
                self.send_json(self.client.get(self.url_gists_page(page)))?;
            let count = current_page.len();
            gists.append(&mut current_page);
            if count < PAGE_SIZE {
                return Ok(gists);
            }
            page += 1;
        }
    }

    pub fn create_private_gist(&self, description: &str, content: &str) -> Result<Gist> {
        let request = GistCreate::private_tree(description, content);
        self.send_json(self.client.post(self.url_gists()).json(&request))
    }

    pub fn update_gist(&self, id: &str, content: &str) -> Result<Gist> {
        let request = GistUpdate::tree(content);
        self.send_json(self.client.patch(self.url_gist(id)).json(&request))
    }

    pub fn fetch_raw(&self, url: &str) -> Result<String> {
        let request = self.raw_client.get(url);
        let request = match raw_authentication_policy(&self.api_base, url)? {
            RawAuthentication::Bearer => self.with_headers(request),
            RawAuthentication::Anonymous => request,
        };

        self.check_status(request.send().map_err(network_error)?)?
            .text()
            .map_err(network_error)
    }

    fn url_gists(&self) -> String {
        format!("{}/gists", self.api_base)
    }

    fn url_gists_page(&self, page: usize) -> String {
        format!("{}?per_page={PAGE_SIZE}&page={page}", self.url_gists())
    }

    fn url_gist(&self, id: &str) -> String {
        format!("{}/{}", self.url_gists(), id)
    }

    fn with_headers(&self, request: RequestBuilder) -> RequestBuilder {
        request.headers(self.headers.clone())
    }

    fn send_json<T: serde::de::DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
        self.check_status(self.with_headers(request).send().map_err(network_error)?)?
            .json()
            .map_err(|error| GithubError::Parse(error.to_string()))
    }

    fn check_status(&self, response: Response) -> Result<Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        if status.as_u16() == 404 {
            return Err(GithubError::NotFound);
        }
        if matches!(status.as_u16(), 401 | 403) {
            return Err(GithubError::Auth(status.as_u16()));
        }

        let body = response.text().unwrap_or_default();
        Err(GithubError::Http {
            status: status.as_u16(),
            body: redact_token(&body, &self.headers),
        })
    }
}

pub fn find_reusable_gist_id(gists: &[Gist], description: &str) -> Option<String> {
    gists
        .iter()
        .filter(|gist| {
            gist.description.as_deref() == Some(description) && gist.files.contains_key("tree.json")
        })
        .max_by_key(|gist| gist.updated_at)
        .map(|gist| gist.id.clone())
}

fn raw_authentication_policy(api_base: &str, raw_url: &str) -> Result<RawAuthentication> {
    let raw_url = Url::parse(raw_url).map_err(|_| untrusted_raw_url_error())?;
    if raw_url.scheme() != "https" {
        return Err(untrusted_raw_url_error());
    }

    let raw_host = raw_url.host_str().ok_or_else(untrusted_raw_url_error)?;
    let api_url = Url::parse(api_base).map_err(|_| untrusted_raw_url_error())?;
    if api_url.scheme() != "https" {
        return Err(untrusted_raw_url_error());
    }

    let api_host = api_url.host_str().ok_or_else(untrusted_raw_url_error)?;
    if api_host == raw_host {
        return Ok(RawAuthentication::Bearer);
    }

    if api_host == GITHUB_API_HOST && GITHUB_RAW_HOSTS.contains(&raw_host) {
        return Ok(RawAuthentication::Anonymous);
    }

    Err(untrusted_raw_url_error())
}

fn untrusted_raw_url_error() -> GithubError {
    GithubError::Http {
        status: 400,
        body: "untrusted raw URL: only HTTPS API-base hosts and official GitHub raw-content hosts are allowed".into(),
    }
}

fn network_error(error: reqwest::Error) -> GithubError {
    GithubError::Network(error.without_url().to_string())
}

fn redact_token(body: &str, headers: &HeaderMap) -> String {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    match token {
        Some(token) if !token.is_empty() => body.replace(token, "[REDACTED]"),
        _ => body.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        find_reusable_gist_id, raw_authentication_policy, GithubClient, RawAuthentication,
    };
    use crate::snippet::config::GithubConfig;
    use crate::snippet::github::types::{Gist, GistCreate, GistFile, GistFileCreate};
    use chrono::{DateTime, Utc};
    use std::collections::HashMap;

    fn github_config() -> GithubConfig {
        GithubConfig {
            api_base: "https://api.example.test/".into(),
            token: "gh-token".into(),
            ..GithubConfig::default()
        }
    }

    fn gist(id: &str, description: Option<&str>, updated_at: &str, has_tree: bool) -> Gist {
        let mut files = HashMap::new();
        if has_tree {
            files.insert(
                "tree.json".into(),
                GistFile {
                    content: Some("[]".into()),
                    truncated: false,
                    raw_url: None,
                },
            );
        }

        Gist {
            id: id.into(),
            description: description.map(str::to_owned),
            files,
            created_at: Some(updated_at.parse::<DateTime<Utc>>().unwrap()),
            updated_at: Some(updated_at.parse::<DateTime<Utc>>().unwrap()),
        }
    }

    #[test]
    fn raw_authentication_policy_rejects_untrusted_urls_without_a_token() {
        let error = raw_authentication_policy(
            "https://api.github.com",
            "https://attacker.example/raw/tree.json",
        )
        .unwrap_err();

        assert!(matches!(
            error,
            crate::snippet::github::error::GithubError::Http { status: 400, .. }
        ));
        assert!(error.to_string().contains("untrusted raw URL"));
    }

    #[test]
    fn raw_authentication_policy_authenticates_only_https_api_base_same_origin_urls() {
        assert_eq!(
            raw_authentication_policy(
                "https://github.example/api/v3",
                "https://github.example/api/v3/gists/1/raw",
            )
            .unwrap(),
            RawAuthentication::Bearer,
        );
        assert!(raw_authentication_policy(
            "https://github.example/api/v3",
            "http://github.example/api/v3/gists/1/raw",
        )
        .is_err());
        assert_eq!(
            raw_authentication_policy(
                "https://github.example/api/v3",
                "https://github.example:8443/api/v3/gists/1/raw",
            )
            .unwrap(),
            RawAuthentication::Bearer,
        );
        assert!(raw_authentication_policy("https://github.example/api/v3", "not a URL").is_err());
    }

    #[test]
    fn raw_authentication_policy_allows_official_github_raw_hosts_anonymously() {
        for raw_url in [
            "https://gist.githubusercontent.com/user/gist/raw/tree.json",
            "https://raw.githubusercontent.com/user/repo/main/tree.json",
        ] {
            assert_eq!(
                raw_authentication_policy("https://api.github.com", raw_url).unwrap(),
                RawAuthentication::Anonymous,
            );
        }
    }
    #[test]
    fn gist_requests_use_bearer_accept_and_user_agent_headers() {
        let client = GithubClient::new(github_config());
        let headers = client.headers();

        assert_eq!(headers["Authorization"], "Bearer gh-token");
        assert_eq!(headers["Accept"], "application/vnd.github+json");
        assert_eq!(headers["User-Agent"], "copy-time-tray/0.3");
    }

    #[test]
    fn gist_urls_normalize_api_base_and_include_expected_paths_and_query() {
        let client = GithubClient::new(github_config());

        assert_eq!(
            client.url_gist("gist-1"),
            "https://api.example.test/gists/gist-1"
        );
        assert_eq!(
            client.url_gists_page(3),
            "https://api.example.test/gists?per_page=100&page=3"
        );
        assert_eq!(client.url_gists(), "https://api.example.test/gists");
    }

    #[test]
    fn gist_file_deserializes_optional_content_truncation_and_raw_url() {
        let file: GistFile = serde_json::from_str(
            r#"{"content":null,"truncated":true,"raw_url":"https://gist.example/raw/tree.json"}"#,
        )
        .unwrap();

        assert_eq!(file.content, None);
        assert!(file.truncated);
        assert_eq!(
            file.raw_url.as_deref(),
            Some("https://gist.example/raw/tree.json")
        );

        let defaulted: GistFile = serde_json::from_str(r#"{}"#).unwrap();
        assert!(!defaulted.truncated);
        assert_eq!(defaulted.content, None);
        assert_eq!(defaulted.raw_url, None);
    }

    #[test]
    fn reusable_gist_prefers_newest_matching_tree_json() {
        let gists = vec![
            gist(
                "older",
                Some("CopyTimeTrayConfig"),
                "2026-07-10T12:00:00Z",
                true,
            ),
            gist(
                "newer",
                Some("CopyTimeTrayConfig"),
                "2026-07-11T12:00:00Z",
                true,
            ),
            gist(
                "wrong-layout",
                Some("CopyTimeTrayConfig"),
                "2026-07-12T12:00:00Z",
                false,
            ),
            gist(
                "wrong-description",
                Some("Other"),
                "2026-07-13T12:00:00Z",
                true,
            ),
        ];

        assert_eq!(
            find_reusable_gist_id(&gists, "CopyTimeTrayConfig"),
            Some("newer".into())
        );
    }

    #[test]
    fn private_creation_body_contains_only_requested_tree_file() {
        let body = GistCreate::private_tree("CopyTimeTrayConfig", "[{}]");
        let value = serde_json::to_value(body).unwrap();

        assert_eq!(value["description"], "CopyTimeTrayConfig");
        assert_eq!(value["public"], false);
        assert_eq!(value["files"]["tree.json"]["content"], "[{}]");
        assert_eq!(value["files"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn create_file_serializes_its_content() {
        let file = GistFileCreate::new("contents");
        assert_eq!(
            serde_json::to_string(&file).unwrap(),
            r#"{"content":"contents"}"#
        );
    }
}
