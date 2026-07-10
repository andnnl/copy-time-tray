use crate::snippet::import_export::{export_to_termora, import_from_termora};
use crate::snippet::model::Snippet;
use crate::snippet::store::ImportSummary;
use std::path::PathBuf;

/// Data returned from an import worker without retaining GTK objects.
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

/// Result returned by the export worker without retaining GTK objects.
pub enum ExportResult {
    Exported(Result<(), String>),
}

/// Serializes and writes a snapshot independently of GTK state.
pub fn export_snapshot(snippets: &[Snippet], path: &std::path::Path) -> Result<(), String> {
    export_to_termora(snippets, path).map_err(|error| error.to_string())
}

/// Serializes and writes an already-captured snippet snapshot on a worker thread.
pub fn spawn_export(
    snippets: Vec<Snippet>,
    path: PathBuf,
    sender: async_channel::Sender<ExportResult>,
) {
    std::thread::spawn(move || {
        let result = export_snapshot(&snippets, &path);
        let _ = sender.send_blocking(ExportResult::Exported(result));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_worker_writes_snapshot_to_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("snippets.json");
        let snippets = vec![Snippet::new_snippet(
            "list files".to_string(),
            "ls -la".to_string(),
            None,
        )];
        let (sender, receiver) = async_channel::bounded(1);

        spawn_export(snippets, path.clone(), sender);

        assert!(matches!(
            receiver.recv_blocking().unwrap(),
            ExportResult::Exported(Ok(()))
        ));

        let exported = import_from_termora(&path).unwrap();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].name, "list files");
        assert_eq!(exported[0].content.as_deref(), Some("ls -la"));
    }
}
