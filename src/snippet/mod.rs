pub mod config;
pub mod gitee;
pub mod import_export;
pub mod model;
pub mod store;
pub mod sync;
pub mod ui;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};

use config::Config;
use model::Snippet;
use store::SnippetStore;
use ui::UiResponse;

/// Debounce delay in milliseconds before triggering sync after snippet changes
const DEBOUNCE_MS: u64 = 5000;

fn delayed_auto_sync_delay_ms() -> u64 {
    DEBOUNCE_MS
}

fn should_auto_sync(config: &Config) -> bool {
    config.sync.is_some() && config.general.auto_sync
}
/// Maximum retry attempts for sync operations
const MAX_RETRIES: u32 = 3;
/// Initial backoff delay in seconds (doubles each retry: 1s, 2s, 4s)
const INITIAL_BACKOFF_SECS: u64 = 1;

fn set_gist_id(config: &mut Config, provider: &config::SyncProvider, id: String) {
    if let Some(ref mut sync) = config.sync {
        match provider {
            config::SyncProvider::Github => sync.github_config.gist_id = Some(id),
            config::SyncProvider::Gitee => sync.gitee_config.gist_id = Some(id),
        }
    }
}

fn set_github_gist_id(config: &mut Config, id: String) {
    set_gist_id(config, &config::SyncProvider::Github, id);
}

fn set_gitee_gist_id(config: &mut Config, id: String) {
    set_gist_id(config, &config::SyncProvider::Gitee, id);
}

fn snippets_for_push(sync_config: &config::SyncConfig, store: &SnippetStore) -> Vec<Snippet> {
    match sync_config.provider {
        config::SyncProvider::Gitee => store.list().iter().cloned().collect(),
        config::SyncProvider::Github => store.pending_sync().into_iter().cloned().collect(),
    }
}

pub struct SnippetManager {
    store: Mutex<SnippetStore>,
    config: Config,
    client: sync::RealHttpClient,
    ui_sender: Option<Sender<UiResponse>>,
    ui_receiver: Option<Receiver<UiResponse>>,
    /// Tracks the next push sequence number; incremented on each schedule_push call.
    /// The debounce thread compares against this to decide if a newer push was scheduled.
    push_seq: Arc<AtomicU64>,
}

impl SnippetManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::load()?;
        let store_path = Config::config_dir()?.join("snippets.json");
        eprintln!(
            "[DEBUG] SnippetManager::new() - store_path: {:?}",
            store_path
        );
        eprintln!("[DEBUG] store_path exists: {}", store_path.exists());

        let store = SnippetStore::new(store_path)?;
        eprintln!(
            "[DEBUG] Store created, snippets count: {}",
            store.list().len()
        );

        let client = sync::RealHttpClient::new();

        let (tx, rx) = unbounded();

        let push_seq = Arc::new(AtomicU64::new(0));

        Ok(Self {
            store: Mutex::new(store),
            config,
            client,
            ui_sender: Some(tx),
            ui_receiver: Some(rx),
            push_seq,
        })
    }

    pub fn ui_sender(&self) -> Option<Sender<UiResponse>> {
        self.ui_sender.clone()
    }

    pub fn ui_receiver(&mut self) -> Option<Receiver<UiResponse>> {
        self.ui_receiver.take()
    }

    pub fn start_background_sync(&self) {
        eprintln!("[SYNC] Starting background sync...");
        if !should_auto_sync(&self.config) {
            eprintln!("[SYNC] Auto sync disabled or not configured, skipping background sync");
            return;
        }
        let sync_config = match &self.config.sync {
            Some(c) => c.clone(),
            None => {
                eprintln!("[SYNC] No sync config, skipping");
                return;
            }
        };
        let store_path = Config::config_dir()
            .expect("config_dir")
            .join("snippets.json");
        thread::spawn(move || {
            eprintln!("[SYNC] Pulling from remote...");
            let client = sync::RealHttpClient::new();
            match sync::pull(&sync_config, &client) {
                Ok(remote) => {
                    eprintln!(
                        "[SYNC] Pull succeeded, {} snippets from remote",
                        remote.len()
                    );
                    if !remote.is_empty() {
                        if let Ok(mut store) = SnippetStore::new(store_path) {
                            let _ = store.load_from_sync(remote);
                            eprintln!(
                                "[SYNC] Loaded {} snippets to local store",
                                store.list().len()
                            );
                        }
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("同步成功")
                        .body("从远程拉取完成")
                        .show();
                }
                Err(e) => {
                    eprintln!("[SYNC] Background sync failed: {}", e);
                    log::warn!("Background sync failed: {}", e);
                    let _ = notify_rust::Notification::new()
                        .summary("同步失败")
                        .body(&e.to_string())
                        .show();
                }
            }
        });
    }

    pub fn do_pull(&self) -> Result<(), String> {
        let sync_config = self
            .config
            .sync
            .as_ref()
            .ok_or_else(|| "未配置同步".to_string())?;
        let remote = sync::pull(sync_config, &self.client).map_err(|e| e.to_string())?;
        if let Ok(mut store) = self.store.lock() {
            store.load_from_sync(remote).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn apply_gist_id(&mut self, provider: &config::SyncProvider, id: String) -> Result<(), String> {
        let mut cfg = Config::load().map_err(|e| e.to_string())?;
        set_gist_id(&mut cfg, provider, id.clone());
        cfg.save().map_err(|e| e.to_string())?;
        set_gist_id(&mut self.config, provider, id);
        Ok(())
    }

    fn apply_gitee_gist_id(&mut self, id: String) -> Result<(), String> {
        self.apply_gist_id(&config::SyncProvider::Gitee, id)
    }

    pub fn do_push(&mut self) -> Result<(), String> {
        eprintln!("[SYNC] do_push called");
        let sync_config = self
            .config
            .sync
            .as_ref()
            .cloned()
            .ok_or_else(|| "未配置同步".to_string())?;
        let pending: Vec<Snippet> = {
            let store = self.store.lock().map_err(|e| e.to_string())?;
            snippets_for_push(&sync_config, &store)
        };
        eprintln!("[SYNC] Pending sync: {} snippets", pending.len());
        if pending.is_empty() {
            eprintln!("[SYNC] No pending snippets to push");
            return Ok(());
        }
        for attempt in 0..MAX_RETRIES {
            eprintln!("[SYNC] Push attempt {} of {}", attempt + 1, MAX_RETRIES);
            match sync::push(&sync_config, &self.client, &pending) {
                Ok((_, gist_id)) => {
                    eprintln!("[SYNC] Push succeeded, gist_id={:?}", gist_id);
                    if let Some(id) = gist_id {
                        if !id.is_empty() {
                            eprintln!("[SYNC] Got gist_id: {}", id);
                            self.apply_gitee_gist_id(id)?;
                            eprintln!("[SYNC] Saved gist_id to config and updated runtime config");
                        }
                    }
                    // Mark as synced without changing update_date. Using update() here would bump
                    // update_date to now, making older local copies look newer than remote edits.
                    if let Ok(mut store) = self.store.lock() {
                        let ids: Vec<String> = pending.iter().map(|s| s.id.clone()).collect();
                        let _ = store.mark_synced(&ids);
                        eprintln!("[SYNC] Marked {} snippets as synced", pending.len());
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("同步成功")
                        .body(&format!("已上传 {} 个片段", pending.len()))
                        .show();
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("[SYNC] Push attempt {} failed: {}", attempt + 1, e);
                    let backoff = Duration::from_secs(INITIAL_BACKOFF_SECS * (1 << attempt));
                    log::warn!(
                        "do_push attempt {}/{} failed: {}, retrying in {:?}",
                        attempt + 1,
                        MAX_RETRIES,
                        e,
                        backoff
                    );
                    if attempt + 1 < MAX_RETRIES {
                        thread::sleep(backoff);
                    }
                }
            }
        }
        let _ = notify_rust::Notification::new()
            .summary("同步失败")
            .body(&format!("上传失败，重试{}次后仍失败", MAX_RETRIES))
            .show();
        Err(format!("push failed after {} attempts", MAX_RETRIES))
    }

    pub fn schedule_push(&self) {
        eprintln!("[SYNC] schedule_push called");
        if !should_auto_sync(&self.config) {
            eprintln!("[SYNC] Auto sync disabled or not configured, skipping schedule_push");
            return;
        }
        let seq = self.push_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let push_seq = Arc::clone(&self.push_seq);
        let sync_config = match &self.config.sync {
            Some(c) => c.clone(),
            None => {
                eprintln!("[SYNC] No sync config, skipping schedule_push");
                return;
            }
        };
        let store_path = Config::config_dir()
            .expect("config_dir")
            .join("snippets.json");
        let client = sync::RealHttpClient::new();

        thread::spawn(move || {
            eprintln!("[SYNC] Waiting {}ms for debounce...", DEBOUNCE_MS);
            thread::sleep(Duration::from_millis(DEBOUNCE_MS));
            let current = push_seq.load(Ordering::SeqCst);
            if current != seq {
                eprintln!(
                    "[SYNC] Debounce: newer push scheduled (seq {} vs {}), skipping",
                    current, seq
                );
                return;
            }
            eprintln!("[SYNC] Executing sync after debounce (seq={})", seq);
            let sync_config = match Config::load() {
                Ok(cfg) if should_auto_sync(&cfg) => cfg.sync.unwrap(),
                Ok(_) => {
                    eprintln!(
                        "[SYNC] Auto sync disabled or not configured after debounce, skipping"
                    );
                    return;
                }
                Err(_) => sync_config.clone(),
            };

            match sync::pull(&sync_config, &client) {
                Ok(remote) => {
                    if !remote.is_empty() {
                        match SnippetStore::new(store_path.clone()) {
                            Ok(mut store) => {
                                if let Err(e) = store.load_from_sync(remote) {
                                    eprintln!("[SYNC] schedule_push: pull merge failed: {}", e);
                                    log::warn!("schedule_push: pull merge failed: {}", e);
                                    return;
                                }
                            }
                            Err(e) => {
                                eprintln!("[SYNC] schedule_push: store open failed: {}", e);
                                log::warn!("schedule_push: store open failed: {}", e);
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[SYNC] schedule_push: pull before push failed: {}", e);
                    log::warn!("schedule_push: pull before push failed: {}", e);
                    return;
                }
            }

            for attempt in 0..MAX_RETRIES {
                let store = match SnippetStore::new(store_path.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[SYNC] schedule_push: store open failed: {}", e);
                        log::warn!("schedule_push: store open failed: {}", e);
                        return;
                    }
                };
                let pending: Vec<Snippet> = snippets_for_push(&sync_config, &store);
                eprintln!("[SYNC] schedule_push: {} pending snippets", pending.len());
                if pending.is_empty() {
                    eprintln!("[SYNC] No pending snippets, skipping push");
                    return;
                }
                eprintln!(
                    "[SYNC] schedule_push: attempt {} of {}",
                    attempt + 1,
                    MAX_RETRIES
                );
                match sync::push(&sync_config, &client, &pending) {
                    Ok((_, gist_id)) => {
                        eprintln!(
                            "[SYNC] schedule_push: push succeeded, gist_id={:?}",
                            gist_id
                        );
                        if let Some(id) = gist_id {
                            if !id.is_empty() {
                                match Config::load() {
                                    Ok(mut cfg) => {
                                        set_gitee_gist_id(&mut cfg, id);
                                        if let Err(e) = cfg.save() {
                                            eprintln!(
                                                "[SYNC] schedule_push: save gist_id failed: {}",
                                                e
                                            );
                                            log::warn!("schedule_push: save gist_id failed: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[SYNC] schedule_push: load config failed: {}",
                                            e
                                        );
                                        log::warn!("schedule_push: load config failed: {}", e);
                                    }
                                }
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        eprintln!("[SYNC] schedule_push attempt {} failed: {}", attempt + 1, e);
                        let backoff = Duration::from_secs(INITIAL_BACKOFF_SECS * (1 << attempt));
                        log::warn!(
                            "schedule_push attempt {}/{} failed: {}, retrying in {:?}",
                            attempt + 1,
                            MAX_RETRIES,
                            e,
                            backoff
                        );
                        thread::sleep(backoff);
                    }
                }
            }
            eprintln!(
                "[SYNC] schedule_push: all {} retry attempts exhausted",
                MAX_RETRIES
            );
            log::error!(
                "schedule_push: all {} retry attempts exhausted",
                MAX_RETRIES
            );
        });
    }

    pub fn do_pull_with_retry(&self) -> Result<(), String> {
        let sync_config = self
            .config
            .sync
            .as_ref()
            .ok_or_else(|| "未配置同步".to_string())?;
        for attempt in 0..MAX_RETRIES {
            match sync::pull(sync_config, &self.client) {
                Ok(remote) => {
                    if !remote.is_empty() {
                        if let Ok(mut store) = self.store.lock() {
                            store.load_from_sync(remote).map_err(|e| e.to_string())?;
                        }
                    }
                    return Ok(());
                }
                Err(e) => {
                    let backoff = Duration::from_secs(INITIAL_BACKOFF_SECS * (1 << attempt));
                    log::warn!(
                        "do_pull attempt {}/{} failed: {}, retrying in {:?}",
                        attempt + 1,
                        MAX_RETRIES,
                        e,
                        backoff
                    );
                    if attempt + 1 < MAX_RETRIES {
                        thread::sleep(backoff);
                    }
                }
            }
        }
        Err(format!("pull failed after {} attempts", MAX_RETRIES))
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_configured()
    }

    pub fn with_store<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&SnippetStore) -> R,
    {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        Ok(f(&store))
    }

    pub fn with_store_mut<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut SnippetStore) -> R,
    {
        let mut store = self.store.lock().map_err(|e| e.to_string())?;
        Ok(f(&mut store))
    }

    pub fn reload_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config = Config::load()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{GiteeConfig, GithubConfig, SyncConfig, SyncProvider};

    fn gitee_config_with_id(id: Option<&str>) -> Config {
        Config {
            sync: Some(SyncConfig {
                provider: SyncProvider::Gitee,
                owner: String::new(),
                repo: String::new(),
                branch: String::new(),
                token: String::new(),
                github_config: GithubConfig::default(),
                gitee_config: GiteeConfig {
                    gist_id: id.map(String::from),
                    ..GiteeConfig::default()
                },
            }),
            general: Default::default(),
            sync_status: Default::default(),
        }
    }

    #[test]
    fn test_delayed_auto_sync_waits_five_seconds() {
        assert_eq!(delayed_auto_sync_delay_ms(), 5000);
    }

    #[test]
    fn test_auto_sync_requires_sync_config_and_general_switch() {
        let enabled = gitee_config_with_id(Some("gist-id"));
        assert!(should_auto_sync(&enabled));

        let mut disabled = gitee_config_with_id(Some("gist-id"));
        disabled.general.auto_sync = false;
        assert!(!should_auto_sync(&disabled));

        let mut missing_sync = gitee_config_with_id(None);
        missing_sync.sync = None;
        assert!(!should_auto_sync(&missing_sync));
    }
    #[test]
    fn test_set_github_gist_id_does_not_change_gitee_id() {
        let mut config = gitee_config_with_id(Some("gitee-gist"));

        set_github_gist_id(&mut config, "github-gist".into());

        let sync = config.sync.unwrap();
        assert_eq!(sync.github_config.gist_id.as_deref(), Some("github-gist"));
        assert_eq!(sync.gitee_config.gist_id.as_deref(), Some("gitee-gist"));
    }

    #[test]
    fn test_set_gitee_gist_id_does_not_change_github_id() {
        let mut config = gitee_config_with_id(Some("gitee-gist"));
        set_github_gist_id(&mut config, "github-gist".into());

        set_gitee_gist_id(&mut config, "new-gitee-gist".into());

        let sync = config.sync.unwrap();
        assert_eq!(sync.github_config.gist_id.as_deref(), Some("github-gist"));
        assert_eq!(sync.gitee_config.gist_id.as_deref(), Some("new-gitee-gist"));
    }

    #[test]
    fn test_set_gitee_gist_id_replaces_stale_runtime_id() {
        let mut config = gitee_config_with_id(Some("old-gist"));

        set_gitee_gist_id(&mut config, "new-gist".into());

        assert_eq!(
            config.sync.unwrap().gitee_config.gist_id,
            Some("new-gist".into())
        );
    }

    #[test]
    fn test_gitee_push_uses_whole_tree_not_only_pending_items() {
        let sync_config = gitee_config_with_id(Some("gist-id")).sync.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut store = SnippetStore::new(dir.path().join("snippets.json")).unwrap();

        let mut synced = Snippet::new_snippet("synced".into(), "already remote".into(), None);
        synced.id = "synced-id".into();
        synced.synced = true;
        store.add(synced).unwrap();

        let mut pending = Snippet::new_snippet("pending".into(), "local change".into(), None);
        pending.id = "pending-id".into();
        pending.synced = false;
        store.add(pending).unwrap();

        let snippets = snippets_for_push(&sync_config, &store);

        assert_eq!(snippets.len(), 2);
        assert!(snippets.iter().any(|s| s.id == "synced-id"));
        assert!(snippets.iter().any(|s| s.id == "pending-id"));
    }

    #[test]
    fn test_github_push_uses_only_pending_items() {
        let mut config = gitee_config_with_id(Some("gist-id"));
        let sync_config = config.sync.as_mut().unwrap();
        sync_config.provider = SyncProvider::Github;
        let sync_config = config.sync.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut store = SnippetStore::new(dir.path().join("snippets.json")).unwrap();

        let mut synced = Snippet::new_snippet("synced".into(), "already remote".into(), None);
        synced.id = "synced-id".into();
        synced.synced = true;
        store.add(synced).unwrap();

        let mut pending = Snippet::new_snippet("pending".into(), "local change".into(), None);
        pending.id = "pending-id".into();
        pending.synced = false;
        store.add(pending).unwrap();

        let snippets = snippets_for_push(&sync_config, &store);

        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].id, "pending-id");
    }
}

impl Default for SnippetManager {
    fn default() -> Self {
        Self::new().expect("Failed to create SnippetManager")
    }
}
