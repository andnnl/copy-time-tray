use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncProvider {
    Github,
    Gitee,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub provider: SyncProvider,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default)]
    pub github_config: GithubConfig,
    #[serde(default)]
    pub gitee_config: GiteeConfig,
}

impl SyncConfig {
    pub fn github(github_config: GithubConfig) -> Self {
        Self {
            provider: SyncProvider::Github,
            owner: String::new(),
            repo: String::new(),
            branch: String::new(),
            token: String::new(),
            github_config,
            gitee_config: GiteeConfig::default(),
        }
    }

    pub fn github_token(&self) -> &str {
        if self.github_config.token.is_empty() {
            &self.token
        } else {
            &self.github_config.token
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    #[serde(default = "default_github_api_base")]
    pub api_base: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub gist_id: Option<String>,
    #[serde(default = "default_gist_description")]
    pub gist_description: String,
}

fn default_github_api_base() -> String {
    "https://api.github.com".to_string()
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            api_base: default_github_api_base(),
            token: String::new(),
            gist_id: None,
            gist_description: default_gist_description(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeConfig {
    pub api_base: String,
    pub token: String,
    pub auto_sync: bool,
    #[serde(default = "default_sync_interval")]
    pub sync_interval: u64,
    #[serde(default)]
    pub gist_id: Option<String>,
    #[serde(default = "default_gist_description")]
    pub gist_description: String,
}

fn default_gist_description() -> String {
    "CopyTimeTrayConfig".to_string()
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
            gist_id: None,
            gist_description: "CopyTimeTrayConfig".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnippetWindowState {
    #[serde(default = "default_snippet_window_width")]
    pub width: i32,
    #[serde(default = "default_snippet_window_height")]
    pub height: i32,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default)]
    pub maximized: bool,
}

fn default_snippet_window_width() -> i32 {
    800
}

fn default_snippet_window_height() -> i32 {
    550
}

impl SnippetWindowState {
    pub fn path() -> Result<PathBuf, ConfigError> {
        Ok(Config::config_dir()?.join("snippet_window.json"))
    }

    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(&Self::path()?)
    }

    fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|error| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            ))
        })
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        Self::save_to(self, &Self::path()?)
    }

    fn save_to(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_vec_pretty(self).map_err(|error| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            ))
        })?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, content)?;
        fs::rename(&temporary, path)?;
        Ok(())
    }
}

impl Default for SnippetWindowState {
    fn default() -> Self {
        Self {
            width: default_snippet_window_width(),
            height: default_snippet_window_height(),
            x: None,
            y: None,
            maximized: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_true")]
    pub auto_sync: bool,
    #[serde(default = "default_sync_interval")]
    pub sync_interval: u64,
    #[serde(default = "default_true")]
    pub notify_on_success: bool,
    #[serde(default = "default_true")]
    pub notify_on_failure: bool,
    #[serde(default = "default_true")]
    pub notify_on_create: bool,
    #[serde(default = "default_snippet_hotkey")]
    pub snippet_hotkey: String,
}

fn default_true() -> bool {
    true
}

fn default_snippet_hotkey() -> String {
    "Ctrl+Shift+S".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            auto_sync: true,
            sync_interval: 300,
            notify_on_success: true,
            notify_on_failure: true,
            notify_on_create: true,
            snippet_hotkey: default_snippet_hotkey(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncDirection {
    Push,
    Pull,
}

impl fmt::Display for SyncDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncDirection::Push => write!(f, "Push"),
            SyncDirection::Pull => write!(f, "Pull"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncResultType {
    Success,
    Failed,
}

impl fmt::Display for SyncResultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncResultType::Success => write!(f, "成功"),
            SyncResultType::Failed => write!(f, "失败"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHistoryEntry {
    pub timestamp: DateTime<Local>,
    pub direction: SyncDirection,
    pub result: SyncResultType,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_sync_time: Option<DateTime<Local>>,
    pub pending_count: usize,
    pub last_sync_result: Option<SyncResultType>,
    #[serde(default)]
    pub history: Vec<SyncHistoryEntry>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            last_sync_time: None,
            pending_count: 0,
            last_sync_result: None,
            history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sync: Option<SyncConfig>,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub sync_status: SyncStatus,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sync: None,
            general: GeneralConfig::default(),
            sync_status: SyncStatus::default(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO error: {}", e),
            ConfigError::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(e: serde_yaml::Error) -> Self {
        ConfigError::Parse(e)
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Home directory not found",
            ))
        })?;
        Ok(home.join(".config").join("copy-time-tray"))
    }

    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.yaml"))
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config {
                sync: None,
                general: GeneralConfig::default(),
                sync_status: SyncStatus::default(),
            })
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)?;
        let content = serde_yaml::to_string(self)?;
        let path = Self::config_path()?;
        fs::write(&path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.sync.is_some()
    }

    pub fn sync_status_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("sync_status.json"))
    }

    pub fn load_sync_status() -> Result<SyncStatus, ConfigError> {
        let path = Self::sync_status_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let status: SyncStatus = serde_json::from_str(&content).map_err(|e| {
                ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))
            })?;
            Ok(status)
        } else {
            Ok(SyncStatus::default())
        }
    }

    pub fn save_sync_status(status: &SyncStatus) -> Result<(), ConfigError> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)?;
        let content = serde_json::to_string_pretty(status).map_err(|e| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;
        let path = Self::sync_status_path()?;
        fs::write(&path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    pub fn add_sync_history(&mut self, entry: SyncHistoryEntry) {
        self.sync_status.history.push(entry);
        // 只保留最近 20 条
        if self.sync_status.history.len() > 20 {
            self.sync_status.history = self
                .sync_status
                .history
                .split_off(self.sync_status.history.len() - 20);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_dir() -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("copy-time-tray");
        fs::create_dir_all(&config_dir).unwrap();
        (config_dir, dir)
    }

    #[test]
    fn github_legacy_repository_yaml_remains_readable() {
        let config: Config = serde_yaml::from_str(
            r#"
sync:
  provider: github
  owner: old-user
  repo: old-repo
  branch: main
  token: legacy-token
"#,
        )
        .unwrap();

        let sync = config.sync.unwrap();
        assert_eq!(sync.owner, "old-user");
        assert_eq!(sync.github_token(), "legacy-token");
        assert!(sync.github_config.gist_id.is_none());
    }

    #[test]
    fn new_github_config_omits_empty_legacy_repository_fields() {
        let sync = SyncConfig::github(GithubConfig {
            token: "new-token".into(),
            ..GithubConfig::default()
        });
        let yaml = serde_yaml::to_string(&Config {
            sync: Some(sync),
            ..Config::default()
        })
        .unwrap();

        assert!(!yaml.contains("owner:"));
        assert!(!yaml.contains("repo:"));
        assert!(!yaml.contains("branch:"));
        assert!(!yaml.contains("\n  token:"));
    }

    #[test]
    fn github_config_defaults_to_gist_api_and_description() {
        let github = GithubConfig::default();

        assert_eq!(github.api_base, "https://api.github.com");
        assert_eq!(github.gist_description, "CopyTimeTrayConfig");
    }

    #[test]
    fn github_token_prefers_provider_token_over_legacy_token() {
        let sync = SyncConfig::github(GithubConfig {
            token: "new-token".into(),
            ..GithubConfig::default()
        });
        let sync = SyncConfig {
            token: "legacy-token".into(),
            ..sync
        };

        assert_eq!(sync.github_token(), "new-token");
    }

    #[test]
    fn test_load_no_config_returns_empty() {
        let (_config_dir, _temp) = temp_config_dir();
        let config_str = r#"sync: null"#;
        let config: Config = serde_yaml::from_str(config_str).unwrap();
        assert!(!config.is_configured());
        assert!(config.sync.is_none());
    }

    #[test]
    fn test_parse_github_config() {
        let yaml = r#"
sync:
  provider: github
  owner: testuser
  repo: my-snippets
  branch: main
  token: ghp_xxxxx
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.is_configured());
        let sync = config.sync.unwrap();
        assert_eq!(sync.provider, SyncProvider::Github);
        assert_eq!(sync.owner, "testuser");
        assert_eq!(sync.repo, "my-snippets");
        assert_eq!(sync.branch, "main");
        assert_eq!(sync.token, "ghp_xxxxx");
    }

    #[test]
    fn test_parse_gitee_config() {
        let yaml = r#"
sync:
  provider: gitee
  owner: giteeuser
  repo: code-snippets
  branch: master
  token: xxx_token
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let sync = config.sync.unwrap();
        assert_eq!(sync.provider, SyncProvider::Gitee);
        assert_eq!(sync.owner, "giteeuser");
    }

    #[test]
    fn test_serialize_config() {
        let config = Config {
            sync: Some(SyncConfig {
                provider: SyncProvider::Github,
                owner: "user".into(),
                repo: "repo".into(),
                branch: "main".into(),
                token: "token".into(),
                github_config: GithubConfig::default(),
                gitee_config: GiteeConfig::default(),
            }),
            general: GeneralConfig::default(),
            sync_status: SyncStatus::default(),
        };
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("provider: github"));
        assert!(yaml.contains("owner: user"));
    }

    #[test]
    fn test_invalid_provider_fails() {
        let yaml = r#"
sync:
  provider: invalid
  owner: user
  repo: repo
  branch: main
  token: token
"#;
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_legacy_repository_fields_default_to_empty() {
        let yaml = r#"
sync:
  provider: github
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let sync = config.sync.unwrap();

        assert!(sync.owner.is_empty());
        assert!(sync.repo.is_empty());
        assert!(sync.branch.is_empty());
        assert!(sync.token.is_empty());
    }

    #[test]
    fn test_general_config_defaults() {
        let config = GeneralConfig::default();
        assert!(config.auto_sync);
        assert_eq!(config.sync_interval, 300);
        assert!(config.notify_on_success);
        assert!(config.notify_on_failure);
        assert!(config.notify_on_create);
    }

    #[test]
    fn legacy_general_config_uses_default_snippet_hotkey() {
        let yaml = r#"
 sync: null
 general:
   auto_sync: true
 "#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.general.snippet_hotkey, "Ctrl+Shift+S");
    }

    #[test]
    fn snippet_window_state_uses_an_independent_atomic_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("snippet_window.json");
        let state = SnippetWindowState {
            width: 1100,
            height: 700,
            x: Some(24),
            y: Some(48),
            maximized: true,
        };

        SnippetWindowState::save_to(&state, &path).unwrap();

        assert_eq!(SnippetWindowState::load_from(&path).unwrap(), state);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn snippet_hotkey_round_trips_through_yaml() {
        let mut config = Config::default();
        config.general.snippet_hotkey = "Alt+F8".to_string();

        let yaml = serde_yaml::to_string(&config).unwrap();
        let restored: Config = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(restored.general.snippet_hotkey, "Alt+F8");
    }

    #[test]
    fn test_sync_status_defaults() {
        let status = SyncStatus::default();
        assert!(status.last_sync_time.is_none());
        assert_eq!(status.pending_count, 0);
        assert!(status.history.is_empty());
    }

    #[test]
    fn test_parse_config_with_general() {
        let yaml = r#"
sync: null
general:
  auto_sync: false
  sync_interval: 600
  notify_on_success: false
  notify_on_failure: true
  notify_on_create: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.general.auto_sync);
        assert_eq!(config.general.sync_interval, 600);
        assert!(!config.general.notify_on_success);
    }
}
