use arboard::Clipboard;
use chrono::Local;
use gtk::{prelude::*, MenuItem, Menu};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use notify_rust::Notification;

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

fn main() {
    gtk::init().expect("Failed to initialize GTK");

    let mut indicator = AppIndicator::new("copy-time", "accessories-clock");
    indicator.set_status(AppIndicatorStatus::Active);
    indicator.set_title("Copy Time");

    let mut menu = Menu::new();

    // 复制完整时间
    let full_item = MenuItem::with_label("复制完整时间");
    full_item.connect_activate(move |_| {
        copy_time("full");
    });
    menu.append(&full_item);

    // 复制日期
    let date_item = MenuItem::with_label("复制日期");
    date_item.connect_activate(move |_| {
        copy_time("date");
    });
    menu.append(&date_item);

    // 复制纯数字
    let compact_item = MenuItem::with_label("复制纯数字");
    compact_item.connect_activate(move |_| {
        copy_time("compact");
    });
    menu.append(&compact_item);

    menu.show_all();
    indicator.set_menu(&mut menu);

    gtk::main();
}
