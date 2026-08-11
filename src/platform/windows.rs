use crate::app::{apply_config_with_hotkey, send_refresh_to_search, UiCommand};
use crate::hotkey::{
    capture_shortcut, CaptureResult, HotkeyError, HotkeySettingsState, HotkeySettingsStatus,
    Shortcut,
};
use crate::json_formatter::{
    format_json, json_editor_height, json_formatter_min_window_height, line_numbers, JsonFormat,
};
use crate::menu_order::{windows_menu_order, MenuEntry};
use crate::snippet::config::Config;
use crate::utilities::{generate_md5, generate_password, get_time_text};
use arboard::Clipboard;
use eframe::egui;
use global_hotkey::hotkey::{Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tray_icon::{TrayIcon, TrayIconBuilder};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LWIN, VK_RWIN};

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

struct JsonFormatterApp {
    input: String,
    output: String,
    format: JsonFormat,
    error: Option<String>,
    status: Option<String>,
}

impl Default for JsonFormatterApp {
    fn default() -> Self {
        Self {
            input: String::new(),
            output: String::new(),
            format: JsonFormat::PrettyFourSpaces,
            error: None,
            status: None,
        }
    }
}

impl JsonFormatterApp {
    fn paste(&mut self) {
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                self.input = text;
                self.error = None;
                self.status = Some("已从剪贴板载入输入".to_string());
            } else {
                self.error = Some("无法读取剪贴板文本".to_string());
                self.status = None;
            }
        } else {
            self.error = Some("无法打开剪贴板".to_string());
            self.status = None;
        }
    }

    fn format(&mut self) {
        match format_json(&self.input, self.format) {
            Ok(output) => {
                self.output = output;
                self.error = None;
                self.status = Some("格式化成功".to_string());
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.status = None;
            }
        }
    }

    fn copy(&mut self) {
        if self.output.is_empty() {
            return;
        }
        if let Ok(mut clipboard) = Clipboard::new() {
            if clipboard.set_text(&self.output).is_ok() {
                self.error = None;
                self.status = Some("已复制格式化结果".to_string());
            } else {
                self.error = Some("无法设置剪贴板文本".to_string());
                self.status = None;
            }
        } else {
            self.error = Some("无法打开剪贴板".to_string());
            self.status = None;
        }
    }

    fn clear(&mut self) {
        self.input.clear();
        self.output.clear();
        self.error = None;
        self.status = Some("已清除输入和结果".to_string());
    }
}

fn numbered_text_editor(ui: &mut egui::Ui, text: &mut String, interactive: bool, height: f32) {
    let numbers = line_numbers(text);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(numbers).monospace().weak())
                        .selectable(false),
                );
                ui.separator();
                ui.add_sized(
                    [ui.available_width(), height],
                    egui::TextEdit::multiline(text)
                        .font(egui::TextStyle::Monospace)
                        .interactive(interactive)
                        .desired_width(f32::INFINITY),
                );
            });
        });
}

static JSON_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);

struct JsonWindowGuard;

impl Drop for JsonWindowGuard {
    fn drop(&mut self) {
        JSON_WINDOW_OPEN.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn launch_json_formatter() {
    if JSON_WINDOW_OPEN
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(|| {
        let _guard = JsonWindowGuard;
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1000.0, 700.0])
                .with_min_inner_size([600.0, json_formatter_min_window_height()]),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "JSON 格式化工具",
            native_options,
            Box::new(|_cc| Ok(Box::new(JsonFormatterApp::default()))),
        );
    });
}

impl eframe::App for JsonFormatterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut format_idx = JsonFormat::ALL
                .iter()
                .position(|&f| f == self.format)
                .unwrap_or(0);
            egui::ComboBox::from_label("格式")
                .selected_text(self.format.label())
                .show_ui(ui, |ui| {
                    for (i, format) in JsonFormat::ALL.iter().enumerate() {
                        ui.selectable_value(&mut format_idx, i, format.label());
                    }
                });
            self.format = JsonFormat::ALL[format_idx];

            let editor_height = json_editor_height(ui.available_height());

            ui.columns(2, |columns| {
                columns[0].heading("输入");
                numbered_text_editor(&mut columns[0], &mut self.input, true, editor_height);

                columns[1].heading("输出");
                numbered_text_editor(&mut columns[1], &mut self.output, false, editor_height);
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("从剪贴板粘贴").clicked() {
                    self.paste();
                }
                if ui.button("格式化").clicked() {
                    self.format();
                }
                let copy_enabled = !self.output.is_empty();
                ui.add_enabled_ui(copy_enabled, |ui| {
                    if ui.button("复制结果").clicked() {
                        self.copy();
                    }
                });
                if ui.button("清除").clicked() {
                    self.clear();
                }
                if ui.button("关闭").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            if let Some(ref error) = self.error {
                ui.colored_label(egui::Color32::RED, error);
            } else if let Some(ref status) = self.status {
                ui.label(status);
            }
        });
    }
}

type ConfigApplyRequest = (Config, crossbeam_channel::Sender<Result<(), String>>);

static CONFIG_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);

struct ConfigWindowGuard;

impl Drop for ConfigWindowGuard {
    fn drop(&mut self) {
        CONFIG_WINDOW_OPEN.store(false, Ordering::SeqCst);
    }
}

struct HotkeySettingsApp {
    config: Config,
    snippet_state: HotkeySettingsState,
    json_state: HotkeySettingsState,
    apply_tx: crossbeam_channel::Sender<ConfigApplyRequest>,
}

impl HotkeySettingsApp {
    fn new(
        config: Config,
        snippet_current: Shortcut,
        json_current: Shortcut,
        apply_tx: crossbeam_channel::Sender<ConfigApplyRequest>,
    ) -> Self {
        Self {
            config,
            snippet_state: HotkeySettingsState::new(snippet_current),
            json_state: HotkeySettingsState::new(json_current),
            apply_tx,
        }
    }

    fn process_capture_events(&mut self, ctx: &egui::Context) {
        if !self.snippet_state.is_capturing() && !self.json_state.is_capturing() {
            return;
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            let result = capture_from_egui(modifiers, key);
            if self.snippet_state.is_capturing() {
                self.snippet_state.handle_capture(result);
            } else {
                self.json_state.handle_capture(result);
            }
            break;
        }
    }

    fn apply(&mut self) -> bool {
        let mut candidate_config = self.config.clone();
        candidate_config.general.snippet_hotkey = self.snippet_state.candidate().to_string();
        candidate_config.general.json_hotkey = self.json_state.candidate().to_string();
        if candidate_config.general.snippet_hotkey == candidate_config.general.json_hotkey {
            self.json_state
                .mark_failure("代码片段和 JSON 对话框不能使用相同快捷键");
            return false;
        }
        let (response_tx, response_rx) = crossbeam_channel::bounded(1);
        if self.apply_tx.send((candidate_config, response_tx)).is_err() {
            self.snippet_state.mark_failure("无法连接配置应用服务");
            return false;
        }
        match response_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => {
                self.snippet_state.mark_success();
                self.json_state.mark_success();
                true
            }
            Ok(Err(error)) => {
                self.snippet_state.mark_failure(error);
                false
            }
            Err(_) => {
                self.snippet_state.mark_failure("配置应用超时，请重试");
                false
            }
        }
    }
}

impl eframe::App for HotkeySettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_capture_events(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("全局快捷键");
            ui.label("F1-F12 可单独使用；其他按键必须包含 Ctrl、Alt 或 Win。");
            ui.add_space(12.0);

            shortcut_setting_ui(ui, "代码片段", &mut self.snippet_state);
            ui.separator();
            shortcut_setting_ui(ui, "JSON 对话框", &mut self.json_state);

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("保存").clicked() && self.apply() {
                    let _ = Notification::new()
                        .summary("配置已保存")
                        .body("代码片段和 JSON 对话框快捷键已更新")
                        .show();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("取消").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

fn shortcut_setting_ui(ui: &mut egui::Ui, label: &str, state: &mut HotkeySettingsState) {
    ui.strong(label);
    ui.horizontal(|ui| {
        ui.label("当前绑定：");
        ui.strong(state.current().to_string());
    });
    ui.horizontal(|ui| {
        ui.label("候选绑定：");
        ui.strong(state.candidate().to_string());
    });
    let capture_label = if state.is_capturing() {
        "请按下快捷键（Esc 取消）"
    } else {
        "按下新快捷键"
    };
    if ui.button(capture_label).clicked() {
        state.begin_capture();
    }
    if let Some(status) = state.status() {
        match status {
            HotkeySettingsStatus::Success => {
                ui.colored_label(egui::Color32::DARK_GREEN, "绑定成功");
            }
            HotkeySettingsStatus::Error(message) => {
                ui.colored_label(egui::Color32::RED, message);
            }
        }
    }
}

fn launch_hotkey_settings(apply_tx: crossbeam_channel::Sender<ConfigApplyRequest>) {
    if CONFIG_WINDOW_OPEN
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        let _guard = ConfigWindowGuard;
        let config = Config::load().unwrap_or_default();
        let snippet_current = match config.general.snippet_hotkey.parse::<Shortcut>() {
            Ok(shortcut) => shortcut,
            Err(error) => {
                let _ = Notification::new()
                    .summary("代码片段快捷键配置无效")
                    .body(&error.to_string())
                    .show();
                return;
            }
        };
        let json_current = match config.general.json_hotkey.parse::<Shortcut>() {
            Ok(shortcut) => shortcut,
            Err(error) => {
                let _ = Notification::new()
                    .summary("JSON 快捷键配置无效")
                    .body(&error.to_string())
                    .show();
                return;
            }
        };
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([500.0, 460.0])
                .with_resizable(false),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "Copy Time Tray - 配置",
            native_options,
            Box::new(move |_cc| {
                Ok(Box::new(HotkeySettingsApp::new(
                    config,
                    snippet_current,
                    json_current,
                    apply_tx,
                )))
            }),
        );
    });
}

fn capture_from_egui(
    modifiers: egui::Modifiers,
    key: egui::Key,
) -> Result<CaptureResult, HotkeyError> {
    capture_shortcut(egui_modifiers(modifiers), egui_key_code(key)?)
}

fn egui_modifiers(modifiers: egui::Modifiers) -> Option<Modifiers> {
    let mut result = Modifiers::empty();
    if modifiers.ctrl {
        result.insert(Modifiers::CONTROL);
    }
    if modifiers.alt {
        result.insert(Modifiers::ALT);
    }
    if modifiers.shift {
        result.insert(Modifiers::SHIFT);
    }
    if win_key_pressed() {
        result.insert(Modifiers::SUPER);
    }
    (!result.is_empty()).then_some(result)
}

fn win_key_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_LWIN as i32) < 0 || GetAsyncKeyState(VK_RWIN as i32) < 0 }
}

fn egui_key_code(key: egui::Key) -> Result<Option<Code>, HotkeyError> {
    let code = match key {
        egui::Key::Escape => Code::Escape,
        egui::Key::Tab => Code::Tab,
        egui::Key::Backspace => Code::Backspace,
        egui::Key::Enter => Code::Enter,
        egui::Key::Space => Code::Space,
        egui::Key::Insert => Code::Insert,
        egui::Key::Delete => Code::Delete,
        egui::Key::Home => Code::Home,
        egui::Key::End => Code::End,
        egui::Key::PageUp => Code::PageUp,
        egui::Key::PageDown => Code::PageDown,
        egui::Key::ArrowUp => Code::ArrowUp,
        egui::Key::ArrowDown => Code::ArrowDown,
        egui::Key::ArrowLeft => Code::ArrowLeft,
        egui::Key::ArrowRight => Code::ArrowRight,
        egui::Key::Num0 => Code::Digit0,
        egui::Key::Num1 => Code::Digit1,
        egui::Key::Num2 => Code::Digit2,
        egui::Key::Num3 => Code::Digit3,
        egui::Key::Num4 => Code::Digit4,
        egui::Key::Num5 => Code::Digit5,
        egui::Key::Num6 => Code::Digit6,
        egui::Key::Num7 => Code::Digit7,
        egui::Key::Num8 => Code::Digit8,
        egui::Key::Num9 => Code::Digit9,
        egui::Key::A => Code::KeyA,
        egui::Key::B => Code::KeyB,
        egui::Key::C => Code::KeyC,
        egui::Key::D => Code::KeyD,
        egui::Key::E => Code::KeyE,
        egui::Key::F => Code::KeyF,
        egui::Key::G => Code::KeyG,
        egui::Key::H => Code::KeyH,
        egui::Key::I => Code::KeyI,
        egui::Key::J => Code::KeyJ,
        egui::Key::K => Code::KeyK,
        egui::Key::L => Code::KeyL,
        egui::Key::M => Code::KeyM,
        egui::Key::N => Code::KeyN,
        egui::Key::O => Code::KeyO,
        egui::Key::P => Code::KeyP,
        egui::Key::Q => Code::KeyQ,
        egui::Key::R => Code::KeyR,
        egui::Key::S => Code::KeyS,
        egui::Key::T => Code::KeyT,
        egui::Key::U => Code::KeyU,
        egui::Key::V => Code::KeyV,
        egui::Key::W => Code::KeyW,
        egui::Key::X => Code::KeyX,
        egui::Key::Y => Code::KeyY,
        egui::Key::Z => Code::KeyZ,
        egui::Key::F1 => Code::F1,
        egui::Key::F2 => Code::F2,
        egui::Key::F3 => Code::F3,
        egui::Key::F4 => Code::F4,
        egui::Key::F5 => Code::F5,
        egui::Key::F6 => Code::F6,
        egui::Key::F7 => Code::F7,
        egui::Key::F8 => Code::F8,
        egui::Key::F9 => Code::F9,
        egui::Key::F10 => Code::F10,
        egui::Key::F11 => Code::F11,
        egui::Key::F12 => Code::F12,
        unsupported => {
            return Err(HotkeyError::InvalidSyntax(format!(
                "暂不支持按键 {unsupported:?}"
            )))
        }
    };
    Ok(Some(code))
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
    let (config_apply_tx, config_apply_rx) = crossbeam_channel::unbounded::<ConfigApplyRequest>();

    // 复制时间菜单项
    let full_id = muda::MenuId::new("full");
    let date_id = muda::MenuId::new("date");
    let compact_id = muda::MenuId::new("compact");
    let full_item = MenuItem::with_id(full_id.clone(), "复制完整时间", true, None);
    let date_item = MenuItem::with_id(date_id.clone(), "复制日期", true, None);
    let compact_item = MenuItem::with_id(compact_id.clone(), "复制纯数字", true, None);
    let json_formatter_id = muda::MenuId::new("open_json_formatter");
    let json_formatter_item = MenuItem::with_id(json_formatter_id.clone(), "JSON", true, None);

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

    // MD5 子菜单
    let md5_submenu = Submenu::new("MD5", true);
    let md5_16_id = muda::MenuId::new("md5_16");
    let md5_32_id = muda::MenuId::new("md5_32");
    let md5_16_item = MenuItem::with_id(md5_16_id.clone(), "16位MD5", true, None);
    let md5_32_item = MenuItem::with_id(md5_32_id.clone(), "32位MD5", true, None);
    md5_submenu
        .append_items(&[&md5_16_item, &md5_32_item])
        .unwrap();

    let config_id = muda::MenuId::new("open_config");
    let config_item = MenuItem::with_id(config_id.clone(), "配置", true, None);
    let snippet_id = muda::MenuId::new("search_snippets");
    let snippet_item = MenuItem::with_id(snippet_id.clone(), "代码片段", true, None);
    let quit_id = muda::MenuId::new("quit");
    let quit_item = MenuItem::with_id(quit_id.clone(), "退出", true, None);
    for entry in windows_menu_order() {
        match entry {
            MenuEntry::Snippets => menu.append(&snippet_item).unwrap(),
            MenuEntry::FullTime => menu.append(&full_item).unwrap(),
            MenuEntry::Date => menu.append(&date_item).unwrap(),
            MenuEntry::CompactTime => menu.append(&compact_item).unwrap(),
            MenuEntry::Separator => menu.append(&PredefinedMenuItem::separator()).unwrap(),
            MenuEntry::Json => menu.append(&json_formatter_item).unwrap(),
            MenuEntry::Password => menu.append(&password_submenu).unwrap(),
            MenuEntry::Md5 => menu.append(&md5_submenu).unwrap(),
            MenuEntry::Config => menu.append(&config_item).unwrap(),
            MenuEntry::Sync => {}
            MenuEntry::Quit => menu.append(&quit_item).unwrap(),
        }
    }

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
    let config_apply_tx_for_thread = config_apply_tx.clone();
    let menu_channel = MenuEvent::receiver();
    std::thread::spawn(move || loop {
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
                    launch_hotkey_settings(config_apply_tx_for_thread.clone());
                }
                "search_snippets" => {
                    send_refresh_to_search();
                    let _ = cmd_tx.send(UiCommand::Show);
                }
                "open_json_formatter" => launch_json_formatter(),
                "quit" => {
                    running_for_thread.store(false, std::sync::atomic::Ordering::SeqCst);
                }
                _ => {}
            }
        }
    });

    // Windows 保持进程存活，直到收到退出信号
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        while let Ok((config, response_tx)) = config_apply_rx.try_recv() {
            let _ = response_tx.send(apply_config_with_hotkey(config));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
