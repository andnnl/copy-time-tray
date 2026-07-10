use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::snippet::config::{GiteeConfig, SyncConfig, SyncProvider};
use crate::snippet::gitee::types::{Gist, GistCreate, GistFileCreate, GistUpdate};
use crate::snippet::gitee::GiteeClient;
use crate::snippet::github::client::{find_reusable_gist_id, GithubClient};
use crate::snippet::github::error::GithubError;
use crate::snippet::github::types::GistFile as GithubGistFile;
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
            SyncError::Http(e) => write!(f, "HTTP error: {e}"),
            SyncError::Parse(e) => write!(f, "Parse error: {e}"),
            SyncError::Config(e) => write!(f, "Config error: {e}"),
            SyncError::Store(e) => write!(f, "Store error: {e}"),
        }
    }
}

impl std::error::Error for SyncError {}

/// Entry in tree.json representing a snippet or folder.
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

pub const TOMBSTONE_RETENTION_DAYS: i64 = 30;

/// Keeps live snippets and tombstones recent enough to propagate deletion.
pub fn snippets_for_tree(snippets: &[Snippet], now: DateTime<Utc>) -> Vec<&Snippet> {
    let cutoff = now - chrono::Duration::days(TOMBSTONE_RETENTION_DAYS);
    snippets
        .iter()
        .filter(|snippet| !snippet.deleted || snippet.update_date >= cutoff)
        .collect()
}

pub fn build_tree_entries(snippets: &[&Snippet]) -> Vec<TreeEntry> {
    snippets
        .iter()
        .map(|snippet| TreeEntry {
            id: snippet.id.clone(),
            parent_id: snippet.parent_id.clone(),
            name: snippet.name.clone(),
            entry_type: snippet.snippet_type.clone(),
            sort: snippet.sort,
            deleted: snippet.deleted,
            create_date: snippet.create_date,
            update_date: snippet.update_date,
            content: snippet.content.clone(),
        })
        .collect()
}

pub fn tree_entries_to_snippets(tree_entries: Vec<TreeEntry>) -> Vec<Snippet> {
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

pub fn pull(config: &SyncConfig) -> Result<Vec<Snippet>, SyncError> {
    match config.provider {
        SyncProvider::Gitee => pull_from_gitee_gist(config),
        SyncProvider::Github => pull_from_github_gist(config),
    }
}

fn pull_from_gitee_gist(config: &SyncConfig) -> Result<Vec<Snippet>, SyncError> {
    let gist_id = match &config.gitee_config.gist_id {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let gist = GiteeClient::new(config.gitee_config.clone())
        .get_gist(gist_id)
        .map_err(|error| SyncError::Http(error.to_string()))?;
    let tree_file = gist
        .files
        .get("tree.json")
        .ok_or_else(|| SyncError::Parse("tree.json not found in gist".into()))?;
    let tree_content = tree_file.content.as_deref().unwrap_or("");
    if tree_content.trim().is_empty() {
        return Ok(Vec::new());
    }
    decode_tree(tree_content)
}

fn pull_from_github_gist(config: &SyncConfig) -> Result<Vec<Snippet>, SyncError> {
    let gist_id = match github_gist_id_for_pull(config) {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let client = github_client(config);
    let gist = client
        .get_gist(gist_id)
        .map_err(|error| SyncError::Http(error.to_string()))?;
    let tree_file = gist
        .files
        .get("tree.json")
        .ok_or_else(|| SyncError::Parse("tree.json not found in GitHub Gist".into()))?;
    let raw_content = if tree_file.truncated {
        let raw_url = tree_file
            .raw_url
            .as_deref()
            .ok_or_else(|| SyncError::Parse("truncated tree.json has no raw_url".into()))?;
        Some(
            client
                .fetch_raw(raw_url)
                .map_err(|error| SyncError::Http(error.to_string()))?,
        )
    } else {
        None
    };
    let tree_content = github_tree_content(tree_file, raw_content.as_deref())?;
    if tree_content.trim().is_empty() {
        return Err(SyncError::Parse("tree.json is empty".into()));
    }
    decode_tree(&tree_content)
}

fn github_gist_id_for_pull(config: &SyncConfig) -> Option<&str> {
    config.github_config.gist_id.as_deref()
}

fn github_tree_content(
    file: &GithubGistFile,
    raw_content: Option<&str>,
) -> Result<String, SyncError> {
    if file.truncated {
        return raw_content
            .map(str::to_owned)
            .ok_or_else(|| SyncError::Parse("truncated tree.json has no raw content".into()));
    }
    file.content
        .clone()
        .ok_or_else(|| SyncError::Parse("tree.json content is missing".into()))
}

fn decode_tree(tree_content: &str) -> Result<Vec<Snippet>, SyncError> {
    let entries = serde_json::from_str(tree_content)
        .map_err(|error| SyncError::Parse(format!("failed to parse tree.json: {error}")))?;
    Ok(tree_entries_to_snippets(entries))
}

pub fn push(
    config: &SyncConfig,
    snippets: &[Snippet],
) -> Result<(Vec<String>, Option<String>), SyncError> {
    let gist_id = match config.provider {
        SyncProvider::Gitee => push_to_gitee_gist(config, snippets)?,
        SyncProvider::Github => push_to_github_gist(config, snippets)?,
    };
    Ok((Vec::new(), Some(gist_id)))
}

fn serialized_tree(snippets: &[Snippet]) -> Result<(Vec<&Snippet>, String), SyncError> {
    let active = snippets_for_tree(snippets, Utc::now());
    let content = serde_json::to_string_pretty(&build_tree_entries(&active))
        .map_err(|error| SyncError::Store(error.to_string()))?;
    Ok((active, content))
}

fn push_to_github_gist(config: &SyncConfig, snippets: &[Snippet]) -> Result<String, SyncError> {
    let (active, tree_json) = serialized_tree(snippets)?;
    let github_config = &config.github_config;
    if active.is_empty() && github_config.gist_id.is_none() {
        return Ok(String::new());
    }
    let client = github_client(config);
    let gist_id = match &github_config.gist_id {
        Some(id) => match client.get_gist(id) {
            Ok(_) => id.clone(),
            Err(GithubError::NotFound) => {
                select_or_create_github_gist(&client, github_config, &tree_json)?
            }
            Err(error) => return Err(SyncError::Http(error.to_string())),
        },
        None => select_or_create_github_gist(&client, github_config, &tree_json)?,
    };
    client
        .update_gist(&gist_id, &tree_json)
        .map_err(|error| SyncError::Http(error.to_string()))?;
    Ok(gist_id)
}

fn select_or_create_github_gist(
    client: &GithubClient,
    config: &crate::snippet::config::GithubConfig,
    tree_json: &str,
) -> Result<String, SyncError> {
    let gists = client
        .list_gists()
        .map_err(|error| SyncError::Http(error.to_string()))?;
    if let Some(id) = find_reusable_gist_id(&gists, &config.gist_description) {
        return Ok(id);
    }
    client
        .create_private_gist(&config.gist_description, tree_json)
        .map(|gist| gist.id)
        .map_err(|error| SyncError::Http(error.to_string()))
}

fn github_client(config: &SyncConfig) -> GithubClient {
    let mut github_config = config.github_config.clone();
    if github_config.token.is_empty() {
        github_config.token = config.github_token().to_owned();
    }
    GithubClient::new(github_config)
}

fn find_reusable_gitee_gist_id(gists: &[Gist], description: &str) -> Option<String> {
    gists
        .iter()
        .filter(|gist| gist.description.as_deref() == Some(description))
        .filter(|gist| gist.files.contains_key("tree.json"))
        .max_by_key(|gist| gist.updated_at)
        .map(|gist| gist.id.clone())
}

fn push_to_gitee_gist(config: &SyncConfig, snippets: &[Snippet]) -> Result<String, SyncError> {
    let (active, tree_json) = serialized_tree(snippets)?;
    let gitee_config = &config.gitee_config;
    let gist_id = match &gitee_config.gist_id {
        Some(id) => {
            let client = GiteeClient::new(gitee_config.clone());
            match client.get_gist(id) {
                Ok(_) => id.clone(),
                Err(_) => create_or_reuse_gitee_gist(gitee_config, &tree_json)?,
            }
        }
        None if active.is_empty() => return Ok(String::new()),
        None => create_or_reuse_gitee_gist(gitee_config, &tree_json)?,
    };
    let update =
        GistUpdate::new(Some(&gitee_config.gist_description)).add_file("tree.json", &tree_json);
    GiteeClient::new(gitee_config.clone())
        .update_gist(&gist_id, &update)
        .map_err(|error| SyncError::Http(error.to_string()))?;
    Ok(gist_id)
}

fn create_or_reuse_gitee_gist(config: &GiteeConfig, tree_json: &str) -> Result<String, SyncError> {
    let client = GiteeClient::new(config.clone());
    if let Ok(gists) = client.list_gists() {
        if let Some(id) = find_reusable_gitee_gist_id(&gists, &config.gist_description) {
            return Ok(id);
        }
    }
    let files = HashMap::from([("tree.json".to_owned(), GistFileCreate::new(Some(tree_json)))]);
    client
        .create_gist(&GistCreate {
            description: Some(&config.gist_description),
            files,
            public: false,
        })
        .map(|gist| gist.id)
        .map_err(|error| SyncError::Http(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippet::github::types::{Gist, GistFile};
    use crate::snippet::model::SnippetType;

    #[test]
    fn test_github_pull_without_gist_id_has_no_remote_target() {
        let config = SyncConfig::github(crate::snippet::config::GithubConfig::default());

        assert_eq!(github_gist_id_for_pull(&config), None);
    }

    #[test]
    fn test_github_tree_keeps_recent_deleted_tombstones_for_30_days() {
        let now = Utc::now();
        let active = Snippet::new_snippet("active".into(), "content".into(), None);
        let mut recent = Snippet::new_snippet("recent".into(), "content".into(), None);
        recent.deleted = true;
        recent.update_date = now - chrono::Duration::days(29);
        let mut expired = Snippet::new_snippet("expired".into(), "content".into(), None);
        expired.deleted = true;
        expired.update_date = now - chrono::Duration::days(31);
        assert_eq!(snippets_for_tree(&[active, recent, expired], now).len(), 2);
    }

    #[test]
    fn test_github_tree_json_decodes_tombstone_as_synced() {
        let snippets = decode_tree(r#"[{"id":"deleted","parent_id":null,"name":"deleted","type":"snippet","sort":0,"deleted":true,"create_date":"2026-01-01T00:00:00Z","update_date":"2026-01-02T00:00:00Z","content":"old"}]"#).unwrap();
        assert!(snippets[0].deleted);
        assert!(snippets[0].synced);
    }

    #[test]
    fn test_github_truncated_tree_uses_raw_content() {
        let file = GistFile {
            content: None,
            truncated: true,
            raw_url: Some("https://gist.githubusercontent.com/a/b/raw/tree.json".into()),
        };
        assert_eq!(github_tree_content(&file, Some("[]")).unwrap(), "[]");
    }

    #[test]
    fn test_github_truncated_tree_without_raw_content_is_parse_error() {
        let file = GistFile {
            content: None,
            truncated: true,
            raw_url: None,
        };
        assert!(matches!(
            github_tree_content(&file, None),
            Err(SyncError::Parse(_))
        ));
    }

    #[test]
    fn test_github_missing_content_is_parse_error() {
        let file = GistFile {
            content: None,
            truncated: false,
            raw_url: None,
        };
        assert!(matches!(
            github_tree_content(&file, None),
            Err(SyncError::Parse(_))
        ));
    }

    #[test]
    fn test_github_reuse_prefers_newest_matching_tree_gist() {
        let gist = |id: &str, updated: &str| Gist {
            id: id.into(),
            description: Some("CopyTimeTrayConfig".into()),
            files: HashMap::from([(
                "tree.json".into(),
                GistFile {
                    content: Some("[]".into()),
                    truncated: false,
                    raw_url: None,
                },
            )]),
            created_at: None,
            updated_at: Some(updated.parse().unwrap()),
        };
        assert_eq!(
            find_reusable_gist_id(
                &[
                    gist("old", "2026-01-01T00:00:00Z"),
                    gist("new", "2026-01-02T00:00:00Z")
                ],
                "CopyTimeTrayConfig"
            ),
            Some("new".into())
        );
    }

    #[test]
    fn test_github_private_create_body_contains_tree_file() {
        let value = serde_json::to_value(crate::snippet::github::types::GistCreate::private_tree(
            "CopyTimeTrayConfig",
            "[]",
        ))
        .unwrap();
        assert_eq!(value["public"], false);
        assert_eq!(value["files"]["tree.json"]["content"], "[]");
    }

    #[test]
    fn test_tree_entries_round_trip_content_and_folder() {
        let folder = Snippet::new_folder("folder".into(), None);
        let snippet =
            Snippet::new_snippet("snippet".into(), "echo ok".into(), Some(folder.id.clone()));
        let entries = build_tree_entries(&[&folder, &snippet]);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].entry_type, SnippetType::Folder);
        assert_eq!(entries[1].content.as_deref(), Some("echo ok"));
    }

    #[test]
    fn test_sync_module_contains_no_github_repository_endpoints() {
        let source = include_str!("sync.rs");
        let git_tree_endpoint = ["/git", "/trees/"].concat();
        let contents_endpoint = ["/contents", "/"].concat();
        assert!(!source.contains(&git_tree_endpoint));
        assert!(!source.contains(&contents_endpoint));
    }
}
