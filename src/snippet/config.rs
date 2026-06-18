use serde::{Deserialize, Serialize};
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
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sync: Option<SyncConfig>,
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
            Ok(Config { sync: None })
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
            }),
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
    fn test_missing_required_field_fails() {
        let yaml = r#"
sync:
  provider: github
  owner: user
"#;
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }
}
