pub mod config;
pub mod gitee;
pub mod model;
pub mod store;
pub mod sync;
pub mod ui;

use std::sync::Mutex;
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use config::Config;
use model::Snippet;
use store::SnippetStore;
use ui::UiResponse;

pub struct SnippetManager {
    store: Mutex<SnippetStore>,
    config: Config,
    client: sync::RealHttpClient,
    ui_sender: Option<Sender<UiResponse>>,
    ui_receiver: Option<Receiver<UiResponse>>,
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

        Ok(Self {
            store: Mutex::new(store),
            config,
            client,
            ui_sender: Some(tx),
            ui_receiver: Some(rx),
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
        sync::push(sync_config, &self.client, &pending).map_err(|e| e.to_string())?;
        Ok(())
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
