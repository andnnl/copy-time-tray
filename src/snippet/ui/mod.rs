use crossbeam_channel::Sender;
use crate::snippet::model::Snippet;

pub mod search;
pub mod editor;
#[cfg(target_os = "linux")]
pub mod config_dialog;
#[cfg(target_os = "linux")]
pub mod config_window;
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
    RequestPull,
    RequestPush,
    Close,
}

pub struct UiChannels {
    pub command_sender: Sender<UiCommand>,
    pub response_sender: Sender<UiResponse>,
}
