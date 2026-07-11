use crate::app::MANAGER;
use crate::json_formatter::{format_json, JsonFormat};
use crate::snippet::ui::UiCommand;
use crate::utilities::{generate_md5, generate_password, get_time_text};
use gtk::prelude::*;
use gtk::{
    glib, Button, ComboBoxText, Dialog, Label, Menu, MenuItem, Orientation, Paned, ScrolledWindow,
    SeparatorMenuItem, TextView, WindowPosition,
};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};

static JSON_DIALOG_OPEN: AtomicBool = AtomicBool::new(false);

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

fn show_json_formatter() {
    if JSON_DIALOG_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }

    let dialog = Dialog::new();
    dialog.set_title("JSON 格式化");
    dialog.set_default_size(900, 600);
    dialog.set_position(WindowPosition::Center);
    dialog.set_resizable(true);

    dialog.connect_destroy(|_| {
        JSON_DIALOG_OPEN.store(false, Ordering::SeqCst);
    });

    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let format_selector = ComboBoxText::new();
    for format in JsonFormat::ALL {
        format_selector.append_text(format.label());
    }
    format_selector.set_active(Some(0));
    content.pack_start(&format_selector, false, false, 0);

    let input_view = TextView::new();
    let input_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    input_view.set_buffer(Some(&input_buffer));
    input_view.set_wrap_mode(gtk::WrapMode::WordChar);
    let output_view = TextView::new();
    let output_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    output_view.set_buffer(Some(&output_buffer));
    output_view.set_editable(false);
    output_view.set_cursor_visible(false);
    output_view.set_wrap_mode(gtk::WrapMode::WordChar);

    let input_column = gtk::Box::new(Orientation::Vertical, 6);
    input_column.pack_start(&Label::new(Some("输入 JSON")), false, false, 0);
    let input_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    input_scroll.add(&input_view);
    input_column.pack_start(&input_scroll, true, true, 0);

    let output_column = gtk::Box::new(Orientation::Vertical, 6);
    output_column.pack_start(&Label::new(Some("格式化结果")), false, false, 0);
    let output_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    output_scroll.add(&output_view);
    output_column.pack_start(&output_scroll, true, true, 0);

    let columns = Paned::new(Orientation::Horizontal);
    columns.pack1(&input_column, true, false);
    columns.pack2(&output_column, true, false);
    content.pack_start(&columns, true, true, 0);

    let status_label = Label::new(None);
    status_label.set_xalign(0.0);
    status_label.set_line_wrap(true);
    content.pack_start(&status_label, false, false, 0);

    let button_row = gtk::Box::new(Orientation::Horizontal, 6);
    let paste_button = Button::with_label("从剪贴板粘贴");
    let format_button = Button::with_label("格式化");
    let copy_button = Button::with_label("复制结果");
    copy_button.set_sensitive(false);
    let close_button = Button::with_label("关闭");
    button_row.pack_start(&paste_button, false, false, 0);
    button_row.pack_start(&format_button, false, false, 0);
    button_row.pack_start(&copy_button, false, false, 0);
    button_row.pack_end(&close_button, false, false, 0);
    content.pack_start(&button_row, false, false, 0);

    paste_button.connect_clicked({
        let input_buffer = input_buffer.clone();
        let status_label = status_label.clone();
        move |_| {
            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
            let input_buffer = input_buffer.clone();
            let status_label = status_label.clone();
            clipboard.request_text(move |_, text| match text {
                Some(text) => {
                    input_buffer.set_text(text);
                    status_label.set_text("已从剪贴板载入输入");
                }
                None => status_label.set_text("剪贴板中没有可读取的文本"),
            });
        }
    });

    format_button.connect_clicked(glib::clone!(
        @strong format_selector,
        @strong input_buffer,
        @strong output_buffer,
        @strong status_label,
        @strong copy_button => move |_| {
            let input = input_buffer
                .text(&input_buffer.start_iter(), &input_buffer.end_iter(), true)
                .unwrap_or_default();
            let format = format_selector
                .active()
                .and_then(|index| JsonFormat::ALL.get(index as usize).copied())
                .unwrap_or_default();

            match format_json(&input, format) {
                Ok(output) => {
                    output_buffer.set_text(&output);
                    copy_button.set_sensitive(true);
                    status_label.set_text("格式化成功");
                }
                Err(error) => status_label.set_text(&error.to_string()),
            }
        }
    ));

    copy_button.connect_clicked({
        let output_buffer = output_buffer.clone();
        let status_label = status_label.clone();
        move |_| {
            let output = output_buffer
                .text(&output_buffer.start_iter(), &output_buffer.end_iter(), true)
                .unwrap_or_default();
            if output.is_empty() {
                status_label.set_text("没有可复制的格式化结果");
                return;
            }

            let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
            clipboard.set_text(&output);
            clipboard.store();
            status_label.set_text("已复制格式化结果");
        }
    });

    close_button.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });

    dialog.show_all();
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

    let json_item = MenuItem::with_label("JSON");
    json_item.connect_activate(|_| show_json_formatter());
    menu.append(&json_item);

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
