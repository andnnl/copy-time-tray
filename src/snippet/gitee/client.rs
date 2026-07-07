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

        self.check_status(resp)?.json::<Gist>().map_err(|e| {
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

        self.check_status(resp)?
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

        self.check_status(resp).map(|_| ())
    }

    fn check_status(&self, resp: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
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
            Ok(resp)
        } else {
            Err(error::GiteeError::Http(format!(
                "HTTP {}",
                status
            )))
        }
    }
}
