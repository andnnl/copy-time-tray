pub mod config;
pub mod gitee;
pub mod model;
pub mod store;
pub mod sync;
pub mod ui;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};

use config::Config;
use model::Snippet;
use store::SnippetStore;
use ui::UiResponse;

/// Debounce delay in milliseconds before triggering push after changes
const DEBOUNCE_MS: u64 = 2000;
/// Maximum retry attempts for sync operations
const MAX_RETRIES: u32 = 3;
/// Initial backoff delay in seconds (doubles each retry: 1s, 2s, 4s)
const INITIAL_BACKOFF_SECS: u64 = 1;

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
        eprintln!("[DEBUG] SnippetManager::new() - store_path: {:?}", store_path);
        eprintln!("[DEBUG] store_path exists: {}", store_path.exists());
        
        let store = SnippetStore::new(store_path)?;
        eprintln!("[DEBUG] Store created, snippets count: {}", store.list().len());
        
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
        let sync_config = match &self.config.sync {
            Some(c) => c.clone(),
            None => return,
        };
        let store_path = Config::config_dir()
            .expect("config_dir")
            .join("snippets.json");
        thread::spawn(move || {
            let client = sync::RealHttpClient::new();
            match sync::pull(&sync_config, &client) {
                Ok(remote) => {
                    if !remote.is_empty() {
                        if let Ok(mut store) = SnippetStore::new(store_path) {
                            let _ = store.load_from_sync(remote);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Background sync failed: {}", e);
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

    pub fn do_push(&self) -> Result<(), String> {
        let sync_config = self
            .config
            .sync
            .as_ref()
            .ok_or_else(|| "未配置同步".to_string())?;
        let pending: Vec<Snippet> = {
            let store = self.store.lock().map_err(|e| e.to_string())?;
            store.pending_sync().into_iter().cloned().collect()
        };
        if pending.is_empty() {
            return Ok(());
        }
        for attempt in 0..MAX_RETRIES {
            match sync::push(sync_config, &self.client, &pending) {
                Ok(_) => return Ok(()),
                Err(e) => {
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
        Err(format!("push failed after {} attempts", MAX_RETRIES))
    }

    pub fn schedule_push(&self) {
        let seq = self.push_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let push_seq = Arc::clone(&self.push_seq);
        let sync_config = match &self.config.sync {
            Some(c) => c.clone(),
            None => return,
        };
        let store_path = Config::config_dir()
            .expect("config_dir")
            .join("snippets.json");
        let client = sync::RealHttpClient::new();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(DEBOUNCE_MS));
            let current = push_seq.load(Ordering::SeqCst);
            if current != seq {
                return;
            }
            for attempt in 0..MAX_RETRIES {
                let store = match SnippetStore::new(store_path.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("schedule_push: store open failed: {}", e);
                        return;
                    }
                };
                let pending: Vec<Snippet> = store
                    .pending_sync()
                    .into_iter()
                    .cloned()
                    .collect();
                if pending.is_empty() {
                    return;
                }
                match sync::push(&sync_config, &client, &pending) {
                    Ok(_) => return,
                    Err(e) => {
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
            log::error!("schedule_push: all {} retry attempts exhausted", MAX_RETRIES);
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
}

impl Default for SnippetManager {
    fn default() -> Self {
        Self::new().expect("Failed to create SnippetManager")
    }
}
