use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::snippet::model::{Snippet, SnippetType};

/// Termora 导出格式顶层结构
#[derive(Debug, Serialize, Deserialize)]
pub struct TermoraExport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exporter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "exportDate"
    )]
    pub export_date: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "exportDateHuman"
    )]
    pub export_date_human: Option<String>,
    pub snippets: Vec<TermoraSnippet>,
}

/// Termora 单条 snippet 格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermoraSnippet {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default, rename = "parentId")]
    pub parent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default)]
    pub sort: i32,
    #[serde(default, rename = "createDate")]
    pub create_date: i64,
    #[serde(default, rename = "updateDate")]
    pub update_date: i64,
}

#[derive(Debug)]
pub enum ImportExportError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    Format(String),
}

impl std::fmt::Display for ImportExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportExportError::Io(e) => write!(f, "IO 错误: {}", e),
            ImportExportError::Parse(e) => write!(f, "解析错误: {}", e),
            ImportExportError::Format(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ImportExportError {}

impl From<std::io::Error> for ImportExportError {
    fn from(e: std::io::Error) -> Self {
        ImportExportError::Io(e)
    }
}

impl From<serde_json::Error> for ImportExportError {
    fn from(e: serde_json::Error) -> Self {
        ImportExportError::Parse(e)
    }
}

/// 从 Termora JSON 文件导入 snippets
pub fn import_from_termora(path: &Path) -> Result<Vec<Snippet>, ImportExportError> {
    let content = fs::read_to_string(path)?;
    let export: TermoraExport = serde_json::from_str(&content)?;

    let mut result = Vec::new();
    for ts in export.snippets {
        let parent_id = if ts.parent_id == "0" || ts.parent_id.is_empty() {
            None
        } else {
            Some(ts.parent_id.clone())
        };

        let snippet_type = match ts.r#type.as_deref() {
            Some("Folder") => SnippetType::Folder,
            _ => SnippetType::Snippet,
        };

        let content = if snippet_type == SnippetType::Snippet {
            ts.snippet.clone()
        } else {
            None
        };

        let create_date = millis_to_datetime(ts.create_date);
        let update_date = millis_to_datetime(ts.update_date);

        let snippet = Snippet {
            id: ts.id.clone(),
            parent_id,
            name: ts.name.clone(),
            snippet_type,
            content,
            sort: ts.sort,
            deleted: false,
            synced: false,
            create_date,
            update_date,
            gitee_gist_id: None,
        };

        result.push(snippet);
    }

    Ok(result)
}

/// 将本地 snippets 导出为 Termora JSON 格式
pub fn export_to_termora(snippets: &[Snippet], path: &Path) -> Result<(), ImportExportError> {
    let mut termora_snippets = Vec::new();

    for s in snippets {
        if s.deleted {
            continue;
        }

        let parent_id = s.parent_id.clone().unwrap_or_else(|| "0".to_string());

        let r#type = match s.snippet_type {
            SnippetType::Folder => Some("Folder".to_string()),
            SnippetType::Snippet => None,
        };

        let ts = TermoraSnippet {
            id: s.id.clone(),
            name: s.name.clone(),
            snippet: s.content.clone(),
            parent_id,
            r#type,
            sort: s.sort,
            create_date: s.create_date.timestamp_millis(),
            update_date: s.update_date.timestamp_millis(),
        };

        termora_snippets.push(ts);
    }

    let export = TermoraExport {
        exporter: Some("copy-time-tray".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        export_date: Some(Utc::now().timestamp_millis()),
        export_date_human: Some(Utc::now().to_rfc3339()),
        snippets: termora_snippets,
    };

    let json = serde_json::to_string_pretty(&export)?;
    fs::write(path, json)?;
    Ok(())
}

fn millis_to_datetime(millis: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(millis)
        .single()
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_termora_json() -> &'static str {
        r#"{
            "exporter": "test",
            "version": "1.0.0",
            "exportDate": 1700000000000,
            "snippets": [
                {
                    "id": "folder-1",
                    "name": "测试文件夹",
                    "parentId": "0",
                    "type": "Folder",
                    "sort": 0,
                    "createDate": 1700000000000,
                    "updateDate": 1700000000000
                },
                {
                    "id": "snippet-1",
                    "name": "查看日志",
                    "snippet": "tail -f /var/log/syslog",
                    "parentId": "folder-1",
                    "sort": 0,
                    "createDate": 1700000000000,
                    "updateDate": 1700000000000
                },
                {
                    "id": "snippet-2",
                    "name": "根目录片段",
                    "snippet": "ls -la",
                    "parentId": "0",
                    "sort": 1,
                    "createDate": 1700000000000,
                    "updateDate": 1700000000000
                }
            ]
        }"#
    }

    #[test]
    fn test_import_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("termora.json");
        fs::write(&path, sample_termora_json()).unwrap();

        let snippets = import_from_termora(&path).unwrap();
        assert_eq!(snippets.len(), 3);

        let folder = snippets.iter().find(|s| s.id == "folder-1").unwrap();
        assert_eq!(folder.name, "测试文件夹");
        assert_eq!(folder.snippet_type, SnippetType::Folder);
        assert_eq!(folder.parent_id, None);
        assert_eq!(folder.content, None);
    }

    #[test]
    fn test_import_snippet_with_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("termora.json");
        fs::write(&path, sample_termora_json()).unwrap();

        let snippets = import_from_termora(&path).unwrap();
        let snippet = snippets.iter().find(|s| s.id == "snippet-1").unwrap();
        assert_eq!(snippet.name, "查看日志");
        assert_eq!(snippet.snippet_type, SnippetType::Snippet);
        assert_eq!(snippet.parent_id, Some("folder-1".to_string()));
        assert_eq!(snippet.content, Some("tail -f /var/log/syslog".to_string()));
    }

    #[test]
    fn test_import_root_snippet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("termora.json");
        fs::write(&path, sample_termora_json()).unwrap();

        let snippets = import_from_termora(&path).unwrap();
        let snippet = snippets.iter().find(|s| s.id == "snippet-2").unwrap();
        assert_eq!(snippet.parent_id, None);
    }

    #[test]
    fn test_export_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("termora.json");
        fs::write(&path, sample_termora_json()).unwrap();

        let imported = import_from_termora(&path).unwrap();

        let export_path = dir.path().join("export.json");
        export_to_termora(&imported, &export_path).unwrap();

        let reimported = import_from_termora(&export_path).unwrap();
        assert_eq!(reimported.len(), imported.len());

        for (orig, reimp) in imported.iter().zip(reimported.iter()) {
            assert_eq!(orig.id, reimp.id);
            assert_eq!(orig.name, reimp.name);
            assert_eq!(orig.snippet_type, reimp.snippet_type);
            assert_eq!(orig.content, reimp.content);
            assert_eq!(orig.parent_id, reimp.parent_id);
        }
    }

    #[test]
    fn test_export_skips_deleted() {
        let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        snippet.deleted = true;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        export_to_termora(&[snippet], &path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let export: TermoraExport = serde_json::from_str(&content).unwrap();
        assert_eq!(export.snippets.len(), 0);
    }

    #[test]
    fn test_export_folder_has_type() {
        let folder = Snippet::new_folder("目录".into(), None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        export_to_termora(&[folder], &path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let export: TermoraExport = serde_json::from_str(&content).unwrap();
        assert_eq!(export.snippets[0].r#type, Some("Folder".to_string()));
        assert!(export.snippets[0].snippet.is_none());
    }

    #[test]
    fn test_export_snippet_no_type() {
        let snippet = Snippet::new_snippet("命令".into(), "ls".into(), None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        export_to_termora(&[snippet], &path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let export: TermoraExport = serde_json::from_str(&content).unwrap();
        assert!(export.snippets[0].r#type.is_none());
        assert_eq!(export.snippets[0].snippet, Some("ls".to_string()));
    }

    #[test]
    fn test_import_file_not_found() {
        let result = import_from_termora(Path::new("/nonexistent/file.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_import_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "not json").unwrap();

        let result = import_from_termora(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_millis_to_datetime() {
        use chrono::Datelike;
        let dt = millis_to_datetime(1700000000000);
        assert_eq!(dt.year(), 2023);
    }

    #[test]
    fn test_import_real_termora_file() {
        let path = Path::new("/home/andnnl/Documents/Termora.json");
        if !path.exists() {
            eprintln!("Termora.json not found, skipping real file test");
            return;
        }
        let snippets = import_from_termora(path).expect("Failed to import real Termora.json");
        assert!(
            !snippets.is_empty(),
            "Should have imported at least one snippet"
        );

        let folders: Vec<_> = snippets
            .iter()
            .filter(|s| s.snippet_type == SnippetType::Folder)
            .collect();
        let snippet_items: Vec<_> = snippets
            .iter()
            .filter(|s| s.snippet_type == SnippetType::Snippet)
            .collect();

        eprintln!("=== Real Termora.json Import Results ===");
        eprintln!("Total: {} items", snippets.len());
        eprintln!("Folders: {}", folders.len());
        eprintln!("Snippets: {}", snippet_items.len());

        for s in &snippets {
            let type_str = match s.snippet_type {
                SnippetType::Folder => "[Folder]",
                SnippetType::Snippet => "[Snippet]",
            };
            let parent = s.parent_id.as_deref().unwrap_or("root");
            eprintln!("  {} {} (parent: {})", type_str, s.name, parent);
        }

        // Verify no empty names
        for s in &snippets {
            assert!(!s.name.is_empty(), "Snippet should have a non-empty name");
        }

        // Verify folders have no content
        for f in &folders {
            assert!(
                f.content.is_none(),
                "Folder '{}' should have no content",
                f.name
            );
        }

        // Verify snippets have content
        for s in &snippet_items {
            assert!(
                s.content.is_some(),
                "Snippet '{}' should have content",
                s.name
            );
        }
    }
}
