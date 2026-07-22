use crate::snippet::model::Snippet;
use crossbeam_channel::Sender;

#[cfg(target_os = "linux")]
pub mod config_dialog;
#[cfg(target_os = "linux")]
pub mod config_window;
pub mod editor;
#[cfg(target_os = "linux")]
pub mod linux_manager;
pub mod search;
#[cfg(target_os = "linux")]
pub mod tabs;

#[derive(Debug, Clone)]
pub enum UiCommand {
    Show,
    Hide,
    CopyToClipboard(String),
    RefreshSnippets(Vec<Snippet>),
    Sync,
}

#[derive(Debug)]
pub enum UiResponse {
    SnippetCopied(String),
    SnippetCreated(Snippet),
    SnippetUpdated(Snippet),
    SnippetDeleted(String),
    FavoriteToggled { id: String, favorite: bool },
    RequestPull,
    RequestPush,
    Close,
}

pub struct UiChannels {
    pub command_sender: Sender<UiCommand>,
    pub response_sender: Sender<UiResponse>,
}
