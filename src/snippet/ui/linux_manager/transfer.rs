use crate::snippet::import_export::import_from_termora;
use crate::snippet::store::ImportSummary;
use std::path::PathBuf;

/// Data returned from the worker without retaining GTK objects.
pub enum ImportResult {
    Imported(Result<ImportSummary, String>),
}

/// Reads, parses, and persists the import on a worker thread. GTK callers only
/// receive this data result through the supplied async channel.
pub fn spawn_import(path: PathBuf, sender: async_channel::Sender<ImportResult>) {
    std::thread::spawn(move || {
        let result = import_from_termora(&path)
            .map_err(|error| error.to_string())
            .and_then(|snippets| {
                let manager = crate::app::MANAGER
                    .get()
                    .ok_or_else(|| "Snippet 管理器未初始化".to_string())?
                    .lock()
                    .map_err(|error| error.to_string())?;
                manager
                    .with_store_mut(|store| store.import_batch(snippets))
                    .map_err(|error| error.to_string())?
                    .map_err(|error| error.to_string())
            });
        let _ = sender.send_blocking(ImportResult::Imported(result));
    });
}
