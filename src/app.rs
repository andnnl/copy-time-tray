use crate::hotkey::{should_open_snippets, HotkeyService, Shortcut};
use crate::platform;
use crate::snippet::config::Config;
use crate::snippet::SnippetManager;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use std::cell::RefCell;
#[cfg(target_os = "windows")]
use std::sync::Arc;

#[cfg(target_os = "windows")]
use crate::snippet::ui::search::SearchPopup;
pub use crate::snippet::ui::{UiCommand, UiResponse};

pub static MANAGER: OnceLock<Mutex<SnippetManager>> = OnceLock::new();
#[cfg(target_os = "linux")]
static HOTKEY_SERVICE: OnceLock<Mutex<HotkeyService>> = OnceLock::new();
#[cfg(target_os = "windows")]
thread_local! {
    static HOTKEY_SERVICE: RefCell<Option<HotkeyService>> = const { RefCell::new(None) };
}
static HOTKEY_CURRENT_ID: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "windows")]
static CMD_TX: OnceLock<crossbeam_channel::Sender<UiCommand>> = OnceLock::new();

pub fn run() {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<UiCommand>();
    #[cfg(target_os = "windows")]
    let (resp_tx, resp_rx) = crossbeam_channel::unbounded::<UiResponse>();
    #[cfg(target_os = "windows")]
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    match SnippetManager::new() {
        Ok(manager) => {
            manager.start_background_sync();
            let _ = MANAGER.set(Mutex::new(manager));
        }
        Err(error) => {
            let _ = notify_rust::Notification::new()
                .summary("Snippet 初始化失败")
                .body(&format!("无法启动 Snippet 功能: {}", error))
                .show();
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = CMD_TX.set(cmd_tx.clone());
        let response_sender = resp_tx.clone();
        let running_for_ui = running.clone();
        std::thread::spawn(move || launch_egui(cmd_rx, response_sender, running_for_ui));
    }

    if let Err(error) = launch_global_hotkey(cmd_tx.clone()) {
        let _ = notify_rust::Notification::new()
            .summary("快捷键初始化失败")
            .body(&error)
            .show();
    }
    #[cfg(target_os = "windows")]
    spawn_response_handler(resp_rx);

    #[cfg(target_os = "linux")]
    platform::linux::main(cmd_tx, cmd_rx);
    #[cfg(target_os = "windows")]
    platform::windows::main(cmd_tx, running);
}

fn broadcast_refresh(
    snippet_type_filter: Option<crate::snippet::model::SnippetType>,
) -> Vec<crate::snippet::model::Snippet> {
    let manager_guard = match MANAGER.get().and_then(|manager| manager.lock().ok()) {
        Some(guard) => guard,
        None => return Vec::new(),
    };
    manager_guard
        .with_store(|store| {
            store
                .list()
                .iter()
                .filter(|snippet| {
                    !snippet.deleted
                        && snippet_type_filter
                            .as_ref()
                            .map(|filter| snippet.snippet_type == *filter)
                            .unwrap_or(true)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
pub fn send_refresh_to_search() {
    if let Some(sender) = CMD_TX.get() {
        let _ = sender.send(UiCommand::RefreshSnippets(broadcast_refresh(None)));
    }
}

fn launch_global_hotkey(cmd_tx: crossbeam_channel::Sender<UiCommand>) -> Result<(), String> {
    let configured = Config::load().unwrap_or_default().general.snippet_hotkey;
    let initial = match configured.parse::<Shortcut>() {
        Ok(shortcut) => shortcut,
        Err(error) => {
            let fallback = "Ctrl+Shift+S"
                .parse::<Shortcut>()
                .map_err(|fallback_error| fallback_error.to_string())?;
            let _ = notify_rust::Notification::new()
                .summary("快捷键配置无效")
                .body(&format!("{error}，已临时使用 {fallback}"))
                .show();
            fallback
        }
    };
    let service = HotkeyService::new(initial).map_err(|error| error.to_string())?;
    HOTKEY_CURRENT_ID.store(service.current_id(), Ordering::Release);
    #[cfg(target_os = "linux")]
    HOTKEY_SERVICE
        .set(Mutex::new(service))
        .map_err(|_| "快捷键服务已初始化".to_string())?;
    #[cfg(target_os = "windows")]
    HOTKEY_SERVICE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            Err("快捷键服务已初始化".to_string())
        } else {
            *slot = Some(service);
            Ok(())
        }
    })?;

    let event_receiver = global_hotkey::GlobalHotKeyEvent::receiver();
    std::thread::spawn(move || loop {
        if let Ok(event) = event_receiver.recv() {
            let current_id = HOTKEY_CURRENT_ID.load(Ordering::Acquire);
            let matches = should_open_snippets(current_id, event.id, event.state);
            if matches {
                let _ = cmd_tx.send(UiCommand::Show);
            }
        }
    });
    Ok(())
}

fn apply_config_with_service(
    mut config: Config,
    service: &mut HotkeyService,
) -> Result<(), String> {
    let candidate = config
        .general
        .snippet_hotkey
        .parse::<Shortcut>()
        .map_err(|error| error.to_string())?;
    config.general.snippet_hotkey = candidate.to_string();
    let previous_config = Config::load().map_err(|error| error.to_string())?;
    let previous_shortcut = service.current();

    service
        .rebind(candidate)
        .map_err(|error| error.to_string())?;
    HOTKEY_CURRENT_ID.store(service.current_id(), Ordering::Release);
    if let Err(error) = config.save() {
        return match service.rebind(previous_shortcut) {
            Ok(()) => {
                HOTKEY_CURRENT_ID.store(service.current_id(), Ordering::Release);
                Err(format!("配置保存失败：{error}"))
            }
            Err(rollback_error) => Err(format!(
                "配置保存失败：{error}；快捷键回滚失败：{rollback_error}"
            )),
        };
    }

    if let Some(mut manager) = MANAGER.get().and_then(|manager| manager.lock().ok()) {
        if let Err(error) = manager.reload_config() {
            let restore_config_result = previous_config.save();
            let restore_hotkey_result = service.rebind(previous_shortcut);
            HOTKEY_CURRENT_ID.store(service.current_id(), Ordering::Release);
            return Err(match (restore_config_result, restore_hotkey_result) {
                (Ok(()), Ok(())) => format!("配置应用失败：{error}"),
                (config_result, hotkey_result) => format!(
                    "配置应用失败：{error}；配置回滚结果：{config_result:?}；快捷键回滚结果：{hotkey_result:?}"
                ),
            });
        }
        manager.schedule_push();
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn apply_config_with_hotkey(config: Config) -> Result<(), String> {
    let service = HOTKEY_SERVICE
        .get()
        .ok_or_else(|| "快捷键服务尚未初始化".to_string())?;
    let mut service = service.lock().map_err(|error| error.to_string())?;
    apply_config_with_service(config, &mut service)
}

#[cfg(target_os = "windows")]
pub fn apply_config_with_hotkey(config: Config) -> Result<(), String> {
    HOTKEY_SERVICE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let service = slot
            .as_mut()
            .ok_or_else(|| "快捷键服务尚未初始化".to_string())?;
        apply_config_with_service(config, service)
    })
}

fn handle_ui_response(response: UiResponse) {
    let manager_guard = match MANAGER.get().and_then(|manager| manager.lock().ok()) {
        Some(guard) => guard,
        None => return,
    };
    let manager = &*manager_guard;
    match response {
        UiResponse::SnippetCopied(content) => {
            #[cfg(target_os = "linux")]
            {
                let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
                clipboard.set_text(&content);
                clipboard.store();
            }
            #[cfg(target_os = "windows")]
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(&content);
            }
        }
        UiResponse::SnippetCreated(snippet) => {
            if manager.with_store_mut(|store| store.add(snippet)).is_ok() {
                manager.schedule_push();
            }
            #[cfg(target_os = "windows")]
            send_refresh_to_search();
        }
        UiResponse::SnippetUpdated(snippet) => {
            let id = snippet.id.clone();
            if manager
                .with_store_mut(|store| {
                    store.update(&id, |item| {
                        item.name = snippet.name;
                        item.content = snippet.content;
                    })
                })
                .is_ok()
            {
                manager.schedule_push();
            }
            #[cfg(target_os = "windows")]
            send_refresh_to_search();
        }
        UiResponse::SnippetDeleted(id) => {
            if manager.with_store_mut(|store| store.delete(&id)).is_ok() {
                manager.schedule_push();
            }
            #[cfg(target_os = "windows")]
            send_refresh_to_search();
        }
        UiResponse::RequestPull | UiResponse::RequestPush | UiResponse::Close => {}
    }
}

fn spawn_response_handler(resp_rx: crossbeam_channel::Receiver<UiResponse>) {
    std::thread::spawn(move || {
        while let Ok(response) = resp_rx.recv() {
            handle_ui_response(response);
        }
    });
}

#[cfg(target_os = "windows")]
fn launch_egui(
    cmd_rx: crossbeam_channel::Receiver<UiCommand>,
    resp_tx: crossbeam_channel::Sender<UiResponse>,
    running: Arc<std::sync::atomic::AtomicBool>,
) {
    let popup_tx = Arc::new(Mutex::new(None::<crossbeam_channel::Sender<UiCommand>>));
    let popup_tx_clone = popup_tx.clone();
    std::thread::spawn(move || {
        while let Ok(command) = cmd_rx.recv() {
            if let Some(sender) = popup_tx_clone.lock().unwrap().as_ref() {
                let _ = sender.send(command);
            }
        }
    });
    while running.load(std::sync::atomic::Ordering::Relaxed) {
        let (popup_sender, popup_receiver) = crossbeam_channel::unbounded::<UiCommand>();
        *popup_tx.lock().unwrap() = Some(popup_sender);
        let search = SearchPopup::new(popup_receiver, resp_tx.clone());
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([600.0, 400.0])
                .with_title("Snippet 搜索")
                .with_visible(false),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "Copy Time Tray - Snippet",
            native_options,
            Box::new(|_| Ok(Box::new(search))),
        );
    }
}
