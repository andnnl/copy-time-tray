use crate::app::MANAGER;
use crate::json_formatter::{format_json, line_numbers, JsonFormat};
use crate::menu_order::{linux_menu_order, MenuEntry};
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
use std::time::Duration;

static JSON_DIALOG_OPEN: AtomicBool = AtomicBool::new(false);

fn reset_json_dialog_open() {
    JSON_DIALOG_OPEN.store(false, Ordering::SeqCst);
}

fn ui_command_poll_interval() -> Duration {
    Duration::from_millis(50)
}

fn json_formatter_is_resizable() -> bool {
    true
}

fn json_formatter_window_type_hint() -> gtk::gdk::WindowTypeHint {
    gtk::gdk::WindowTypeHint::Normal
}

#[cfg(test)]
mod tests {
    use super::{
        json_formatter_is_resizable, json_formatter_window_type_hint, reset_json_dialog_open,
        sync_success_notification_body, ui_command_poll_interval, JSON_DIALOG_OPEN,
    };
    use std::sync::atomic::Ordering;

    #[test]
    fn destroying_the_json_dialog_releases_the_single_window_guard() {
        JSON_DIALOG_OPEN.store(true, Ordering::SeqCst);

        reset_json_dialog_open();

        assert!(!JSON_DIALOG_OPEN.load(Ordering::SeqCst));
    }

    #[test]
    fn ui_command_poll_interval_is_non_zero() {
        assert!(!ui_command_poll_interval().is_zero());
    }

    #[test]
    fn json_formatter_window_is_resizable() {
        assert!(json_formatter_is_resizable());
    }

    #[test]
    fn json_formatter_uses_normal_window_type_for_maximize_support() {
        assert_eq!(
            json_formatter_window_type_hint(),
            gtk::gdk::WindowTypeHint::Normal
        );
    }

    #[test]
    fn sync_success_notification_is_remote_neutral() {
        assert_eq!(sync_success_notification_body(), "snippet 已同步到远程");
    }
}

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
    dialog.set_type_hint(json_formatter_window_type_hint());
    dialog.set_default_size(900, 600);
    dialog.set_position(WindowPosition::Center);
    dialog.set_resizable(json_formatter_is_resizable());

    dialog.connect_delete_event(|_, _| {
        reset_json_dialog_open();
        glib::Propagation::Proceed
    });
    dialog.connect_destroy(|_| {
        reset_json_dialog_open();
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
    input_view.set_monospace(true);
    input_view.set_wrap_mode(gtk::WrapMode::None);
    input_view.set_hexpand(true);
    let output_view = TextView::new();
    let output_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    output_view.set_buffer(Some(&output_buffer));
    output_view.set_monospace(true);
    output_view.set_editable(false);
    output_view.set_cursor_visible(false);
    output_view.set_wrap_mode(gtk::WrapMode::None);
    output_view.set_hexpand(true);

    let input_lines = TextView::new();
    let input_lines_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    input_lines.set_buffer(Some(&input_lines_buffer));
    input_lines_buffer.set_text("1");
    input_lines.set_monospace(true);
    input_lines.set_editable(false);
    input_lines.set_cursor_visible(false);
    input_lines.set_wrap_mode(gtk::WrapMode::None);
    input_lines.set_sensitive(false);

    let output_lines = TextView::new();
    let output_lines_buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    output_lines.set_buffer(Some(&output_lines_buffer));
    output_lines_buffer.set_text("1");
    output_lines.set_monospace(true);
    output_lines.set_editable(false);
    output_lines.set_cursor_visible(false);
    output_lines.set_wrap_mode(gtk::WrapMode::None);
    output_lines.set_sensitive(false);

    input_buffer.connect_changed({
        let input_lines_buffer = input_lines_buffer.clone();
        move |buffer| {
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .unwrap_or_default();
            input_lines_buffer.set_text(&line_numbers(&text));
        }
    });
    output_buffer.connect_changed({
        let output_lines_buffer = output_lines_buffer.clone();
        move |buffer| {
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .unwrap_or_default();
            output_lines_buffer.set_text(&line_numbers(&text));
        }
    });

    let input_column = gtk::Box::new(Orientation::Vertical, 6);
    input_column.pack_start(&Label::new(Some("输入 JSON")), false, false, 0);
    let input_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    let input_editor = gtk::Box::new(Orientation::Horizontal, 8);
    input_editor.pack_start(&input_lines, false, false, 4);
    input_editor.pack_start(&input_view, true, true, 0);
    input_scroll.add(&input_editor);
    input_column.pack_start(&input_scroll, true, true, 0);

    let output_column = gtk::Box::new(Orientation::Vertical, 6);
    output_column.pack_start(&Label::new(Some("格式化结果")), false, false, 0);
    let output_scroll = ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    let output_editor = gtk::Box::new(Orientation::Horizontal, 8);
    output_editor.pack_start(&output_lines, false, false, 4);
    output_editor.pack_start(&output_view, true, true, 0);
    output_scroll.add(&output_editor);
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
    let clear_button = Button::with_label("清除");
    let close_button = Button::with_label("关闭");
    button_row.pack_start(&paste_button, false, false, 0);
    button_row.pack_start(&format_button, false, false, 0);
    button_row.pack_start(&copy_button, false, false, 0);
    button_row.pack_start(&clear_button, false, false, 0);
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

    clear_button.connect_clicked(glib::clone!(
        @strong input_buffer,
        @strong output_buffer,
        @strong status_label,
        @strong copy_button => move |_| {
            input_buffer.set_text("");
            output_buffer.set_text("");
            copy_button.set_sensitive(false);
            status_label.set_text("已清除输入和结果");
        }
    ));

    close_button.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });

    dialog.show_all();
}

fn sync_success_notification_body() -> &'static str {
    "snippet 已同步到远程"
}

pub fn main(
    _cmd_tx: crossbeam_channel::Sender<UiCommand>,
    cmd_rx: crossbeam_channel::Receiver<UiCommand>,
) {
    gtk::init().expect("Failed to initialize GTK");
    gtk::glib::timeout_add_local(ui_command_poll_interval(), move || {
        while let Ok(command) = cmd_rx.try_recv() {
            match command {
                UiCommand::Show => crate::snippet::ui::linux_manager::show(),
                UiCommand::ShowJson => show_json_formatter(),
                UiCommand::Sync => {
                    std::thread::spawn(|| {
                        if let Some(mut manager) =
                            MANAGER.get().and_then(|manager| manager.lock().ok())
                        {
                            let result =
                                manager.do_pull_with_retry().and_then(|_| manager.do_push());
                            let (summary, body) = match result {
                                Ok(()) => {
                                    ("同步完成", sync_success_notification_body().to_string())
                                }
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

    let date_item = MenuItem::with_label("复制日期");
    date_item.connect_activate(move |_| copy_time("date"));

    let compact_item = MenuItem::with_label("复制纯数字");
    compact_item.connect_activate(move |_| copy_time("compact"));

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

    let json_item = MenuItem::with_label("JSON");
    json_item.connect_activate(|_| show_json_formatter());

    let config_item = MenuItem::with_label("配置");
    config_item.connect_activate(move |_| {
        glib::idle_add(move || {
            if crate::snippet::ui::config_window::ConfigWindow::show(
                None,
                crate::app::apply_config_with_hotkey,
            ) {
                let _ = notify_rust::Notification::new()
                    .summary("配置已保存")
                    .body("快捷键绑定成功，配置已更新")
                    .show();
            }
            glib::ControlFlow::Break
        });
    });
    let snippet_item = MenuItem::with_label("代码片段");
    let cmd_tx_clone = _cmd_tx.clone();
    snippet_item.connect_activate(move |_| {
        let _ = cmd_tx_clone.send(UiCommand::Show);
    });

    let sync_item = MenuItem::with_label("同步");
    let cmd_tx_for_sync = _cmd_tx.clone();
    sync_item.connect_activate(move |_| {
        eprintln!("[SYNC] Sync menu clicked");
        let _ = cmd_tx_for_sync.send(UiCommand::Sync);
    });

    let quit_item = MenuItem::with_label("退出");
    quit_item.connect_activate(|_| gtk::main_quit());
    for entry in linux_menu_order() {
        match entry {
            MenuEntry::Snippets => menu.append(&snippet_item),
            MenuEntry::FullTime => menu.append(&full_item),
            MenuEntry::Date => menu.append(&date_item),
            MenuEntry::CompactTime => menu.append(&compact_item),
            MenuEntry::Separator => menu.append(&SeparatorMenuItem::new()),
            MenuEntry::Json => menu.append(&json_item),
            MenuEntry::Password => menu.append(&password_item),
            MenuEntry::Md5 => menu.append(&md5_item),
            MenuEntry::Config => menu.append(&config_item),
            MenuEntry::Sync => menu.append(&sync_item),
            MenuEntry::Quit => menu.append(&quit_item),
        }
    }

    menu.show_all();
    indicator.set_menu(&mut menu);

    gtk::main();
}
