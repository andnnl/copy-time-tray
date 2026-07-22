use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::snippet::model::{Snippet, SnippetType};

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
    NotFavoriteable(String),
    Duplicate(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "IO error: {}", e),
            StoreError::Serde(e) => write!(f, "Serialization error: {}", e),
            StoreError::NotFound(id) => write!(f, "Snippet not found: {}", id),
            StoreError::NotFavoriteable(id) => write!(f, "Snippet cannot be favorited: {}", id),
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

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    pub skipped_existing_id: usize,
    pub skipped_duplicate: usize,
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

    pub fn import_batch(&mut self, snippets: Vec<Snippet>) -> Result<ImportSummary, StoreError> {
        use std::collections::HashSet;

        let mut ids: HashSet<String> = self.data.snippets.iter().map(|s| s.id.clone()).collect();
        let mut names: HashSet<(String, SnippetType, Option<String>)> = self
            .data
            .snippets
            .iter()
            .filter(|s| !s.deleted)
            .map(|s| (s.name.clone(), s.snippet_type.clone(), s.parent_id.clone()))
            .collect();
        let mut summary = ImportSummary::default();

        for snippet in snippets {
            if ids.contains(&snippet.id) {
                summary.skipped_existing_id += 1;
                continue;
            }

            let duplicate_key = (
                snippet.name.clone(),
                snippet.snippet_type.clone(),
                snippet.parent_id.clone(),
            );
            if !snippet.deleted && names.contains(&duplicate_key) {
                summary.skipped_duplicate += 1;
                continue;
            }

            ids.insert(snippet.id.clone());
            if !snippet.deleted {
                names.insert(duplicate_key);
            }
            self.data.snippets.push(snippet);
            summary.added += 1;
        }

        if summary.added > 0 {
            self.save()?;
        }

        Ok(summary)
    }

    pub fn get(&self, id: &str) -> Option<&Snippet> {
        self.data.snippets.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Snippet> {
        self.data.snippets.iter_mut().find(|s| s.id == id)
    }

    pub fn update(&mut self, id: &str, f: impl FnOnce(&mut Snippet)) -> Result<(), StoreError> {
        if let Some(snippet) = self.get_mut(id) {
            f(snippet);
            snippet.update_date = chrono::Utc::now();
            snippet.synced = false;
            self.save()
        } else {
            Err(StoreError::NotFound(id.to_string()))
        }
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<(), StoreError> {
        match self.get(id) {
            None => Err(StoreError::NotFound(id.to_string())),
            Some(snippet) if snippet.deleted || snippet.snippet_type != SnippetType::Snippet => {
                Err(StoreError::NotFavoriteable(id.to_string()))
            }
            Some(_) => self.update(id, |snippet| snippet.favorite = favorite),
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

    pub fn mark_synced(&mut self, ids: &[String]) -> Result<(), StoreError> {
        for id in ids {
            if let Some(snippet) = self.get_mut(id) {
                snippet.synced = true;
            }
        }
        self.save()
    }

    pub fn mark_github_synced(
        &mut self,
        successes: &[crate::snippet::sync::GithubPushSuccess],
    ) -> Result<(), StoreError> {
        for success in successes {
            if let Some(snippet) = self.get_mut(&success.id) {
                if snippet.update_date == success.expected_update_date {
                    snippet.synced = true;
                    snippet.github_remote_path = Some(success.path.clone());
                }
            }
        }
        self.save()
    }

    pub fn load_from_sync(&mut self, remote_snippets: Vec<Snippet>) -> Result<(), StoreError> {
        use std::collections::HashMap;

        let mut newest_remote_by_id: HashMap<String, Snippet> = HashMap::new();
        for remote in remote_snippets {
            match newest_remote_by_id.get(&remote.id) {
                Some(current) if current.update_date >= remote.update_date => {}
                _ => {
                    newest_remote_by_id.insert(remote.id.clone(), remote);
                }
            }
        }

        let mut by_id: HashMap<String, usize> = self
            .data
            .snippets
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();

        for remote in newest_remote_by_id.into_values() {
            if let Some(idx) = by_id.get(&remote.id) {
                let existing = &mut self.data.snippets[*idx];
                if remote.update_date > existing.update_date {
                    *existing = remote;
                }
            } else {
                let index = self.data.snippets.len();
                by_id.insert(remote.id.clone(), index);
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
        eprintln!(
            "Content (first 200): {}",
            &content[..content.len().min(200)]
        );

        let result: Result<SnippetFile, _> = serde_json::from_str(&content);
        match result {
            Ok(file) => {
                eprintln!("Parsed OK: {} snippets", file.snippets.len());
                for (i, s) in file.snippets.iter().enumerate() {
                    eprintln!(
                        "  [{}] name='{}' type={:?} deleted={}",
                        i, s.name, s.snippet_type, s.deleted
                    );
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
    fn test_mark_synced_preserves_update_date_and_sets_synced_true() {
        let (mut store, _dir) = temp_store();
        let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        snippet.id = "s1".into();
        snippet.synced = false;
        snippet.update_date = chrono::Utc::now() - chrono::Duration::minutes(30);
        let original_update_date = snippet.update_date;
        store.add(snippet).unwrap();

        store.mark_synced(&["s1".to_string()]).unwrap();

        let updated = store.get("s1").unwrap();
        assert!(updated.synced);
        assert_eq!(updated.update_date, original_update_date);
    }

    #[test]
    fn test_load_from_sync_merges_whole_tree_by_item_update_date() {
        let (mut store, _dir) = temp_store();
        let base = chrono::Utc::now() - chrono::Duration::hours(1);

        let mut local_newer = Snippet::new_snippet("local-newer".into(), "local wins".into(), None);
        local_newer.id = "same-local-newer".into();
        local_newer.update_date = base + chrono::Duration::minutes(20);
        store.add(local_newer).unwrap();

        let mut remote_newer_local =
            Snippet::new_snippet("remote-newer".into(), "old local".into(), None);
        remote_newer_local.id = "same-remote-newer".into();
        remote_newer_local.update_date = base + chrono::Duration::minutes(10);
        store.add(remote_newer_local).unwrap();

        let mut remote_older =
            Snippet::new_snippet("local-newer".into(), "remote loses".into(), None);
        remote_older.id = "same-local-newer".into();
        remote_older.update_date = base + chrono::Duration::minutes(5);
        remote_older.synced = true;

        let mut remote_newer =
            Snippet::new_snippet("remote-newer".into(), "remote wins".into(), None);
        remote_newer.id = "same-remote-newer".into();
        remote_newer.update_date = base + chrono::Duration::minutes(30);
        remote_newer.synced = true;

        let mut remote_only = Snippet::new_snippet("remote-only".into(), "new remote".into(), None);
        remote_only.id = "remote-only".into();
        remote_only.update_date = base + chrono::Duration::minutes(40);
        remote_only.synced = true;

        store
            .load_from_sync(vec![remote_older, remote_newer, remote_only])
            .unwrap();

        assert_eq!(
            store.get("same-local-newer").unwrap().content,
            Some("local wins".into())
        );
        assert_eq!(
            store.get("same-remote-newer").unwrap().content,
            Some("remote wins".into())
        );
        assert_eq!(
            store.get("remote-only").unwrap().content,
            Some("new remote".into())
        );
    }

    #[test]
    fn test_load_from_sync_remote_newer_deleted_tombstone_deletes_local() {
        let (mut store, _dir) = temp_store();
        let base = chrono::Utc::now() - chrono::Duration::hours(1);

        let mut local = Snippet::new_snippet("local active".into(), "local content".into(), None);
        local.id = "same-id".into();
        local.deleted = false;
        local.update_date = base;
        store.add(local).unwrap();

        let mut remote =
            Snippet::new_snippet("remote deleted".into(), "remote content".into(), None);
        remote.id = "same-id".into();
        remote.deleted = true;
        remote.update_date = base + chrono::Duration::minutes(10);
        remote.synced = true;

        store.load_from_sync(vec![remote]).unwrap();

        let result = store.get("same-id").unwrap();
        assert!(result.deleted);
        assert_eq!(result.name, "remote deleted");
    }

    #[test]
    fn test_load_from_sync_local_newer_deleted_tombstone_is_not_resurrected() {
        let (mut store, _dir) = temp_store();
        let base = chrono::Utc::now() - chrono::Duration::hours(1);

        let mut local = Snippet::new_snippet("local deleted".into(), "local content".into(), None);
        local.id = "same-id".into();
        local.deleted = true;
        local.update_date = base + chrono::Duration::minutes(10);
        store.add(local).unwrap();

        let mut remote =
            Snippet::new_snippet("remote active".into(), "remote content".into(), None);
        remote.id = "same-id".into();
        remote.deleted = false;
        remote.update_date = base;
        remote.synced = true;

        store.load_from_sync(vec![remote]).unwrap();

        let result = store.get("same-id").unwrap();
        assert!(result.deleted);
        assert_eq!(result.name, "local deleted");
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

    #[test]
    fn test_import_batch_adds_unique_items_and_skips_id_and_name_conflicts() {
        let (mut store, _dir) = temp_store();
        let mut existing = Snippet::new_snippet("existing".into(), "one".into(), None);
        existing.id = "existing-id".into();
        store.add(existing).unwrap();

        let mut same_id = Snippet::new_snippet("replacement".into(), "two".into(), None);
        same_id.id = "existing-id".into();
        let duplicate_name = Snippet::new_snippet("existing".into(), "three".into(), None);
        let mut new_item = Snippet::new_snippet("new".into(), "four".into(), None);
        new_item.id = "new-id".into();

        let summary = store
            .import_batch(vec![same_id, duplicate_name, new_item])
            .unwrap();

        assert_eq!(summary.added, 1);
        assert_eq!(summary.skipped_existing_id, 1);
        assert_eq!(summary.skipped_duplicate, 1);
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn test_import_batch_skips_duplicates_within_the_same_batch() {
        let (mut store, _dir) = temp_store();
        let mut first = Snippet::new_snippet("same name".into(), "one".into(), None);
        first.id = "first-id".into();
        let mut duplicate = Snippet::new_snippet("same name".into(), "two".into(), None);
        duplicate.id = "second-id".into();

        let summary = store.import_batch(vec![first, duplicate]).unwrap();

        assert_eq!(summary.added, 1);
        assert_eq!(summary.skipped_existing_id, 0);
        assert_eq!(summary.skipped_duplicate, 1);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn test_import_batch_does_not_reserve_id_for_name_conflict() {
        let (mut store, _dir) = temp_store();
        store
            .add(Snippet::new_snippet("existing".into(), "one".into(), None))
            .unwrap();
        let mut conflicting_name = Snippet::new_snippet("existing".into(), "two".into(), None);
        conflicting_name.id = "reused-id".into();
        let mut later_unique_item = Snippet::new_snippet("new".into(), "three".into(), None);
        later_unique_item.id = "reused-id".into();

        let summary = store
            .import_batch(vec![conflicting_name, later_unique_item])
            .unwrap();

        assert_eq!(summary.added, 1);
        assert_eq!(summary.skipped_existing_id, 0);
        assert_eq!(summary.skipped_duplicate, 1);
        assert!(store.get("reused-id").is_some());
    }

    #[test]
    fn set_favorite_persists_marks_pending_and_updates_timestamp() {
        let (mut store, dir) = temp_store();
        let mut snippet = Snippet::new_snippet("test".into(), "content".into(), None);
        snippet.synced = true;
        snippet.update_date = chrono::Utc::now() - chrono::Duration::minutes(1);
        let before = snippet.update_date;
        let id = snippet.id.clone();
        store.add(snippet).unwrap();

        store.set_favorite(&id, true).unwrap();

        let updated = store.get(&id).unwrap();
        assert!(updated.favorite);
        assert!(!updated.synced);
        assert!(updated.update_date > before);
        drop(store);

        let reloaded = SnippetStore::new(dir.path().join("snippets.json")).unwrap();
        assert!(reloaded.get(&id).unwrap().favorite);
    }

    #[test]
    fn set_favorite_rejects_folder_and_deleted_snippet() {
        let (mut store, _dir) = temp_store();
        let folder = Snippet::new_folder("folder".into(), None);
        let folder_id = folder.id.clone();
        let mut deleted = Snippet::new_snippet("deleted".into(), "content".into(), None);
        deleted.deleted = true;
        let deleted_id = deleted.id.clone();
        store.add(folder).unwrap();
        store.add(deleted).unwrap();

        assert!(matches!(
            store.set_favorite(&folder_id, true),
            Err(StoreError::NotFavoriteable(_))
        ));
        assert!(matches!(
            store.set_favorite(&deleted_id, true),
            Err(StoreError::NotFavoriteable(_))
        ));
    }

    #[test]
    fn newer_remote_favorite_wins_under_lww() {
        let (mut store, _dir) = temp_store();
        let base = chrono::Utc::now() - chrono::Duration::hours(1);
        let mut local = Snippet::new_snippet("same".into(), "content".into(), None);
        local.id = "same-id".into();
        local.favorite = false;
        local.update_date = base;
        store.add(local).unwrap();

        let mut remote = Snippet::new_snippet("same".into(), "content".into(), None);
        remote.id = "same-id".into();
        remote.favorite = true;
        remote.synced = true;
        remote.update_date = base + chrono::Duration::minutes(1);
        store.load_from_sync(vec![remote]).unwrap();

        assert!(store.get("same-id").unwrap().favorite);
    }

    #[test]
    fn load_from_sync_collapses_duplicate_remote_stable_ids_before_append() {
        let (mut store, _dir) = temp_store();
        let base = chrono::Utc::now() - chrono::Duration::hours(1);
        let mut older = Snippet::new_snippet("old path".into(), "old".into(), None);
        older.id = "stable-id".into();
        older.update_date = base;
        let mut newer = Snippet::new_snippet("new path".into(), "new".into(), None);
        newer.id = "stable-id".into();
        newer.update_date = base + chrono::Duration::minutes(1);

        store.load_from_sync(vec![older, newer]).unwrap();

        assert_eq!(
            store
                .list()
                .iter()
                .filter(|item| item.id == "stable-id")
                .count(),
            1
        );
        assert_eq!(store.get("stable-id").unwrap().name, "new path");
    }

    #[test]
    fn test_load_from_sync_matches_by_id_only() {
        let (mut store, _dir) = temp_store();

        let mut local = Snippet::new_snippet("old".into(), "local-content".into(), None);
        local.id = "s1".into();
        local.gitee_gist_id = Some("gist-xxx".into());
        local.update_date = chrono::Utc::now() - chrono::Duration::minutes(10);
        store.add(local).unwrap();

        let mut remote = Snippet::new_snippet("new".into(), "remote-content".into(), None);
        remote.id = "s1".into();
        remote.gitee_gist_id = Some("gist-yyy".into());
        remote.update_date = chrono::Utc::now();
        remote.synced = true;

        store.load_from_sync(vec![remote]).unwrap();

        let result = store.get("s1").unwrap();
        assert_eq!(result.content, Some("remote-content".into()));
        assert_eq!(result.name, "new");
    }
}
