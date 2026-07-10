use crate::app::{send_refresh_to_search, UiCommand};
use crate::utilities::{generate_md5, generate_password, get_time_text};
use arboard::Clipboard;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use notify_rust::Notification;
use std::sync::Arc;
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
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
            if x >= 15 && x <= 17 && y >= 6 && y <= 16 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
            if y >= 15 && y <= 17 && x >= 16 && x <= 24 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    }
    rgba
}

pub fn main(
    cmd_tx: crossbeam_channel::Sender<UiCommand>,
    running: Arc<std::sync::atomic::AtomicBool>,
) {
    let menu = Menu::new();

    // 复制时间菜单项
    let full_id = muda::MenuId::new("full");
    let date_id = muda::MenuId::new("date");
    let compact_id = muda::MenuId::new("compact");
    let full_item = MenuItem::with_id(full_id.clone(), "复制完整时间", true, None);
    let date_item = MenuItem::with_id(date_id.clone(), "复制日期", true, None);
    let compact_item = MenuItem::with_id(compact_id.clone(), "复制纯数字", true, None);
    menu.append_items(&[
        &full_item,
        &date_item,
        &compact_item,
        &PredefinedMenuItem::separator(),
    ])
    .unwrap();

    // 密码生成子菜单
    let password_submenu = Submenu::new("密码", true);
    let pwd8_id = muda::MenuId::new("pwd8");
    let pwd12_id = muda::MenuId::new("pwd12");
    let pwd16_id = muda::MenuId::new("pwd16");
    let pwd8_item = MenuItem::with_id(pwd8_id.clone(), "8位密码", true, None);
    let pwd12_item = MenuItem::with_id(pwd12_id.clone(), "12位密码", true, None);
    let pwd16_item = MenuItem::with_id(pwd16_id.clone(), "16位密码", true, None);
    password_submenu
        .append_items(&[&pwd8_item, &pwd12_item, &pwd16_item])
        .unwrap();
    menu.append(&password_submenu).unwrap();

    // MD5 子菜单
    let md5_submenu = Submenu::new("MD5", true);
    let md5_16_id = muda::MenuId::new("md5_16");
    let md5_32_id = muda::MenuId::new("md5_32");
    let md5_16_item = MenuItem::with_id(md5_16_id.clone(), "16位MD5", true, None);
    let md5_32_item = MenuItem::with_id(md5_32_id.clone(), "32位MD5", true, None);
    md5_submenu
        .append_items(&[&md5_16_item, &md5_32_item])
        .unwrap();
    menu.append(&md5_submenu).unwrap();

    menu.append(&PredefinedMenuItem::separator()).unwrap();

    let config_id = muda::MenuId::new("open_config");
    let config_item = MenuItem::with_id(config_id.clone(), "配置", true, None);
    let snippet_id = muda::MenuId::new("search_snippets");
    let snippet_item = MenuItem::with_id(snippet_id.clone(), "代码片断", true, None);
    let quit_id = muda::MenuId::new("quit");
    let quit_item = MenuItem::with_id(quit_id.clone(), "退出", true, None);
    menu.append_items(&[
        &config_item,
        &snippet_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .unwrap();

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
                    "open_config" => {
                        // Windows 暂不支持 ConfigWindow，显示提示
                        let _ = notify_rust::Notification::new()
                            .summary("配置")
                            .body("请在 ~/.config/copy-time-tray/config.yaml 中手动配置")
                            .show();
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
