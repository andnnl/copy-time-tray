use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnippetType {
    Folder,
    Snippet,
}

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
    #[serde(default)]
    pub favorite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_remote_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gitee_gist_id: Option<String>,
}

impl Snippet {
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
            favorite: false,
            github_remote_path: None,
            gitee_gist_id: None,
        }
    }

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
            favorite: false,
            github_remote_path: None,
            gitee_gist_id: None,
        }
    }

    pub fn remote_id(&self, relative_path: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(relative_path.as_bytes());
        let result = hasher.finalize();
        let hex = format!("{:x}", result);
        hex[..16].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_new_snippet_has_correct_type() {
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        assert_eq!(snippet.snippet_type, SnippetType::Snippet);
        assert_eq!(snippet.content, Some("content".into()));
        assert!(!snippet.deleted);
        assert!(!snippet.synced);
    }

    #[test]
    fn test_new_folder_has_no_content() {
        let folder = Snippet::new_folder("folder".into(), None);
        assert_eq!(folder.snippet_type, SnippetType::Folder);
        assert_eq!(folder.content, None);
    }

    #[test]
    fn test_remote_id_is_deterministic() {
        let snippet = Snippet::new_snippet("查看日志".into(), "content".into(), None);
        let id1 = snippet.remote_id("snippets/查看日志.yaml");
        let id2 = snippet.remote_id("snippets/查看日志.yaml");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_remote_id_differs_by_path() {
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let id1 = snippet.remote_id("snippets/a.yaml");
        let id2 = snippet.remote_id("snippets/b.yaml");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_snippet_serialize_deserialize() {
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let json = serde_json::to_string(&snippet).unwrap();
        let deserialized: Snippet = serde_json::from_str(&json).unwrap();
        assert_eq!(snippet.id, deserialized.id);
        assert_eq!(snippet.name, deserialized.name);
        assert_eq!(snippet.snippet_type, deserialized.snippet_type);
    }

    #[test]
    fn test_folder_serialize_has_null_content() {
        let folder = Snippet::new_folder("folder".into(), None);
        let json = serde_json::to_string(&folder).unwrap();
        assert!(json.contains("\"content\":null"));
        let deserialized: Snippet = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, None);
    }

    #[test]
    fn test_snippet_type_serde_tag() {
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let json = serde_json::to_string(&snippet).unwrap();
        assert!(json.contains("\"type\":\"snippet\""));

        let folder = Snippet::new_folder("folder".into(), None);
        let json = serde_json::to_string(&folder).unwrap();
        assert!(json.contains("\"type\":\"folder\""));
    }

    #[test]
    fn test_deserialize_from_json_string() {
        let json = r#"{
            "id": "test-uuid",
            "parent_id": null,
            "name": "命令",
            "type": "snippet",
            "content": "ls -la",
            "sort": 5,
            "deleted": false,
            "synced": true,
            "create_date": "2026-06-17T10:00:00Z",
            "update_date": "2026-06-17T10:30:00Z"
        }"#;
        let snippet: Snippet = serde_json::from_str(json).unwrap();
        assert_eq!(snippet.id, "test-uuid");
        assert_eq!(snippet.name, "命令");
        assert_eq!(snippet.snippet_type, SnippetType::Snippet);
        assert_eq!(snippet.content, Some("ls -la".into()));
        assert_eq!(snippet.sort, 5);
        assert!(snippet.synced);
        assert_eq!(
            snippet.update_date,
            Utc.with_ymd_and_hms(2026, 6, 17, 10, 30, 0)
                .single()
                .unwrap()
        );
    }

    #[test]
    fn test_deserialize_folder_from_json() {
        let json = r#"{
            "id": "folder-uuid",
            "parent_id": null,
            "name": "常用命令",
            "type": "folder",
            "content": null,
            "sort": 0,
            "deleted": false,
            "synced": true,
            "create_date": "2026-06-17T10:00:00Z",
            "update_date": "2026-06-17T10:00:00Z"
        }"#;
        let folder: Snippet = serde_json::from_str(json).unwrap();
        assert_eq!(folder.snippet_type, SnippetType::Folder);
        assert_eq!(folder.content, None);
    }

    #[test]
    fn new_items_start_unfavorited_without_github_path() {
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let folder = Snippet::new_folder("folder".into(), None);

        assert!(!snippet.favorite);
        assert_eq!(snippet.github_remote_path, None);
        assert!(!folder.favorite);
        assert_eq!(folder.github_remote_path, None);
    }

    #[test]
    fn legacy_json_defaults_favorite_and_github_path() {
        let json = r#"{
            "id":"legacy","parent_id":null,"name":"legacy","type":"snippet",
            "content":"echo legacy","sort":0,"deleted":false,"synced":true,
            "create_date":"2026-07-16T00:00:00Z","update_date":"2026-07-16T00:00:00Z"
        }"#;

        let snippet: Snippet = serde_json::from_str(json).unwrap();

        assert!(!snippet.favorite);
        assert_eq!(snippet.github_remote_path, None);
    }

    #[test]
    fn model_round_trip_keeps_favorite_and_github_path() {
        let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        snippet.favorite = true;
        snippet.github_remote_path = Some("snippets/test.yaml".into());

        let restored: Snippet =
            serde_json::from_str(&serde_json::to_string(&snippet).unwrap()).unwrap();

        assert!(restored.favorite);
        assert_eq!(
            restored.github_remote_path.as_deref(),
            Some("snippets/test.yaml")
        );
    }
}
