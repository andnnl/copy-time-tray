use chrono::Local;
use md5::{Digest, Md5};
use rand::Rng;

mod snippet;

use snippet::SnippetManager;
use snippet::ui::{UiCommand, UiResponse};
#[cfg(target_os = "windows")]
use snippet::ui::search::SearchPopup;
use std::sync::{Arc, Mutex, OnceLock};

static MANAGER: OnceLock<Mutex<SnippetManager>> = OnceLock::new();

#[cfg(target_os = "windows")]
static CMD_TX: OnceLock<crossbeam_channel::Sender<UiCommand>> = OnceLock::new();

#[cfg(target_os = "linux")]
static DIALOG_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "linux")]
static DRAG_DATA: Mutex<Option<String>> = Mutex::new(None);

/// 读取当前 snippet 列表并广播给 UI
fn broadcast_refresh(snippet_type_filter: Option<snippet::model::SnippetType>) -> Vec<snippet::model::Snippet> {
    let manager_guard = match MANAGER.get().and_then(|m| m.lock().ok()) {
        Some(guard) => guard,
        None => return Vec::new(),
    };
    let result = manager_guard.with_store(|store| {
        store.list()
            .iter()
            .filter(|s| {
                if s.deleted {
                    return false;
                }
                if let Some(ref filter_type) = snippet_type_filter {
                    return s.snippet_type == *filter_type;
                }
                true
            })
            .cloned()
            .collect()
    });
    result.unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn send_refresh_to_search() {
    let snippets = broadcast_refresh(None);
    if let Some(tx) = CMD_TX.get() {
        let _ = tx.send(UiCommand::RefreshSnippets(snippets));
    }
}

fn generate_password(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn generate_md5(input: &str, length: usize) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let hex = format!("{:x}", result);
    match length {
        16 => hex[8..24].to_string(),
        _ => hex,
    }
}

fn get_time_text(format: &str) -> String {
    let now = Local::now();
    match format {
        "date" => now.format("%Y-%m-%d").to_string(),
        "compact" => now.format("%Y%m%d%H%M%S").to_string(),
        _ => now.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

// ===== Linux 实现（GTK + libappindicator）=====
#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use gtk::prelude::*;
    use gtk::{glib, Menu, MenuItem, SeparatorMenuItem};
    use libappindicator::{AppIndicator, AppIndicatorStatus};
    use notify_rust::Notification;
    use snippet::ui::config_dialog;

    fn copy_to_clipboard(text: &str) {
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        clipboard.set_text(text);
        clipboard.store();
        let _ = Notification::new()
            .summary("Copy Time")
            .body(&format!("已复制: {}", text))
            .show();
    }

    fn copy_time(format: &str) {
        copy_to_clipboard(&get_time_text(format));
    }

    pub fn main(_cmd_tx: crossbeam_channel::Sender<UiCommand>) {
        gtk::init().expect("Failed to initialize GTK");

        let icon_path = format!("{}/icons/copy-time.svg", env!("CARGO_MANIFEST_DIR"));
        let mut indicator = AppIndicator::new("copy-time", &icon_path);
        indicator.set_status(AppIndicatorStatus::Active);
        indicator.set_title("Copy Time");

        let mut menu = Menu::new();

        // 复制时间菜单项
        let full_item = MenuItem::with_label("复制完整时间");
        full_item.connect_activate(move |_| copy_time("full"));
        menu.append(&full_item);

        let date_item = MenuItem::with_label("复制日期");
        date_item.connect_activate(move |_| copy_time("date"));
        menu.append(&date_item);

        let compact_item = MenuItem::with_label("复制纯数字");
        compact_item.connect_activate(move |_| copy_time("compact"));
        menu.append(&compact_item);

        menu.append(&SeparatorMenuItem::new());

        // 密码生成子菜单
        let password_menu = Menu::new();
        let password_item = MenuItem::with_label("密码");
        password_item.set_submenu(Some(&password_menu));

        let pwd8_item = MenuItem::with_label("8位密码");
        pwd8_item.connect_activate(move |_| {
            let pwd = generate_password(8);
            copy_to_clipboard(&pwd);
        });
        password_menu.append(&pwd8_item);

        let pwd12_item = MenuItem::with_label("12位密码");
        pwd12_item.connect_activate(move |_| {
            let pwd = generate_password(12);
            copy_to_clipboard(&pwd);
        });
        password_menu.append(&pwd12_item);

        let pwd16_item = MenuItem::with_label("16位密码");
        pwd16_item.connect_activate(move |_| {
            let pwd = generate_password(16);
            copy_to_clipboard(&pwd);
        });
        password_menu.append(&pwd16_item);

        menu.append(&password_item);

        // MD5 子菜单
        let md5_menu = Menu::new();
        let md5_item = MenuItem::with_label("MD5");
        md5_item.set_submenu(Some(&md5_menu));

        let md5_16_item = MenuItem::with_label("16位MD5");
        md5_16_item.connect_activate(move |_| {
            let time_text = get_time_text("full");
            let md5_result = generate_md5(&time_text, 16);
            copy_to_clipboard(&md5_result);
        });
        md5_menu.append(&md5_16_item);

        let md5_32_item = MenuItem::with_label("32位MD5");
        md5_32_item.connect_activate(move |_| {
            let time_text = get_time_text("full");
            let md5_result = generate_md5(&time_text, 32);
            copy_to_clipboard(&md5_result);
        });
        md5_menu.append(&md5_32_item);

        menu.append(&md5_item);

        menu.append(&SeparatorMenuItem::new());

        let sync_item = MenuItem::with_label("同步 Snippet");
        sync_item.connect_activate(move |_| {
            std::thread::spawn(|| {
                let manager_guard = match MANAGER.get().and_then(|m| m.lock().ok()) {
                    Some(guard) => guard,
                    None => {
                        let _ = notify_rust::Notification::new()
                            .summary("Snippet Sync Failed")
                            .body("管理器未初始化")
                            .show();
                        return;
                    }
                };
                let manager = &*manager_guard;

                if !manager.is_configured() {
                    glib::idle_add(move || {
                        if let Some(config_result) = config_dialog::show_config_dialog(None) {
                            let config = crate::snippet::config::Config {
                                sync: Some(crate::snippet::config::SyncConfig {
                                    provider: config_result.provider,
                                    owner: config_result.owner,
                                    repo: config_result.repo,
                                    branch: config_result.branch,
                                    token: config_result.token,
                                }),
                            };
                            if let Err(e) = config.save() {
                                let _ = notify_rust::Notification::new()
                                    .summary("配置保存失败")
                                    .body(&format!("{}", e))
                                    .show();
                            } else {
                                let _ = notify_rust::Notification::new()
                                    .summary("配置已保存")
                                    .body("现在可以再次点击同步")
                                    .show();
                            }
                        }
                        glib::ControlFlow::Break
                    });
                    return;
                }

                glib::idle_add(move || {
                    let _ = notify_rust::Notification::new()
                        .summary("Snippet Sync")
                        .body("同步中...")
                        .show();
                    glib::ControlFlow::Break
                });

                let sync_result = manager.do_pull().and_then(|_| manager.do_push());
                let sync_result = std::sync::Arc::new(std::sync::Mutex::new(sync_result));

                glib::idle_add(move || {
                    let result = sync_result.lock().unwrap().clone();
                    match result {
                        Ok(_) => {
                            let _ = notify_rust::Notification::new()
                                .summary("Snippet Sync")
                                .body("同步完成")
                                .show();
                        }
                        Err(e) => {
                            let _ = notify_rust::Notification::new()
                                .summary("Snippet Sync Failed")
                                .body(&e)
                                .show();
                        }
                    }
                    glib::ControlFlow::Break
                });
            });
        });
        menu.append(&sync_item);

        let snippet_item = MenuItem::with_label("代码片断");
        let cmd_tx_clone = _cmd_tx.clone();
        snippet_item.connect_activate(move |_| {
            launch_snippet_dialog_linux(cmd_tx_clone.clone());
        });
        menu.append(&snippet_item);

        let quit_item = MenuItem::with_label("退出");
        quit_item.connect_activate(|_| gtk::main_quit());
        menu.append(&quit_item);

        menu.show_all();
        indicator.set_menu(&mut menu);

        gtk::main();
    }
}

// ===== Windows 实现（tray-icon + muda）=====
#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use arboard::Clipboard;
    use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use notify_rust::Notification;
    use tray_icon::{TrayIcon, TrayIconBuilder};

    fn copy_to_clipboard(text: &str) {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
        let _ = Notification::new()
            .summary("Copy Time")
            .body(&format!("已复制: {}", text))
            .show();
    }

    fn copy_time(format: &str) {
        copy_to_clipboard(&get_time_text(format));
    }

    fn create_icon_rgba() -> Vec<u8> {
        let mut rgba = vec![0u8; 32 * 32 * 4];
        for y in 0..32 {
            for x in 0..32 {
                let idx = (y * 32 + x) * 4;
                let dx = (x as f32 - 16.0) / 12.0;
                let dy = (y as f32 - 16.0) / 12.0;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 1.0 && dist > 0.85 {
                    rgba[idx] = 255; rgba[idx+1] = 255; rgba[idx+2] = 255; rgba[idx+3] = 255;
                }
                if x >= 15 && x <= 17 && y >= 6 && y <= 16 {
                    rgba[idx] = 255; rgba[idx+1] = 255; rgba[idx+2] = 255; rgba[idx+3] = 255;
                }
                if y >= 15 && y <= 17 && x >= 16 && x <= 24 {
                    rgba[idx] = 255; rgba[idx+1] = 255; rgba[idx+2] = 255; rgba[idx+3] = 255;
                }
            }
        }
        rgba
    }

    pub fn main(cmd_tx: crossbeam_channel::Sender<UiCommand>, running: Arc<std::sync::atomic::AtomicBool>) {
        let menu = Menu::new();

        // 复制时间菜单项
        let full_id = muda::MenuId::new("full");
        let date_id = muda::MenuId::new("date");
        let compact_id = muda::MenuId::new("compact");
        let full_item = MenuItem::with_id(full_id.clone(), "复制完整时间", true, None);
        let date_item = MenuItem::with_id(date_id.clone(), "复制日期", true, None);
        let compact_item = MenuItem::with_id(compact_id.clone(), "复制纯数字", true, None);
        menu.append_items(&[&full_item, &date_item, &compact_item, &PredefinedMenuItem::separator()]).unwrap();

        // 密码生成子菜单
        let password_submenu = Submenu::new("密码", true);
        let pwd8_id = muda::MenuId::new("pwd8");
        let pwd12_id = muda::MenuId::new("pwd12");
        let pwd16_id = muda::MenuId::new("pwd16");
        let pwd8_item = MenuItem::with_id(pwd8_id.clone(), "8位密码", true, None);
        let pwd12_item = MenuItem::with_id(pwd12_id.clone(), "12位密码", true, None);
        let pwd16_item = MenuItem::with_id(pwd16_id.clone(), "16位密码", true, None);
        password_submenu.append_items(&[&pwd8_item, &pwd12_item, &pwd16_item]).unwrap();
        menu.append(&password_submenu).unwrap();

        // MD5 子菜单
        let md5_submenu = Submenu::new("MD5", true);
        let md5_16_id = muda::MenuId::new("md5_16");
        let md5_32_id = muda::MenuId::new("md5_32");
        let md5_16_item = MenuItem::with_id(md5_16_id.clone(), "16位MD5", true, None);
        let md5_32_item = MenuItem::with_id(md5_32_id.clone(), "32位MD5", true, None);
        md5_submenu.append_items(&[&md5_16_item, &md5_32_item]).unwrap();
        menu.append(&md5_submenu).unwrap();

        menu.append(&PredefinedMenuItem::separator()).unwrap();

        let sync_id = muda::MenuId::new("sync_snippets");
        let sync_item = MenuItem::with_id(sync_id.clone(), "同步 Snippet", true, None);
        let snippet_id = muda::MenuId::new("search_snippets");
        let snippet_item = MenuItem::with_id(snippet_id.clone(), "代码片断", true, None);
        let quit_id = muda::MenuId::new("quit");
        let quit_item = MenuItem::with_id(quit_id.clone(), "退出", true, None);
        menu.append_items(&[&sync_item, &snippet_item, &PredefinedMenuItem::separator(), &quit_item]).unwrap();

        let rgba = create_icon_rgba();
        let icon = image::RgbaImage::from_raw(32, 32, rgba).expect("Failed to create icon");
        let icon = tray_icon::Icon::from_rgba(icon.into_raw(), 32, 32).expect("Failed to create icon");

        let _tray_icon: TrayIcon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("Copy Time")
            .build()
            .unwrap();

        let running_for_thread = running.clone();
        let menu_channel = MenuEvent::receiver();
        std::thread::spawn(move || {
            loop {
                if let Ok(event) = menu_channel.recv() {
                    match event.id.0.as_str() {
                        "full" => copy_time("full"),
                        "date" => copy_time("date"),
                        "compact" => copy_time("compact"),
                        "pwd8" => copy_to_clipboard(&generate_password(8)),
                        "pwd12" => copy_to_clipboard(&generate_password(12)),
                        "pwd16" => copy_to_clipboard(&generate_password(16)),
                        "md5_16" => {
                            let time_text = get_time_text("full");
                            copy_to_clipboard(&generate_md5(&time_text, 16));
                        }
                        "md5_32" => {
                            let time_text = get_time_text("full");
                            copy_to_clipboard(&generate_md5(&time_text, 32));
                        }
                        "sync_snippets" => {
                            if let Some(manager_guard) = MANAGER.get().and_then(|m| m.lock().ok()) {
                                let manager = &*manager_guard;
                                if !manager.is_configured() {
                                    let _ = notify_rust::Notification::new()
                                        .summary("未配置同步")
                                        .body("请在 ~/.config/copy-time-tray/config.yaml 中配置同步信息")
                                        .show();
                                    continue;
                                }
                                
                                let sync_result = manager.do_pull().and_then(|_| manager.do_push());
                                match sync_result {
                                    Ok(_) => {
                                        let _ = notify_rust::Notification::new()
                                            .summary("同步成功")
                                            .body("Snippets 已同步到云端")
                                            .show();
                                    }
                                    Err(e) => {
                                        let _ = notify_rust::Notification::new()
                                            .summary("同步失败")
                                            .body(&format!("{}", e))
                                            .show();
                                    }
                                }
                            }
                        }
                        "search_snippets" => {
                            send_refresh_to_search();
                            let _ = cmd_tx.send(UiCommand::Show);
                        }
                        "quit" => {
                            running_for_thread.store(false, std::sync::atomic::Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }
            }
        });

        // Windows 保持进程存活，直到收到退出信号
        while running.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(target_os = "windows")]
fn launch_egui(
    cmd_rx: crossbeam_channel::Receiver<UiCommand>,
    resp_tx: crossbeam_channel::Sender<UiResponse>,
    running: Arc<std::sync::atomic::AtomicBool>,
) {
    let popup_tx = Arc::new(std::sync::Mutex::new(None::<crossbeam_channel::Sender<UiCommand>>));

    let popup_tx_clone = popup_tx.clone();
    std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            if let Some(tx) = popup_tx_clone.lock().unwrap().as_ref() {
                let _ = tx.send(cmd);
            }
        }
    });

    while running.load(std::sync::atomic::Ordering::Relaxed) {
        let (new_popup_tx, popup_rx) = crossbeam_channel::unbounded::<UiCommand>();
        *popup_tx.lock().unwrap() = Some(new_popup_tx);

        let search = SearchPopup::new(popup_rx, resp_tx.clone());
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
            Box::new(|_cc| Ok(Box::new(search))),
        );
    }
}

fn launch_global_hotkey(cmd_tx: crossbeam_channel::Sender<UiCommand>) {
    use global_hotkey::hotkey::{HotKey, Modifiers, Code};
    use global_hotkey::GlobalHotKeyManager;

    let manager = GlobalHotKeyManager::new().unwrap();
    let hotkey = HotKey::new(
        Some(Modifiers::CONTROL | Modifiers::SHIFT),
        Code::KeyS,
    );
    manager.register(hotkey).unwrap();
    let event_receiver = global_hotkey::GlobalHotKeyEvent::receiver();

    std::thread::spawn(move || {
        loop {
            if let Ok(event) = event_receiver.recv() {
                if event.state == global_hotkey::HotKeyState::Pressed {
                    let _ = cmd_tx.send(UiCommand::Show);
                }
            }
        }
    });
}

fn handle_ui_response(response: UiResponse) {
    let manager_guard = match MANAGER.get().and_then(|m| m.lock().ok()) {
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
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&content);
                }
            }
            let _ = notify_rust::Notification::new()
                .summary("Snippet Copied")
                .body(&format!("已复制: {}", &content.chars().take(30).collect::<String>()))
                .show();
        }
        UiResponse::SnippetCreated(snippet) => {
            if let Err(e) = manager.with_store_mut(|store| store.add(snippet.clone())) {
                let _ = notify_rust::Notification::new()
                    .summary("创建失败")
                    .body(&format!("{}", e))
                    .show();
            }
            #[cfg(target_os = "windows")]
            send_refresh_to_search();
        }
        UiResponse::SnippetUpdated(snippet) => {
            let id = snippet.id.clone();
            let _ = manager.with_store_mut(|store| {
                store.update(&id, |s| {
                    s.name = snippet.name.clone();
                    s.content = snippet.content.clone();
                })
            });
            #[cfg(target_os = "windows")]
            send_refresh_to_search();
        }
        UiResponse::SnippetDeleted(id) => {
            let _ = manager.with_store_mut(|store| store.delete(&id));
            #[cfg(target_os = "windows")]
            send_refresh_to_search();
        }
        UiResponse::RequestPull
        | UiResponse::RequestPush
        | UiResponse::Close => {}
    }
}

fn spawn_response_handler(resp_rx: crossbeam_channel::Receiver<UiResponse>) {
    std::thread::spawn(move || {
        while let Ok(response) = resp_rx.recv() {
            handle_ui_response(response);
        }
    });
}

#[cfg(target_os = "linux")]
fn show_create_snippet_dialog(folders: &[(String, String)], default_parent_id: Option<&str>) -> Option<(String, String, Option<String>)> {
    use gtk::prelude::*;
    
    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("新建 Snippet"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("创建", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(400, 350);
    dialog.set_position(gtk::WindowPosition::Center);
    
    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);
    
    let folder_combo = gtk::ComboBoxText::new();
    folder_combo.append_text("(根目录)");
    for (_fid, fname) in folders {
        folder_combo.append_text(fname);
    }
    
    let active_idx = match default_parent_id {
        Some(pid) => {
            if let Some(pos) = folders.iter().position(|(id, _)| id == pid) {
                (pos + 1) as u32
            } else {
                0
            }
        }
        None => 0,
    };
    folder_combo.set_active(Some(active_idx));
    let folder_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    folder_row.pack_start(&gtk::Label::new(Some("所属目录：")), false, true, 0);
    folder_row.pack_start(&folder_combo, true, true, 0);
    content.pack_start(&folder_row, false, true, 0);
    
    let name_label = gtk::Label::new(Some("名称："));
    name_label.set_xalign(0.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("片段名称"));
    content.pack_start(&name_label, false, true, 0);
    content.pack_start(&name_entry, false, true, 0);
    
    let content_label = gtk::Label::new(Some("内容："));
    content_label.set_xalign(0.0);
    content_label.set_margin_top(8);
    let content_view = gtk::TextView::new();
    content_view.set_wrap_mode(gtk::WrapMode::Word);
    let scrolled = gtk::ScrolledWindow::new(
        None::<&gtk::Adjustment>,
        None::<&gtk::Adjustment>,
    );
    scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrolled.set_min_content_height(200);
    scrolled.add(&content_view);
    content.pack_start(&content_label, false, true, 0);
    content.pack_start(&scrolled, true, true, 0);
    
    content.show_all();
    
    let response = dialog.run();
    
    let result = if response == gtk::ResponseType::Ok {
        let name = name_entry.text().to_string();
        let buffer = content_view.buffer().unwrap();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .unwrap_or_default()
            .to_string();
        let parent_id = match folder_combo.active() {
            Some(idx) if idx > 0 => {
                folders.get(idx as usize - 1).map(|(id, _)| id.clone())
            }
            _ => None,
        };
        
        if name.is_empty() {
            None
        } else {
            Some((name, text, parent_id))
        }
    } else {
        None
    };
    
    dialog.close();
    result
}

#[cfg(target_os = "linux")]
fn show_edit_snippet_dialog(
    existing_name: &str,
    existing_content: &str,
    folders: &[(String, String)],
    current_parent_id: Option<&str>,
) -> Option<(String, String, Option<String>)> {
    use gtk::prelude::*;
    
    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("编辑 Snippet"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("保存", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(400, 350);
    dialog.set_position(gtk::WindowPosition::Center);
    
    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);
    
    let folder_combo = gtk::ComboBoxText::new();
    folder_combo.append_text("(根目录)");
    for (_fid, fname) in folders {
        folder_combo.append_text(fname);
    }
    let initial_parent_idx = match current_parent_id {
        Some(pid) => {
            folders.iter().position(|(fid, _)| fid == pid).map(|i| i as u32 + 1).unwrap_or(0)
        }
        None => 0,
    };
    folder_combo.set_active(Some(initial_parent_idx));
    let folder_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    folder_row.pack_start(&gtk::Label::new(Some("所属目录：")), false, true, 0);
    folder_row.pack_start(&folder_combo, true, true, 0);
    content.pack_start(&folder_row, false, true, 0);
    
    let name_label = gtk::Label::new(Some("名称："));
    name_label.set_xalign(0.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_text(existing_name);
    name_entry.set_placeholder_text(Some("片段名称"));
    content.pack_start(&name_label, false, true, 0);
    content.pack_start(&name_entry, false, true, 0);
    
    let content_label = gtk::Label::new(Some("内容："));
    content_label.set_xalign(0.0);
    content_label.set_margin_top(8);
    let content_view = gtk::TextView::new();
    content_view.buffer().unwrap().set_text(existing_content);
    content_view.set_wrap_mode(gtk::WrapMode::Word);
    let scrolled = gtk::ScrolledWindow::new(
        None::<&gtk::Adjustment>,
        None::<&gtk::Adjustment>,
    );
    scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scrolled.set_min_content_height(200);
    scrolled.add(&content_view);
    content.pack_start(&content_label, false, true, 0);
    content.pack_start(&scrolled, true, true, 0);
    
    content.show_all();
    
    let response = dialog.run();
    
    let result = if response == gtk::ResponseType::Ok {
        let name = name_entry.text().to_string();
        let buffer = content_view.buffer().unwrap();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .unwrap_or_default()
            .to_string();
        let parent_id = match folder_combo.active() {
            Some(idx) if idx > 0 => {
                folders.get(idx as usize - 1).map(|(id, _)| id.clone())
            }
            _ => None,
        };
        
        if name.is_empty() {
            None
        } else {
            Some((name, text, parent_id))
        }
    } else {
        None
    };
    
    dialog.close();
    result
}

#[cfg(target_os = "linux")]
fn show_delete_snippet_dialog(name: &str) -> bool {
    use gtk::prelude::*;
    
    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("确认删除"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("删除", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(300, 100);
    dialog.set_position(gtk::WindowPosition::Center);
    
    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    
    let message = format!("确定要删除 '{}' 吗？", name);
    let label = gtk::Label::new(Some(&message));
    label.set_xalign(0.5);
    content.pack_start(&label, false, true, 0);
    
    content.show_all();
    
    let response = dialog.run();
    dialog.close();
    
    response == gtk::ResponseType::Ok
}

#[cfg(target_os = "linux")]
fn show_edit_folder_dialog(existing_name: &str) -> Option<String> {
    use gtk::prelude::*;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("编辑文件夹"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("取消", gtk::ResponseType::Cancel),
            ("保存", gtk::ResponseType::Ok),
        ],
    );
    dialog.set_default_size(400, 120);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(10);
    content.set_margin_end(10);

    let name_label = gtk::Label::new(Some("名称："));
    name_label.set_xalign(0.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_text(existing_name);
    name_entry.set_placeholder_text(Some("文件夹名称"));
    content.pack_start(&name_label, false, true, 0);
    content.pack_start(&name_entry, false, true, 0);

    content.show_all();

    let response = dialog.run();
    dialog.close();

    if response == gtk::ResponseType::Ok {
        let name = name_entry.text().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn launch_snippet_dialog_linux(
    _cmd_tx: crossbeam_channel::Sender<UiCommand>,
) {
    use gtk::prelude::*;
    use snippet::model::SnippetType;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("Snippet 管理器"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("关闭", gtk::ResponseType::Close)],
    );
    dialog.set_default_size(800, 550);
    dialog.set_position(gtk::WindowPosition::Center);

    let content = dialog.content_area();

    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some("搜索片段..."));
    entry.set_margin_bottom(6);
    content.pack_start(&entry, false, true, 4);

    let hpaned = gtk::Paned::new(gtk::Orientation::Horizontal);
    hpaned.set_position(280);

    let left_scroll = gtk::ScrolledWindow::new(
        None::<&gtk::Adjustment>,
        None::<&gtk::Adjustment>,
    );
    left_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);

    let store = gtk::TreeStore::new(&[
        String::static_type(),  // 0: id
        String::static_type(),  // 1: name
        String::static_type(),  // 2: content
        bool::static_type(),    // 3: is_folder
        String::static_type(),  // 4: icon_name
    ]);

    let snippets = match MANAGER.get().and_then(|m| m.lock().ok()) {
        Some(guard) => guard
            .with_store(|s| s.list().to_vec())
            .unwrap_or_default(),
        None => Vec::new(),
    };

    let folders: Vec<_> = snippets
        .iter()
        .filter(|s| !s.deleted && s.snippet_type == SnippetType::Folder)
        .collect();
    let all_snippets: Vec<_> = snippets
        .iter()
        .filter(|s| !s.deleted && s.snippet_type == SnippetType::Snippet)
        .collect();

    eprintln!("[DEBUG] folders count: {}", folders.len());
    eprintln!("[DEBUG] all_snippets count: {}", all_snippets.len());

    for folder in &folders {
        let parent_iter = store.insert_with_values(None, None, &[
            (0, &folder.id),
            (1, &folder.name),
            (2, &""),
            (3, &true),
            (4, &"folder-symbolic"),
        ]);
        let mut child_count = 0;
        for s in &all_snippets {
            if s.parent_id.as_deref() == Some(&folder.id) {
                store.insert_with_values(Some(&parent_iter), None, &[
                    (0, &s.id),
                    (1, &s.name),
                    (2, &s.content.clone().unwrap_or_default()),
                    (3, &false),
                    (4, &"text-x-script-symbolic"),
                ]);
                child_count += 1;
            }
        }
        eprintln!("[DEBUG] folder '{}' has {} children", folder.name, child_count);
    }

    for s in &all_snippets {
        if s.parent_id.is_none() || !folders.iter().any(|f| Some(&f.id) == s.parent_id.as_ref()) {
            store.insert_with_values(None, None, &[
                (0, &s.id),
                (1, &s.name),
                (2, &s.content.clone().unwrap_or_default()),
                (3, &false),
                (4, &"text-x-script-symbolic"),
            ]);
        }
    }
    eprintln!("[DEBUG] tree store populated, total rows will be shown after expand_all");

    let tree_view = gtk::TreeView::with_model(&store);

    let refresh_tree_store = {
        let store = store.clone();
        let tree_view = tree_view.clone();
        move |query: Option<String>, select_id: Option<String>| {
            store.clear();
            let snippets = match MANAGER.get().and_then(|m| m.lock().ok()) {
                Some(guard) => guard.with_store(|s| s.list().to_vec()).unwrap_or_default(),
                None => Vec::new(),
            };
            let all_items: Vec<_> = snippets.iter().filter(|s| !s.deleted).collect();
            let folders: Vec<_> = all_items
                .iter()
                .filter(|s| s.snippet_type == SnippetType::Folder)
                .cloned()
                .collect();
            let all_snippets: Vec<_> = all_items
                .iter()
                .filter(|s| s.snippet_type == SnippetType::Snippet)
                .cloned()
                .collect();

            let q = query
                .and_then(|q| if q.is_empty() { None } else { Some(q.to_lowercase()) });

            let matching_snippet_ids: std::collections::HashSet<String> = if let Some(ref query_lower) = q {
                all_snippets
                    .iter()
                    .filter(|s| {
                        s.name.to_lowercase().contains(query_lower)
                            || s.content
                                .as_ref()
                                .map(|c| c.to_lowercase().contains(query_lower))
                                .unwrap_or(false)
                    })
                    .map(|s| s.id.clone())
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

            let folders_to_show: std::collections::HashSet<String> = if let Some(ref query_lower) = q {
                let mut set = std::collections::HashSet::new();
                for folder in &folders {
                    if folder.name.to_lowercase().contains(query_lower) {
                        set.insert(folder.id.clone());
                    }
                    if all_snippets.iter().any(|s| {
                        s.parent_id.as_deref() == Some(&folder.id)
                            && matching_snippet_ids.contains(&s.id)
                    }) {
                        set.insert(folder.id.clone());
                    }
                }
                set
            } else {
                folders.iter().map(|f| f.id.clone()).collect()
            };

            for folder in &folders {
                if !folders_to_show.contains(&folder.id) {
                    continue;
                }
                let parent_iter = store.insert_with_values(None, None, &[
                    (0, &folder.id),
                    (1, &folder.name),
                    (2, &""),
                    (3, &true),
                    (4, &"folder-symbolic"),
                ]);
                for s in &all_snippets {
                    if s.parent_id.as_deref() == Some(&folder.id) {
                        let should_show = if q.is_some() {
                            matching_snippet_ids.contains(&s.id)
                        } else {
                            true
                        };
                        if should_show {
                            store.insert_with_values(Some(&parent_iter), None, &[
                                (0, &s.id),
                                (1, &s.name),
                                (2, &s.content.clone().unwrap_or_default()),
                                (3, &false),
                                (4, &"text-x-script-symbolic"),
                            ]);
                        }
                    }
                }
            }

            for s in &all_snippets {
                if s.parent_id.is_none() {
                    let should_show = if q.is_some() {
                        matching_snippet_ids.contains(&s.id)
                    } else {
                        true
                    };
                    if should_show {
                        store.insert_with_values(None, None, &[
                            (0, &s.id),
                            (1, &s.name),
                            (2, &s.content.clone().unwrap_or_default()),
                            (3, &false),
                            (4, &"text-x-script-symbolic"),
                        ]);
                    }
                }
            }

            tree_view.expand_all();

            if let Some(id) = select_id {
                if let Some(model) = tree_view.model() {
                    if let Some(ts) = model.downcast_ref::<gtk::TreeStore>() {
                        ts.foreach(|model, _path, iter| {
                            let row_id: String =
                                model.value(iter, 0).get::<String>().unwrap_or_default();
                            if row_id == id {
                                tree_view.selection().select_iter(iter);
                                return false;
                            }
                            true
                        });
                    }
                }
            }
        }
    };

    let tree_view_ref = tree_view.clone();
    let search_entry = entry.clone();
    let refresh_for_search = refresh_tree_store.clone();
    search_entry.connect_changed(move |entry| {
        let query = entry.text().to_string();
        refresh_for_search(if query.is_empty() { None } else { Some(query) }, None);
    });
    tree_view.set_headers_visible(false);
    let column = gtk::TreeViewColumn::new();
    let icon_renderer = gtk::CellRendererPixbuf::new();
    let text_renderer = gtk::CellRendererText::new();
    gtk::prelude::TreeViewColumnExt::pack_start(&column, &icon_renderer, false);
    gtk::prelude::TreeViewColumnExt::pack_start(&column, &text_renderer, true);
    gtk::prelude::CellLayoutExt::add_attribute(&column, &icon_renderer, "icon-name", 4);
    gtk::prelude::CellLayoutExt::add_attribute(&column, &text_renderer, "text", 1);
    tree_view.append_column(&column);
    left_scroll.add(&tree_view);

    let right_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let right_label = gtk::Label::new(Some("预览"));
    right_label.set_xalign(0.0);
    right_label.style_context().add_class("dim-label");
    right_box.pack_start(&right_label, false, true, 0);

    let preview_scroll = gtk::ScrolledWindow::new(
        None::<&gtk::Adjustment>,
        None::<&gtk::Adjustment>,
    );
    preview_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    let preview_text = gtk::TextView::new();
    preview_text.set_editable(false);
    preview_text.set_cursor_visible(false);
    preview_text.set_wrap_mode(gtk::WrapMode::Word);
    preview_text.set_monospace(true);
    preview_scroll.add(&preview_text);
    right_box.pack_start(&preview_scroll, true, true, 0);

    hpaned.pack1(&left_scroll, true, true);
    hpaned.pack2(&right_box, true, true);
    content.pack_start(&hpaned, true, true, 0);

    let new_button = gtk::Button::with_label("新增 Snippet");
    new_button.style_context().add_class("suggested-action");
    let new_folder_button = gtk::Button::with_label("新增目录");
    let edit_button = gtk::Button::with_label("编辑");
    let delete_button = gtk::Button::with_label("删除");
    delete_button.style_context().add_class("destructive-action");
    let copy_button = gtk::Button::with_label("复制");
    copy_button.style_context().add_class("suggested-action");
    copy_button.set_sensitive(false);

    let btn_box = gtk::ButtonBox::new(gtk::Orientation::Horizontal);
    btn_box.set_layout(gtk::ButtonBoxStyle::End);
    btn_box.set_spacing(6);
    btn_box.set_margin_top(6);
    btn_box.pack_start(&new_folder_button, false, true, 0);
    btn_box.pack_start(&new_button, false, true, 0);
    btn_box.pack_start(&edit_button, false, true, 0);
    btn_box.pack_start(&delete_button, false, true, 0);
    btn_box.pack_start(&copy_button, false, true, 0);
    content.pack_start(&btn_box, false, true, 0);

    content.show_all();

    tree_view.expand_all();
    tree_view.set_enable_tree_lines(true);
    tree_view.set_show_expanders(true);

    let target_entry = gtk::TargetEntry::new("UTF8_STRING", gtk::TargetFlags::empty(), 0);
    let drop_tree_view = tree_view.clone();
    tree_view.add_events(
        gtk::gdk::EventMask::BUTTON_PRESS_MASK
        | gtk::gdk::EventMask::BUTTON_RELEASE_MASK
        | gtk::gdk::EventMask::BUTTON_MOTION_MASK
        | gtk::gdk::EventMask::POINTER_MOTION_MASK
    );
    tree_view.drag_source_set(
        gtk::gdk::ModifierType::BUTTON1_MASK,
        &[target_entry.clone()],
        gtk::gdk::DragAction::MOVE,
    );
    tree_view.drag_source_set_icon_name("text-x-script-symbolic");
    tree_view.connect_drag_begin(move |_, drag_context| {
        eprintln!("[DEBUG] drag_begin called, context: {:?}", drag_context);
    });
    tree_view.connect_drag_data_get(move |_, _, data, info, _| {
        eprintln!("[DEBUG] drag_data_get called: info={}", info);
        if info == 0 {
            let tv = drop_tree_view.clone();
            if let Some(sel) = tv.selection().selected() {
                let (model, iter) = sel;
                if let Some(ts) = model.downcast_ref::<gtk::TreeStore>() {
                    let is_folder: bool = ts.value(&iter, 3).get::<bool>().unwrap_or_default();
                    eprintln!("[DEBUG] drag_data_get: is_folder={}", is_folder);
                    if is_folder { return; }
                    let id: String = ts.value(&iter, 0).get::<String>().unwrap_or_default();
                    eprintln!("[DEBUG] drag_data_get: storing id '{}' in DRAG_DATA", id);
                    if let Ok(mut drag_data) = DRAG_DATA.lock() {
                        *drag_data = Some(id);
                    }
                }
            }
        }
    });
    let dest_tree_view = tree_view.clone();
    let refresh_for_drag = refresh_tree_store.clone();
    tree_view.drag_dest_set(
        gtk::DestDefaults::empty(),
        &[target_entry],
        gtk::gdk::DragAction::MOVE,
    );
    tree_view.connect_drag_drop(move |tv, dc, _x, _y, time| {
        eprintln!("[DEBUG] drag_drop called");
        let target = tv.drag_dest_find_target(dc, None);
        eprintln!("[DEBUG] drag_drop: found target = {:?}", target.is_some());
        if let Some(target) = target {
            tv.drag_get_data(dc, &target, time);
            true
        } else {
            false
        }
    });
    tree_view.connect_drag_motion(move |_tv, dc, _x, _y, time| {
        dc.drag_status(gtk::gdk::DragAction::MOVE, time);
        true
    });
    tree_view.connect_drag_data_received(move |tv, _dc, x, y, _data, _info, _| {
        eprintln!("[DEBUG] drag_data_received: x={}, y={}", x, y);
        let dragged_id = match DRAG_DATA.lock().ok().and_then(|mut data| data.take()) {
            Some(id) => id,
            None => {
                eprintln!("[DEBUG] drag_data_received: DRAG_DATA is empty");
                return;
            }
        };
        eprintln!("[DEBUG] dragged_id={}", dragged_id);
        let drop_opt = dest_tree_view.dest_row_at_pos(x, y);
        eprintln!("[DEBUG] drop_opt present = {}", drop_opt.is_some());
        match drop_opt {
            Some((Some(ref path), gtk::TreeViewDropPosition::IntoOrBefore)) 
            | Some((Some(ref path), gtk::TreeViewDropPosition::IntoOrAfter)) => {
                let model = dest_tree_view.model().unwrap();
                let ts = match model.downcast_ref::<gtk::TreeStore>() {
                    Some(m) => m,
                    None => return,
                };
                if let Some(iter) = ts.iter(&path) {
                    let target_is_folder: bool = ts.value(&iter, 3).get::<bool>().unwrap_or_default();
                    if !target_is_folder { return; }
                    let target_id: String = ts.value(&iter, 0).get::<String>().unwrap_or_default();
                    eprintln!("[DEBUG] moving {} into folder {}", dragged_id, target_id);
                    if let Some(manager) = MANAGER.get().and_then(|m| m.lock().ok()) {
                        let _ = manager.with_store_mut(|s| {
                            let _ = s.update(&dragged_id, |snip| {
                                snip.parent_id = Some(target_id);
                            });
                        });
                    }
                    refresh_for_drag(None, Some(dragged_id.clone()));
                }
            }
            _ => {
                eprintln!("[DEBUG] moving {} to root", dragged_id);
                if let Some(manager) = MANAGER.get().and_then(|m| m.lock().ok()) {
                    let _ = manager.with_store_mut(|s| {
                        let _ = s.update(&dragged_id, |snip| {
                            snip.parent_id = None;
                        });
                    });
                }
                refresh_for_drag(None, Some(dragged_id.clone()));
            }
        }
    });

    let selection = tree_view.selection();
    selection.set_mode(gtk::SelectionMode::Single);

    let preview_text_ref = preview_text.clone();
    let copy_button_ref = copy_button.clone();
    let selected_content: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_id: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_is_folder: std::sync::Arc<std::sync::Mutex<Option<bool>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_name: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let selected_parent_id: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));

    let sc_clone = selected_content.clone();
    let si_clone = selected_id.clone();
    let sf_clone = selected_is_folder.clone();
    let sn_clone = selected_name.clone();
    let sp_clone = selected_parent_id.clone();
    selection.connect_changed(move |sel| {
        if let Some((model, iter)) = sel.selected() {
            if let Some(tree_store) = model.downcast_ref::<gtk::TreeStore>() {
                let id: String = tree_store.value(&iter, 0).get::<String>().unwrap_or_default();
                let name: String = tree_store.value(&iter, 1).get::<String>().unwrap_or_default();
                let content: String = tree_store.value(&iter, 2).get::<String>().unwrap_or_default();
                let is_folder: bool = tree_store.value(&iter, 3).get::<bool>().unwrap_or_default();
                let parent_id: Option<String> = tree_store.value(&iter, 0).get::<String>().ok();
                *sc_clone.lock().unwrap() = Some(content.clone());
                *si_clone.lock().unwrap() = Some(id);
                *sf_clone.lock().unwrap() = Some(is_folder);
                *sn_clone.lock().unwrap() = Some(name);
                *sp_clone.lock().unwrap() = parent_id;
                if is_folder {
                    if let Some(buffer) = preview_text_ref.buffer() { buffer.set_text("(文件夹)"); }
                    copy_button_ref.set_sensitive(false);
                } else {
                    if let Some(buffer) = preview_text_ref.buffer() { buffer.set_text(&content); }
                    copy_button_ref.set_sensitive(true);
                }
            }
        }
    });

    let dialog_for_dblclick = dialog.clone();
    tree_view.connect_row_activated(move |tv, path, _col| {
        if let Some(model) = tv.model() {
            if let Some(ts) = model.downcast_ref::<gtk::TreeStore>() {
                if let Some(iter) = ts.iter(path) {
                    let is_folder: bool = ts.value(&iter, 3).get::<bool>().unwrap_or_default();
                    if is_folder {
                        if tv.row_expanded(path) {
                            tv.collapse_row(path);
                        } else {
                            tv.expand_row(path, false);
                        }
                    } else {
                        let content: String = ts.value(&iter, 2).get::<String>().unwrap_or_default();
                        let name: String = ts.value(&iter, 1).get::<String>().unwrap_or_default();
                        if !content.is_empty() {
                            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
                            clipboard.set_text(&content);
                            clipboard.store();
                            let _ = notify_rust::Notification::new()
                                .summary("Snippet Copied")
                                .body(&format!("已复制: {}", name.chars().take(30).collect::<String>()))
                                .show();
                            if let Some(d) = dialog_for_dblclick.downcast_ref::<gtk::Dialog>() {
                                d.response(gtk::ResponseType::Close);
                            }
                        }
                    }
                }
            }
        }
    });

    let dialog_for_copy = dialog.clone();
    let sc_for_copy = selected_content.clone();
    copy_button.connect_clicked(move |_| {
        let content = sc_for_copy.lock().unwrap().clone();
        if let Some(c) = content {
            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
            clipboard.set_text(&c);
            clipboard.store();
            let _ = notify_rust::Notification::new()
                .summary("Snippet Copied")
                .body(&format!("已复制: {}", c.chars().take(30).collect::<String>()))
                .show();
            if let Some(d) = dialog_for_copy.downcast_ref::<gtk::Dialog>() { d.response(gtk::ResponseType::Close); }
        }
    });

    let dialog_for_new_folder = dialog.clone();
    new_folder_button.connect_clicked(move |_| {
        eprintln!("[DEBUG] new_folder_button clicked");
        if let Some(d) = dialog_for_new_folder.downcast_ref::<gtk::Dialog>() {
            eprintln!("[DEBUG] sending Accept response for new folder");
            d.response(gtk::ResponseType::Accept);
        } else {
            eprintln!("[DEBUG] failed to downcast dialog for new folder");
        }
    });

    let dialog_for_new_snippet = dialog.clone();
    new_button.connect_clicked(move |_| {
        if let Some(d) = dialog_for_new_snippet.downcast_ref::<gtk::Dialog>() { d.response(gtk::ResponseType::Apply); }
    });

    let dialog_for_edit = dialog.clone();
    let si_for_edit = selected_id.clone();
    let sf_for_edit = selected_is_folder.clone();
    let sn_for_edit = selected_name.clone();
    let sc_for_edit = selected_content.clone();
    let refresh_for_edit = refresh_tree_store.clone();
    edit_button.connect_clicked(move |_| {
        let is_folder = sf_for_edit.lock().unwrap().unwrap_or(false);
        let id_opt = si_for_edit.lock().unwrap().clone();
        let name_opt = sn_for_edit.lock().unwrap().clone();
        if let Some(_id) = id_opt {
            let name = name_opt.unwrap_or_default();
            if is_folder {
                eprintln!("[DEBUG] editing folder: {}", _id);
                if let Some(new_name) = show_edit_folder_dialog(&name) {
                    {
                        if let Some(manager_guard) = MANAGER.get().and_then(|m| m.lock().ok()) {
                            let manager = &*manager_guard;
                            let _ = manager.with_store_mut(|store| {
                                store.update(&_id, |s| {
                                    s.name = new_name.clone();
                                })
                            });
                        }
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("文件夹已更新")
                        .body(&format!("'{}' 已重命名", new_name))
                        .show();
                    refresh_for_edit(None, Some(_id.clone()));
                }
            } else {
                eprintln!("[DEBUG] editing snippet: {}", _id);
                let content = sc_for_edit.lock().unwrap().clone().unwrap_or_default();
                let folders_data: Vec<(String, String)> = if let Some(mg) = MANAGER.get().and_then(|m| m.lock().ok()) {
                    mg.with_store(|s| {
                        s.list().iter()
                            .filter(|sn| !sn.deleted && sn.snippet_type == SnippetType::Folder)
                            .map(|sn| (sn.id.clone(), sn.name.clone()))
                            .collect()
                    }).unwrap_or_default()
                } else {
                    Vec::new()
                };
                let parent_id_opt: Option<String> = if let Some(mg) = MANAGER.get().and_then(|m| m.lock().ok()) {
                    mg.with_store(|s| {
                        s.get(&_id).map(|sn| sn.parent_id.clone()).flatten()
                    }).ok().flatten()
                } else {
                    None
                };
                if let Some((new_name, new_content, new_parent_id)) = show_edit_snippet_dialog(&name, &content, &folders_data, parent_id_opt.as_deref()) {
                    {
                        if let Some(manager_guard) = MANAGER.get().and_then(|m| m.lock().ok()) {
                            let manager = &*manager_guard;
                            let _ = manager.with_store_mut(|store| {
                                store.update(&_id, |s| {
                                    s.name = new_name.clone();
                                    s.content = Some(new_content.clone());
                                    s.parent_id = new_parent_id.clone();
                                })
                            });
                        }
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("Snippet 已更新")
                        .body(&format!("'{}' 已保存", new_name))
                        .show();
                    refresh_for_edit(None, Some(_id.clone()));
                }
            }
        }
    });

    let dialog_for_del = dialog.clone();
    let si_for_del = selected_id.clone();
    let sn_for_del = selected_name.clone();
    let sf_for_del = selected_is_folder.clone();
    let refresh_for_del = refresh_tree_store.clone();
    delete_button.connect_clicked(move |_| {
        let is_folder = sf_for_del.lock().unwrap().unwrap_or(false);
        let id_opt = si_for_del.lock().unwrap().clone();
        let name_opt = sn_for_del.lock().unwrap().clone();
        if let (Some(id), Some(name)) = (id_opt, name_opt) {
            if show_delete_snippet_dialog(&name) {
                {
                    if let Some(manager_guard) = MANAGER.get().and_then(|m| m.lock().ok()) {
                        let manager = &*manager_guard;
                        let _ = manager.with_store_mut(|store| store.delete(&id));
                    }
                }
                let _ = notify_rust::Notification::new()
                    .summary("Snippet 已删除")
                    .body(&format!("'{}' 已删除", name))
                    .show();
                refresh_for_del(None, None);
            }
        }
    });

    dialog.show_all();
    let refresh = refresh_tree_store.clone();
    let dialog_for_response = dialog.clone();
    let selected_parent_id_for_create = selected_parent_id.clone();
    dialog.connect_response(move |_, response| {
        match response {
            gtk::ResponseType::Accept => {
                eprintln!("[DEBUG] Accept response");
                let name_input = gtk::Dialog::with_buttons::<gtk::Window>(
                    Some("新增目录"),
                    Some(dialog_for_response.upcast_ref::<gtk::Window>()),
                    gtk::DialogFlags::MODAL,
                    &[("取消", gtk::ResponseType::Cancel), ("创建", gtk::ResponseType::Ok)],
                );
                let entry = gtk::Entry::new();
                entry.set_placeholder_text(Some("目录名称"));
                let ca = name_input.content_area();
                ca.set_margin_start(10);
                ca.set_margin_end(10);
                ca.add(&entry);
                ca.show_all();
                let folder_response = name_input.run();
                eprintln!("[DEBUG] new folder dialog response: {:?}", folder_response);
                if folder_response == gtk::ResponseType::Ok {
                    let folder_name = entry.text().to_string();
                    eprintln!("[DEBUG] creating folder: '{}'", folder_name);
                    if !folder_name.is_empty() {
                        if let Some(mg) = MANAGER.get().and_then(|m| m.lock().ok()) {
                            let manager = &*mg;
                            let folder = crate::snippet::model::Snippet::new_folder(folder_name.clone(), None);
                            if let Err(e) = manager.with_store_mut(|store| store.add(folder)) {
                                let _ = notify_rust::Notification::new()
                                    .summary("创建失败")
                                    .body(&format!("{}", e))
                                    .show();
                            } else {
                                let _ = notify_rust::Notification::new()
                                    .summary("目录已创建")
                                    .body(&format!("'{}' 已保存", folder_name))
                                    .show();
                            }
                        }
                    }
                }
                name_input.close();
                refresh(None, None);
            }
            gtk::ResponseType::Apply => {
                eprintln!("[DEBUG] Apply response");
                let folder_data: Vec<(String, String)> = {
                    if let Some(mg) = MANAGER.get().and_then(|m| m.lock().ok()) {
                        mg.with_store(|s| {
                            s.list().iter()
                                .filter(|sn| !sn.deleted && sn.snippet_type == SnippetType::Folder)
                                .map(|sn| (sn.id.clone(), sn.name.clone()))
                                .collect()
                        }).unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                };
                if let Some((name, text_content, parent_id)) = show_create_snippet_dialog(&folder_data, selected_parent_id_for_create.lock().unwrap().as_deref()) {
                    if let Some(mg) = MANAGER.get().and_then(|m| m.lock().ok()) {
                        let manager = &*mg;
                        let snippet = crate::snippet::model::Snippet::new_snippet(name.clone(), text_content, parent_id);
                        if let Err(e) = manager.with_store_mut(|store| store.add(snippet)) {
                            let _ = notify_rust::Notification::new()
                                .summary("创建失败")
                                .body(&format!("{}", e))
                                .show();
                        } else {
                            let _ = notify_rust::Notification::new()
                                .summary("Snippet 已创建")
                                .body(&format!("'{}' 已保存", name))
                                .show();
                        }
                    }
                }
                refresh(None, None);
            }
            gtk::ResponseType::Close => {
                eprintln!("[DEBUG] Close response");
                dialog_for_response.hide();
                gtk::main_quit();
            }
            _ => {
                eprintln!("[DEBUG] Unhandled response: {:?}", response);
            }
        }
    });
    gtk::main();
}

#[allow(unused_variables)]
fn main() {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<UiCommand>();
    let (resp_tx, resp_rx) = crossbeam_channel::unbounded::<UiResponse>();
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    match SnippetManager::new() {
        Ok(manager) => {
            manager.start_background_sync();
            let _ = MANAGER.set(Mutex::new(manager));
        }
        Err(e) => {
            let _ = notify_rust::Notification::new()
                .summary("Snippet 初始化失败")
                .body(&format!("无法启动 Snippet 功能: {}", e))
                .show();
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = CMD_TX.set(cmd_tx.clone());
    }

    #[cfg(target_os = "windows")]
    {
        let resp_tx_clone = resp_tx.clone();
        let running_clone = running.clone();
        std::thread::spawn(move || {
            launch_egui(cmd_rx, resp_tx_clone, running_clone);
        });
    }

    #[cfg(target_os = "linux")]
    {
        let cmd_rx_for_linux = cmd_rx;
        std::thread::spawn(move || {
            while let Ok(cmd) = cmd_rx_for_linux.recv() {
                match cmd {
                    UiCommand::Show => {
                        if DIALOG_OPEN.load(std::sync::atomic::Ordering::SeqCst) {
                            continue;
                        }
                        DIALOG_OPEN.store(true, std::sync::atomic::Ordering::SeqCst);
                        let _ = std::thread::spawn(|| {
                            launch_snippet_dialog_linux(crossbeam_channel::unbounded().0);
                            DIALOG_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
                        });
                    }
                    _ => {}
                }
            }
        });
    }

    launch_global_hotkey(cmd_tx.clone());

    spawn_response_handler(resp_rx);

    #[cfg(target_os = "linux")]
    platform::main(cmd_tx);
    #[cfg(target_os = "windows")]
    platform::main(cmd_tx, running);
}
