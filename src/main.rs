use chrono::Local;
use md5::{Digest, Md5};
use rand::Rng;

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
    use gtk::{Menu, MenuItem, SeparatorMenuItem};
    use libappindicator::{AppIndicator, AppIndicatorStatus};
    use notify_rust::Notification;

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

    pub fn main() {
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

    pub fn main() {
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

fn main() {
    platform::main();
}
