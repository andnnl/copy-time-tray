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
use crate::snippet::ui::search::SearchPopup;
use crate::snippet::ui::UiResponse;
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
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW, TranslateMessage, MSG,
    MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT, WM_QUIT,
};

#[cfg(test)]
mod desktop_tests;

pub(crate) fn worker_native_options(viewport: egui::ViewportBuilder) -> eframe::NativeOptions {
    use winit::platform::windows::EventLoopBuilderExtWindows;

    eframe::NativeOptions {
        viewport,
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    }
}

pub(crate) fn desktop_native_options() -> eframe::NativeOptions {
    let mut options = worker_native_options(
        egui::ViewportBuilder::default()
            .with_title("Copy Time Tray")
            .with_inner_size([1.0, 1.0])
            .with_min_inner_size([1.0, 1.0])
            .with_position([-10000.0, -10000.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_transparent(true)
            .with_mouse_passthrough(true),
    );
    options.persist_window = false;
    options
}

fn snippet_viewport() -> egui::ViewportBuilder {
    let state = crate::snippet::config::SnippetWindowState::load().unwrap_or_default();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("代码片段")
        .with_inner_size([state.width as f32, state.height as f32])
        .with_min_inner_size([600.0, 400.0])
        .with_resizable(true)
        .with_maximize_button(true)
        .with_maximized(state.maximized);
    if let (Some(x), Some(y)) = (state.x, state.y) {
        viewport = viewport.with_position([x as f32, y as f32]);
    }
    viewport
}

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

pub(crate) type ConfigApplyRequest = (Config, crossbeam_channel::Sender<Result<(), String>>);

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

pub(crate) struct WindowsUiApp {
    search: SearchPopup,
    search_tx: crossbeam_channel::Sender<UiCommand>,
    cmd_rx: crossbeam_channel::Receiver<UiCommand>,
    config_apply_tx: crossbeam_channel::Sender<ConfigApplyRequest>,
    running: Arc<AtomicBool>,
    json: Option<JsonFormatterApp>,
    settings: Option<HotkeySettingsApp>,
    focus_json: bool,
    focus_settings: bool,
    show_snippets: bool,
    snippet_viewport: egui::ViewportBuilder,
    pending_refresh: Option<Vec<crate::snippet::model::Snippet>>,
}

impl WindowsUiApp {
    pub(crate) fn new(
        cmd_rx: crossbeam_channel::Receiver<UiCommand>,
        resp_tx: crossbeam_channel::Sender<UiResponse>,
        running: Arc<AtomicBool>,
        config_apply_tx: crossbeam_channel::Sender<ConfigApplyRequest>,
    ) -> Self {
        let (search_tx, search_rx) = crossbeam_channel::unbounded();
        Self {
            search: SearchPopup::new(search_rx, resp_tx),
            search_tx,
            cmd_rx,
            config_apply_tx,
            running,
            json: None,
            settings: None,
            focus_json: false,
            focus_settings: false,
            show_snippets: false,
            snippet_viewport: snippet_viewport(),
            pending_refresh: None,
        }
    }

    fn open_settings(&mut self) -> Result<(), String> {
        if self.settings.is_none() {
            let config = Config::load().unwrap_or_default();
            let snippet_current = config
                .general
                .snippet_hotkey
                .parse::<Shortcut>()
                .map_err(|error| format!("代码片段快捷键配置无效: {error}"))?;
            let json_current = config
                .general
                .json_hotkey
                .parse::<Shortcut>()
                .map_err(|error| format!("JSON 快捷键配置无效: {error}"))?;
            self.settings = Some(HotkeySettingsApp::new(
                config,
                snippet_current,
                json_current,
                self.config_apply_tx.clone(),
            ));
        }
        self.focus_settings = true;
        Ok(())
    }

    fn process_commands(&mut self) {
        while let Ok(command) = self.cmd_rx.try_recv() {
            match command {
                UiCommand::Show => {
                    if !self.show_snippets {
                        self.snippet_viewport = snippet_viewport();
                    }
                    self.show_snippets = true;
                    let _ = self.search_tx.send(UiCommand::Show);
                }
                UiCommand::RefreshSnippets(snippets) => {
                    self.pending_refresh = Some(snippets);
                }
                UiCommand::ShowJson => {
                    self.json.get_or_insert_with(JsonFormatterApp::default);
                    self.focus_json = true;
                }
                UiCommand::ShowConfig => {
                    if let Err(error) = self.open_settings() {
                        eprintln!("[ERROR] {error}");
                        let _ = Notification::new()
                            .summary("无法打开配置")
                            .body(&error)
                            .show();
                    }
                }
                command => {
                    let _ = self.search_tx.send(command);
                }
            }
        }
    }
}

impl eframe::App for WindowsUiApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.running.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.process_commands();
        ctx.request_repaint_after(Duration::from_millis(100));
        if self.show_snippets {
            if let Some(snippets) = self.pending_refresh.take() {
                let _ = self.search_tx.send(UiCommand::RefreshSnippets(snippets));
            }
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("snippets"),
                self.snippet_viewport.clone(),
                |ctx, _class| self.search.update(ctx, frame),
            );
            self.show_snippets = self.search.is_visible();
        }

        if let Some(json) = self.json.as_mut() {
            let focus = std::mem::take(&mut self.focus_json);
            let mut close = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("json_formatter"),
                egui::ViewportBuilder::default()
                    .with_title("JSON 格式化工具")
                    .with_inner_size([1000.0, 700.0])
                    .with_min_inner_size([600.0, json_formatter_min_window_height()]),
                |ctx, _class| {
                    if ctx.input(|input| input.viewport().close_requested()) {
                        close = true;
                    } else {
                        if focus {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                        json.update(ctx, frame);
                    }
                },
            );
            if close {
                self.json = None;
            }
        }

        if let Some(settings) = self.settings.as_mut() {
            let focus = std::mem::take(&mut self.focus_settings);
            let mut close = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("hotkey_settings"),
                egui::ViewportBuilder::default()
                    .with_title("Copy Time Tray - 配置")
                    .with_inner_size([500.0, 460.0])
                    .with_resizable(false),
                |ctx, _class| {
                    if ctx.input(|input| input.viewport().close_requested()) {
                        close = true;
                    } else {
                        if focus {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                        settings.update(ctx, frame);
                    }
                },
            );
            if close {
                self.settings = None;
            }
        }
    }
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
    config_apply_rx: crossbeam_channel::Receiver<ConfigApplyRequest>,
    gui_available: Arc<AtomicBool>,
) {
    let menu = Menu::new();

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

    let tray_icon: TrayIcon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip("Copy Time")
        .build()
        .unwrap();

    let menu_channel = MenuEvent::receiver();
    let tray_channel = tray_icon::TrayIconEvent::receiver();
    let mut gui_enabled = true;
    while running.load(Ordering::SeqCst) {
        if !dispatch_pending_messages() {
            running.store(false, Ordering::SeqCst);
            break;
        }
        if gui_enabled && !gui_available.load(Ordering::Acquire) {
            disable_gui_menu_items(&[&snippet_item, &json_formatter_item, &config_item]);
            let _ = tray_icon.set_tooltip(Some("Copy Time - 基础托盘模式（图形界面不可用）"));
            gui_enabled = false;
        }
        while let Ok(event) = menu_channel.try_recv() {
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
                "open_config" if gui_enabled => {
                    let _ = cmd_tx.send(UiCommand::ShowConfig);
                }
                "search_snippets" if gui_enabled => {
                    send_refresh_to_search();
                    let _ = cmd_tx.send(UiCommand::Show);
                }
                "open_json_formatter" if gui_enabled => {
                    let _ = cmd_tx.send(UiCommand::ShowJson);
                }
                "quit" => {
                    running.store(false, Ordering::SeqCst);
                }
                _ => {}
            }
        }
        while tray_channel.try_recv().is_ok() {}
        while let Ok((config, response_tx)) = config_apply_rx.try_recv() {
            let _ = response_tx.send(apply_config_with_hotkey(config));
        }
        if running.load(Ordering::SeqCst) {
            unsafe {
                MsgWaitForMultipleObjectsEx(
                    0,
                    std::ptr::null(),
                    50,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            }
        }
    }
}

fn disable_gui_menu_items(items: &[&MenuItem]) {
    for item in items {
        item.set_enabled(false);
        item.set_text(format!("{}（图形界面不可用）", item.text()));
    }
}

fn dispatch_pending_messages() -> bool {
    let mut message: MSG = unsafe { std::mem::zeroed() };
    while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
        if message.message == WM_QUIT {
            return false;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_thread_dispatches_native_messages_without_gui() {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, KillTimer, SetTimer};

        static TIMER_FIRED: AtomicBool = AtomicBool::new(false);
        unsafe extern "system" fn timer_callback(
            window: HWND,
            _message: u32,
            timer: usize,
            _time: u32,
        ) {
            KillTimer(window, timer);
            TIMER_FIRED.store(true, Ordering::SeqCst);
        }

        let (cmd_tx, _cmd_rx) = crossbeam_channel::unbounded();
        let (_config_tx, config_rx) = crossbeam_channel::unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let running_for_tray = running.clone();
        let tray_thread = std::thread::spawn(move || {
            main(
                cmd_tx,
                running_for_tray,
                config_rx,
                Arc::new(AtomicBool::new(false)),
            )
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let window = loop {
            let window = unsafe { FindWindowW(windows_sys::w!("tray_icon_app"), std::ptr::null()) };
            if !window.is_null() || std::time::Instant::now() >= deadline {
                break window;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        if !window.is_null() {
            unsafe { SetTimer(window, 1, 10, Some(timer_callback)) };
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !TIMER_FIRED.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        running.store(false, Ordering::SeqCst);
        tray_thread.join().unwrap();

        assert!(!window.is_null(), "tray callback window should exist");
        assert!(
            TIMER_FIRED.load(Ordering::SeqCst),
            "tray thread must dispatch queued native messages"
        );
    }

    #[test]
    fn basic_tray_mode_only_disables_gui_menu_items() {
        let menu = Menu::new();
        let snippet = MenuItem::new("代码片段", true, None);
        let json = MenuItem::new("JSON", true, None);
        let config = MenuItem::new("配置", true, None);
        let time = MenuItem::new("复制时间", true, None);
        let password = MenuItem::new("密码", true, None);
        let md5 = MenuItem::new("MD5", true, None);
        let quit = MenuItem::new("退出", true, None);
        for item in [&snippet, &json, &config, &time, &password, &md5, &quit] {
            menu.append(item).unwrap();
        }

        disable_gui_menu_items(&[&snippet, &json, &config]);

        for item in [&snippet, &json, &config] {
            assert!(!item.is_enabled());
            assert!(item.text().contains("图形界面不可用"));
        }
        for item in [&time, &password, &md5, &quit] {
            assert!(item.is_enabled());
        }
    }

    #[test]
    fn repeated_json_commands_reuse_the_existing_window_state() {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (resp_tx, _resp_rx) = crossbeam_channel::unbounded();
        let (config_tx, _config_rx) = crossbeam_channel::unbounded();
        let mut desktop =
            WindowsUiApp::new(cmd_rx, resp_tx, Arc::new(AtomicBool::new(true)), config_tx);
        cmd_tx.send(UiCommand::ShowJson).unwrap();
        desktop.process_commands();
        desktop.json.as_mut().unwrap().input = "keep my input".to_string();
        for _ in 0..100 {
            cmd_tx.send(UiCommand::ShowJson).unwrap();
        }
        desktop.process_commands();

        assert_eq!(desktop.json.as_ref().unwrap().input, "keep my input");
        assert!(desktop.cmd_rx.is_empty());
        assert!(desktop.focus_json);
    }

    #[test]
    fn hidden_snippets_keep_only_the_latest_refresh() {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (resp_tx, _resp_rx) = crossbeam_channel::unbounded();
        let (config_tx, _config_rx) = crossbeam_channel::unbounded();
        let mut desktop =
            WindowsUiApp::new(cmd_rx, resp_tx, Arc::new(AtomicBool::new(true)), config_tx);
        for revision in 0..100 {
            cmd_tx
                .send(UiCommand::RefreshSnippets(vec![
                    crate::snippet::model::Snippet::new_snippet(
                        format!("revision {revision}"),
                        "content".to_string(),
                        None,
                    ),
                ]))
                .unwrap();
            desktop.process_commands();
        }

        assert!(!desktop.show_snippets);
        let snippets = desktop.pending_refresh.as_ref().unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "revision 99");
        assert!(desktop.cmd_rx.is_empty());
    }

    #[test]
    fn worker_native_options_allow_event_loop_on_background_thread() {
        std::thread::spawn(|| {
            let mut options = worker_native_options(egui::ViewportBuilder::default());
            let mut builder = winit::event_loop::EventLoop::with_user_event();
            if let Some(hook) = options.event_loop_builder.take() {
                hook(&mut builder);
            }
            let _event_loop = builder
                .build()
                .expect("worker event loop should initialize");
        })
        .join()
        .expect("worker event loop should not panic");
    }
}
