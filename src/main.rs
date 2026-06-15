
// ===== Linux 实现（GTK + libappindicator）=====
#[cfg(target_os = "linux")]
mod platform {
    use chrono::Local;
    use gtk::prelude::*;
    use gtk::{Menu, MenuItem, SeparatorMenuItem};
    use libappindicator::{AppIndicator, AppIndicatorStatus};
    use notify_rust::Notification;

    fn copy_time(format: &str) {
        let now = Local::now();
        let text = match format {
            "date" => now.format("%Y-%m-%d").to_string(),
            "compact" => now.format("%Y%m%d%H%M%S").to_string(),
            _ => now.format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        clipboard.set_text(&text);
        clipboard.store();
        let _ = Notification::new()
            .summary("Copy Time")
            .body(&format!("已复制: {}", text))
            .show();
    }

    pub fn main() {
        gtk::init().expect("Failed to initialize GTK");

        let icon_path = format!("{}/icons/copy-time.svg", env!("CARGO_MANIFEST_DIR"));
        let mut indicator = AppIndicator::new("copy-time", &icon_path);
        indicator.set_status(AppIndicatorStatus::Active);
        indicator.set_title("Copy Time");

        let mut menu = Menu::new();

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
        menu.show_all();
        indicator.set_menu(&mut menu);

        gtk::main();
    }
}

// ===== Windows 实现（tray-icon + muda）=====
#[cfg(target_os = "windows")]
mod platform {
    use arboard::Clipboard;
    use chrono::Local;
    use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use notify_rust::Notification;
    use tray_icon::{TrayIcon, TrayIconBuilder};

    fn copy_time(format: &str) {
        let now = Local::now();
        let text = match format {
            "date" => now.format("%Y-%m-%d").to_string(),
            "compact" => now.format("%Y%m%d%H%M%S").to_string(),
            _ => now.format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&text);
        }
        let _ = Notification::new()
            .summary("Copy Time")
            .body(&format!("已复制: {}", text))
            .show();
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
        let full_id = muda::MenuId::new("full");
        let date_id = muda::MenuId::new("date");
        let compact_id = muda::MenuId::new("compact");
        let full_item = MenuItem::with_id(full_id.clone(), "复制完整时间", true, None);
        let date_item = MenuItem::with_id(date_id.clone(), "复制日期", true, None);
        let compact_item = MenuItem::with_id(compact_id.clone(), "复制纯数字", true, None);
        menu.append_items(&[&full_item, &date_item, &compact_item, &PredefinedMenuItem::separator()]).unwrap();

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
