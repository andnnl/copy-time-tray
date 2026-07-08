use crate::snippet::config::GiteeConfig;
use crate::snippet::gitee::types::{Gist, GistCreate, GistUpdate};
use crate::snippet::gitee::{error, Result};

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

    fn url_gists(&self) -> String {
        format!("{}/gists", self.api_base())
    }

    fn url_gist(&self, id: &str) -> String {
        format!("{}/gists/{}", self.api_base(), id)
    }

    fn with_auth(&self, req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        req.query(&[("access_token", self.config.token.as_str())])
    }

    pub fn list_gists(&self) -> Result<Vec<Gist>> {
        let mut all_gists = Vec::new();
        let mut page = 1;
        let per_page = 100;

        loop {
            let url = format!("{}?page={}&per_page={}", self.url_gists(), page, per_page);
            eprintln!("[DEBUG] list_gists: GET {}", url);
            let resp = self
                .with_auth(self.client.get(&url))
                .header("Accept", "application/json")
                .send()
                .map_err(|e| error::GiteeError::Http(e.to_string()))?;

            let status = resp.status();
            eprintln!("[DEBUG] list_gists: HTTP {}", status);
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

            let gists: Vec<Gist> = resp
                .json()
                .map_err(|e| error::GiteeError::Parse(e.to_string()))?;

            let count = gists.len();
            all_gists.extend(gists);

            // If we got fewer results than per_page, we've reached the end
            if count < per_page {
                break;
            }
            page += 1;
        }

        Ok(all_gists)
    }

    pub fn create_gist(&self, gist: &GistCreate) -> Result<Gist> {
        eprintln!("[DEBUG] create_gist: POST {}", self.url_gists());
        let resp = self
            .with_auth(self.client.post(self.url_gists()))
            .header("Accept", "application/json")
            .json(gist)
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        self.check_status(resp)?.json::<Gist>().map_err(|e| {
            eprintln!("[DEBUG] create_gist: JSON parse error: {}", e);
            error::GiteeError::Parse(e.to_string())
        })
    }

    pub fn update_gist(&self, id: &str, gist: &GistUpdate) -> Result<Gist> {
        eprintln!("[DEBUG] update_gist: PATCH {}", self.url_gist(id));
        let resp = self
            .with_auth(self.client.patch(self.url_gist(id)))
            .header("Accept", "application/json")
            .json(gist)
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        self.check_status(resp)?
            .json::<Gist>()
            .map_err(|e| {
                eprintln!("[DEBUG] update_gist: JSON parse error: {}", e);
                error::GiteeError::Parse(e.to_string())
            })
    }

    pub fn delete_gist(&self, id: &str) -> Result<()> {
        let resp = self
            .with_auth(self.client.delete(self.url_gist(id)))
            .send()
            .map_err(|e| error::GiteeError::Http(e.to_string()))?;

        self.check_status(resp).map(|_| ())
    }

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

    fn check_status(&self, resp: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        let status = resp.status();
        eprintln!("[DEBUG] check_status: HTTP {}", status);
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
            Ok(resp)
        } else {
            Err(error::GiteeError::Http(format!(
                "HTTP {}",
                status
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippet::config::GiteeConfig;

    #[test]
    fn test_url_gist_constructs_correct_url() {
        let config = GiteeConfig {
            api_base: "https://gitee.com/api/v5".into(),
            token: "test-token".into(),
            auto_sync: true,
            sync_interval: 300,
            gist_id: None,
            gist_description: "CopyTimeTrayConfig".into(),
        };
        let client = GiteeClient::new(config);
        let url = client.url_gist("abc123");
        assert_eq!(url, "https://gitee.com/api/v5/gists/abc123");
    }

    #[test]
    fn test_url_gist_with_custom_api_base() {
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
        assert_eq!(url, "http://localhost:9999/gists/xyz789");
    }
}
