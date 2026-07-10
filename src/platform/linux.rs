use crate::app::MANAGER;
use crate::snippet::ui::UiCommand;
use crate::utilities::{generate_md5, generate_password, get_time_text};
use gtk::prelude::*;
use gtk::{glib, Menu, MenuItem, SeparatorMenuItem};
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

pub fn main(
    _cmd_tx: crossbeam_channel::Sender<UiCommand>,
    cmd_rx: crossbeam_channel::Receiver<UiCommand>,
) {
    gtk::init().expect("Failed to initialize GTK");
    gtk::glib::idle_add_local(move || {
        while let Ok(command) = cmd_rx.try_recv() {
            match command {
                UiCommand::Show => crate::snippet::ui::linux_manager::show(),
                UiCommand::Sync => {
                    std::thread::spawn(|| {
                        if let Some(mut manager) =
                            MANAGER.get().and_then(|manager| manager.lock().ok())
                        {
                            let result =
                                manager.do_pull_with_retry().and_then(|_| manager.do_push());
                            let (summary, body) = match result {
                                Ok(()) => ("同步完成", "snippet 已同步到 Gitee".to_string()),
                                Err(error) => ("同步失败", error),
                            };
                            let _ = notify_rust::Notification::new()
                                .summary(summary)
                                .body(&body)
                                .show();
                        }
                    });
                }
                _ => {}
            }
        }
        gtk::glib::ControlFlow::Continue
    });

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

    let config_item = MenuItem::with_label("配置");
    config_item.connect_activate(move |_| {
        glib::idle_add(move || {
            if let Some(new_config) = crate::snippet::ui::config_window::ConfigWindow::show(None) {
                if let Err(e) = new_config.save() {
                    let _ = notify_rust::Notification::new()
                        .summary("配置保存失败")
                        .body(&e.to_string())
                        .show();
                } else {
                    if let Some(mut m) = MANAGER.get().and_then(|m| m.lock().ok()) {
                        let _ = m.reload_config();
                        m.schedule_push();
                    }
                    let _ = notify_rust::Notification::new()
                        .summary("配置已保存")
                        .body("配置已更新")
                        .show();
                }
            }
            glib::ControlFlow::Break
        });
    });
    menu.append(&config_item);

    let snippet_item = MenuItem::with_label("代码片断");
    let cmd_tx_clone = _cmd_tx.clone();
    snippet_item.connect_activate(move |_| {
        let _ = cmd_tx_clone.send(UiCommand::Show);
    });
    menu.append(&snippet_item);

    let sync_item = MenuItem::with_label("同步");
    let cmd_tx_for_sync = _cmd_tx.clone();
    sync_item.connect_activate(move |_| {
        eprintln!("[SYNC] Sync menu clicked");
        let _ = cmd_tx_for_sync.send(UiCommand::Sync);
    });
    menu.append(&sync_item);

    let quit_item = MenuItem::with_label("退出");
    quit_item.connect_activate(|_| gtk::main_quit());
    menu.append(&quit_item);

    menu.show_all();
    indicator.set_menu(&mut menu);

    gtk::main();
}
