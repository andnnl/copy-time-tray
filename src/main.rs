use chrono::Local;
use md5::{Digest, Md5};
use rand::Rng;

mod snippet;

use snippet::SnippetManager;
use snippet::ui::{UiCommand, UiResponse};
#[cfg(target_os = "windows")]
use snippet::ui::search::SearchPopup;
use std::sync::Arc;

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
                let manager_result = SnippetManager::new();
                let manager = match manager_result {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = notify_rust::Notification::new()
                            .summary("Snippet Sync Failed")
                            .body(&format!("创建管理器失败: {}", e))
                            .show();
                        return;
                    }
                };

                if !manager.is_configured() {
                    glib::idle_add(move || {
                        let _ = config_dialog::show_config_dialog(None);
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

        let snippet_item = MenuItem::with_label("搜索 Snippet");
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

    pub fn main(_cmd_tx: crossbeam_channel::Sender<UiCommand>) {
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
        let snippet_item = MenuItem::with_id(snippet_id.clone(), "搜索 Snippet", true, None);
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
                            let _ = notify_rust::Notification::new()
                                .summary("同步")
                                .body("同步功能待实现")
                                .show();
                        }
                        "search_snippets" => {
                            let _ = notify_rust::Notification::new()
                                .summary("搜索")
                                .body("搜索功能待实现")
                                .show();
                        }
                        "quit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
            }
        });

        // Windows 保持进程存活
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
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
        UiResponse::SnippetCreated(_)
        | UiResponse::SnippetUpdated(_)
        | UiResponse::SnippetDeleted(_)
        | UiResponse::RequestPull
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
fn launch_snippet_dialog_linux(
    _cmd_tx: crossbeam_channel::Sender<UiCommand>,
) {
    use gtk::prelude::*;
    use snippet::model::SnippetType;

    let dialog = gtk::Dialog::with_buttons::<gtk::Window>(
        Some("搜索 Snippet"),
        None,
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("关闭", gtk::ResponseType::Close)],
    );
    dialog.set_default_size(500, 350);

    let content = dialog.content_area();

    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some("搜索片段..."));
    content.pack_start(&entry, false, true, 4);

    let scrolled = gtk::ScrolledWindow::new(
        None::<&gtk::Adjustment>,
        None::<&gtk::Adjustment>,
    );
    scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    scrolled.add(&list_box);
    content.pack_start(&scrolled, true, true, 4);

    let snippets = match SnippetManager::new() {
        Ok(manager) => {
            match manager.with_store(|store| {
                let snippets = store.list();
                snippets.to_vec()
            }) {
                Ok(snippets) => {
                    eprintln!("[DEBUG] Loaded {} snippets from store", snippets.len());
                    snippets
                },
                Err(e) => {
                    eprintln!("[ERROR] Failed to access store: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            eprintln!("[ERROR] Failed to create SnippetManager: {}", e);
            Vec::new()
        }
    };

    let mut snippet_count = 0;
    for snippet in &snippets {
        eprintln!("[DEBUG] Processing snippet: name='{}', type={:?}, deleted={}", 
            snippet.name, snippet.snippet_type, snippet.deleted);
        if snippet.deleted {
            eprintln!("[DEBUG] Skipping deleted snippet: {}", snippet.name);
            continue;
        }
        if snippet.snippet_type != SnippetType::Snippet {
            eprintln!("[DEBUG] Skipping non-snippet type: {}", snippet.name);
            continue;
        }
        let row = gtk::ListBoxRow::new();
        let label = gtk::Label::new(Some(&snippet.name));
        label.set_xalign(0.0);
        label.set_margin_start(6);
        row.add(&label);
        let preview: String = snippet
            .content
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();
        row.set_tooltip_text(Some(&preview));
        list_box.add(&row);
        snippet_count += 1;
    }

    if snippet_count == 0 {
        let empty_label = gtk::Label::new(Some("暂无数据，请先创建 snippet"));
        empty_label.set_justify(gtk::Justification::Center);
        empty_label.set_margin_top(20);
        empty_label.set_margin_bottom(20);
        empty_label.style_context().add_class("dim-label");
        content.pack_start(&empty_label, true, true, 0);
    }

    content.show_all();

    let list_box_clone = list_box.clone();
    entry.connect_changed(move |e| {
        let text = e.text().to_string().to_lowercase();
        for child in list_box_clone.children() {
            if let Some(row) = child.downcast_ref::<gtk::ListBoxRow>() {
                if let Some(c) = row.child() {
                    if let Some(lbl) = c.downcast_ref::<gtk::Label>() {
                        let name = lbl.text().to_lowercase();
                        row.set_visible(text.is_empty() || name.contains(&text));
                    }
                }
            }
        }
    });

    let dialog_clone = dialog.clone();
    list_box.connect_row_activated(move |_, row| {
        let preview = row
            .tooltip_text()
            .unwrap_or_default();
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        clipboard.set_text(&preview);
        clipboard.store();
        let _ = notify_rust::Notification::new()
            .summary("Snippet Copied")
            .body(&format!("已复制: {}", &preview.chars().take(30).collect::<String>()))
            .show();
        dialog_clone.response(gtk::ResponseType::Close);
    });

    entry.set_activates_default(true);
    let _ = dialog.run();
    dialog.hide();
}

#[allow(unused_variables)]
fn main() {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<UiCommand>();
    let (resp_tx, resp_rx) = crossbeam_channel::unbounded::<UiResponse>();
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    if let Ok(manager) = SnippetManager::new() {
        manager.start_background_sync();
    }

    #[cfg(target_os = "windows")]
    {
        let resp_tx_clone = resp_tx.clone();
        std::thread::spawn(move || {
            launch_egui(cmd_rx, resp_tx_clone, running);
        });
    }

    launch_global_hotkey(cmd_tx.clone());

    spawn_response_handler(resp_rx);

    #[cfg(target_os = "linux")]
    platform::main(cmd_tx);
    #[cfg(target_os = "windows")]
    platform::main(cmd_tx);
}
