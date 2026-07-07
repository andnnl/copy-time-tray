use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::snippet::model::Snippet;

#[derive(Debug, Serialize, Deserialize)]
pub struct SnippetFile {
    pub version: u32,
    pub snippets: Vec<Snippet>,
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Serde(serde_json::Error),
    NotFound(String),
    Duplicate(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "IO error: {}", e),
            StoreError::Serde(e) => write!(f, "Serialization error: {}", e),
            StoreError::NotFound(id) => write!(f, "Snippet not found: {}", id),
            StoreError::Duplicate(name) => write!(f, "Duplicate: {}", name),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Serde(e)
    }
}

pub struct SnippetStore {
    file_path: PathBuf,
    data: SnippetFile,
}

impl SnippetStore {
    pub fn new(file_path: PathBuf) -> Result<Self, StoreError> {
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let data: SnippetFile = serde_json::from_str(&content)?;
            Ok(Self { file_path, data })
        } else {
            let data = SnippetFile {
                version: 1,
                snippets: Vec::new(),
            };
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let store = Self { file_path, data };
            store.save()?;
            Ok(store)
        }
    }

    pub fn save(&self) -> Result<(), StoreError> {
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.file_path, content)?;
        Ok(())
    }

    pub fn list(&self) -> &[Snippet] {
        &self.data.snippets
    }

    pub fn add(&mut self, snippet: Snippet) -> Result<(), StoreError> {
        if self.data.snippets.iter().any(|s| {
            !s.deleted
                && s.snippet_type == snippet.snippet_type
                && s.parent_id == snippet.parent_id
                && s.name == snippet.name
        }) {
            return Err(StoreError::Duplicate(snippet.name));
        }
        self.data.snippets.push(snippet);
        self.save()
    }

    pub fn get(&self, id: &str) -> Option<&Snippet> {
        self.data.snippets.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Snippet> {
        self.data.snippets.iter_mut().find(|s| s.id == id)
    }

    pub fn update(
        &mut self,
        id: &str,
        f: impl FnOnce(&mut Snippet),
    ) -> Result<(), StoreError> {
        if let Some(snippet) = self.get_mut(id) {
            f(snippet);
            snippet.update_date = chrono::Utc::now();
            snippet.synced = false;
            self.save()
        } else {
            Err(StoreError::NotFound(id.to_string()))
        }
    }

    pub fn delete(&mut self, id: &str) -> Result<(), StoreError> {
        self.update(id, |s| s.deleted = true)
    }

    pub fn hard_delete(&mut self, id: &str) -> Result<(), StoreError> {
        let len_before = self.data.snippets.len();
        self.data.snippets.retain(|s| s.id != id);
        if self.data.snippets.len() == len_before {
            return Err(StoreError::NotFound(id.to_string()));
        }
        self.save()
    }

    pub fn pending_sync(&self) -> Vec<&Snippet> {
        self.data.snippets.iter().filter(|s| !s.synced).collect()
    }

    pub fn load_from_sync(&mut self, remote_snippets: Vec<Snippet>) -> Result<(), StoreError> {
        use std::collections::HashMap;

        let mut by_id: HashMap<String, usize> = self.data.snippets.iter().enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();

        for remote in remote_snippets {
            // First try matching by gitee_gist_id (for folders synced via gist)
            if let Some(gist_id) = &remote.gitee_gist_id {
                if let Some(existing) = self.data.snippets.iter_mut().find(|s| {
                    s.gitee_gist_id.as_deref() == Some(gist_id)
                }) {
                    if remote.update_date > existing.update_date {
                        *existing = remote;
                    }
                    continue;
                }
            }

            // Fall back to matching by id
            if let Some(idx) = by_id.get(&remote.id) {
                let existing = &mut self.data.snippets[*idx];
                if remote.update_date > existing.update_date {
                    *existing = remote;
                }
            } else {
                self.data.snippets.push(remote);
            }
        }
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (SnippetStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snippets.json");
        (SnippetStore::new(path).unwrap(), dir)
    }

    #[test]
    fn test_deserialize_real_file() {
        let path = std::path::PathBuf::from("/home/andnnl/.config/copy-time-tray/snippets.json");
        if !path.exists() {
            eprintln!("Skipping: file does not exist");
            return;
        }
        let content = fs::read_to_string(&path).expect("read file");
        eprintln!("Content (first 200): {}", &content[..content.len().min(200)]);
        
        let result: Result<SnippetFile, _> = serde_json::from_str(&content);
        match result {
            Ok(file) => {
                eprintln!("Parsed OK: {} snippets", file.snippets.len());
                for (i, s) in file.snippets.iter().enumerate() {
                    eprintln!("  [{}] name='{}' type={:?} deleted={}", i, s.name, s.snippet_type, s.deleted);
                }
            }
            Err(e) => {
                eprintln!("Parse FAILED: {}", e);
                panic!("Deserialization failed");
            }
        }
    }

    #[test]
    fn test_new_store_creates_empty_file() {
        let (store, _dir) = temp_store();
        assert_eq!(store.list().len(), 0);
        assert!(store.file_path.exists());
    }

    #[test]
    fn test_add_snippet() {
        let (mut store, _dir) = temp_store();
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        store.add(snippet).unwrap();
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].name, "test");
    }

    #[test]
    fn test_get_snippet_by_id() {
        let (mut store, _dir) = temp_store();
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let id = snippet.id.clone();
        store.add(snippet).unwrap();
        let found = store.get(&id).unwrap();
        assert_eq!(found.name, "test");
    }

    #[test]
    fn test_update_snippet() {
        let (mut store, _dir) = temp_store();
        let snippet = Snippet::new_snippet("old".into(), "old content".into(), None);
        let id = snippet.id.clone();
        store.add(snippet).unwrap();
        store
            .update(&id, |s| {
                s.name = "new".into();
                s.content = Some("new content".into());
            })
            .unwrap();
        let updated = store.get(&id).unwrap();
        assert_eq!(updated.name, "new");
        assert_eq!(updated.content, Some("new content".into()));
    }

    #[test]
    fn test_delete_is_soft() {
        let (mut store, _dir) = temp_store();
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let id = snippet.id.clone();
        store.add(snippet).unwrap();
        store.delete(&id).unwrap();
        let found = store.get(&id).unwrap();
        assert!(found.deleted);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn test_hard_delete_removes_from_list() {
        let (mut store, _dir) = temp_store();
        let snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        let id = snippet.id.clone();
        store.add(snippet).unwrap();
        store.hard_delete(&id).unwrap();
        assert!(store.get(&id).is_none());
        assert_eq!(store.list().len(), 0);
    }

    #[test]
    fn test_pending_sync_returns_unsynced() {
        let (mut store, _dir) = temp_store();
        let mut s1 = Snippet::new_snippet("s1".into(), "c1".into(), None);
        s1.synced = false;
        let mut s2 = Snippet::new_snippet("s2".into(), "c2".into(), None);
        s2.synced = true;
        store.add(s1).unwrap();
        store.add(s2).unwrap();
        let pending = store.pending_sync();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].name, "s1");
    }

    #[test]
    fn test_load_from_sync_lww_newer_wins() {
        let (mut store, _dir) = temp_store();
        let mut local = Snippet::new_snippet("test".into(), "local".into(), None);
        local.id = "same-id".into();
        local.update_date = chrono::Utc::now() - chrono::Duration::minutes(10);
        store.add(local).unwrap();

        let mut remote = Snippet::new_snippet("test".into(), "remote".into(), None);
        remote.id = "same-id".into();
        remote.update_date = chrono::Utc::now();
        remote.synced = true;

        store.load_from_sync(vec![remote]).unwrap();
        let result = store.get("same-id").unwrap();
        assert_eq!(result.content, Some("remote".into()));
    }

    #[test]
    fn test_update_sets_synced_false() {
        let (mut store, _dir) = temp_store();
        let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        snippet.synced = true;
        let id = snippet.id.clone();
        store.add(snippet).unwrap();
        store.update(&id, |s| s.name = "changed".into()).unwrap();
        let updated = store.get(&id).unwrap();
        assert!(!updated.synced);
    }

    #[test]
    fn test_update_not_found_returns_error() {
        let (mut store, _dir) = temp_store();
        let result = store.update("nonexistent", |_| {});
        assert!(matches!(result, Err(StoreError::NotFound(_))));
    }

    #[test]
    fn test_persist_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snippets.json");
        {
            let mut store = SnippetStore::new(path.clone()).unwrap();
            let snippet = Snippet::new_snippet("persist".into(), "data".into(), None);
            store.add(snippet).unwrap();
        }
        let reloaded = SnippetStore::new(path).unwrap();
        assert_eq!(reloaded.list().len(), 1);
        assert_eq!(reloaded.list()[0].name, "persist");
    }
}
