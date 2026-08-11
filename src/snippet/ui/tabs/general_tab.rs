use super::ConfigTab;
use crate::hotkey::{capture_shortcut, CaptureResult, HotkeyError, Shortcut};
use crate::snippet::config::Config;
use global_hotkey::hotkey::{Code, Modifiers};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct GeneralTab {
    pub auto_sync_switch: gtk::Switch,
    pub sync_interval_entry: gtk::Entry,
    pub notify_success_switch: gtk::Switch,
    pub notify_failure_switch: gtk::Switch,
    pub notify_create_switch: gtk::Switch,
    hotkey_button: gtk::Button,
    hotkey_status: gtk::Label,
    hotkey_candidate: Rc<RefCell<Shortcut>>,
    capture_active: Rc<Cell<bool>>,
    json_hotkey_button: gtk::Button,
    json_hotkey_status: gtk::Label,
    json_hotkey_candidate: Rc<RefCell<Shortcut>>,
    json_capture_active: Rc<Cell<bool>>,
}

impl GeneralTab {
    pub fn new() -> Self {
        let auto_sync_switch = gtk::Switch::new();
        auto_sync_switch.set_active(true);

        let sync_interval_entry = gtk::Entry::new();
        sync_interval_entry.set_text("300");
        sync_interval_entry.set_width_chars(8);

        let notify_success_switch = gtk::Switch::new();
        notify_success_switch.set_active(true);

        let notify_failure_switch = gtk::Switch::new();
        notify_failure_switch.set_active(true);

        let notify_create_switch = gtk::Switch::new();
        notify_create_switch.set_active(true);

        let initial_hotkey = Config::load()
            .unwrap_or_default()
            .general
            .snippet_hotkey
            .parse::<Shortcut>()
            .unwrap_or_else(|_| "Ctrl+Shift+S".parse().expect("default shortcut must parse"));
        let hotkey_button = gtk::Button::with_label(&initial_hotkey.to_string());
        hotkey_button.set_can_focus(true);
        let hotkey_status = gtk::Label::new(Some("点击后按下新快捷键；Esc 取消"));
        hotkey_status.set_xalign(0.0);
        let hotkey_candidate = Rc::new(RefCell::new(initial_hotkey));
        let capture_active = Rc::new(Cell::new(false));

        hotkey_button.connect_clicked({
            let capture_active = capture_active.clone();
            move |button| {
                capture_active.set(true);
                button.set_label("请按下快捷键…");
                button.grab_focus();
            }
        });
        hotkey_button.connect_key_press_event({
            let capture_active = capture_active.clone();
            let hotkey_candidate = hotkey_candidate.clone();
            let hotkey_status = hotkey_status.clone();
            move |button, event| {
                if !capture_active.get() {
                    return gtk::glib::Propagation::Proceed;
                }
                let key_name = event
                    .keyval()
                    .name()
                    .map(|name| name.to_string())
                    .unwrap_or_default();
                match capture_from_gdk(&key_name, event.state()) {
                    Ok(CaptureResult::Updated(shortcut)) => {
                        *hotkey_candidate.borrow_mut() = shortcut;
                        capture_active.set(false);
                        button.set_label(&shortcut.to_string());
                        set_status(&hotkey_status, "#2e7d32", "快捷键待保存");
                    }
                    Ok(CaptureResult::Cancelled) => {
                        capture_active.set(false);
                        button.set_label(&hotkey_candidate.borrow().to_string());
                        set_status(&hotkey_status, "#666666", "已取消快捷键捕获");
                    }
                    Ok(CaptureResult::Ignored) => {}
                    Err(error) => set_status(&hotkey_status, "#c62828", &error.to_string()),
                }
                gtk::glib::Propagation::Stop
            }
        });

        let initial_json_hotkey = Config::load()
            .unwrap_or_default()
            .general
            .json_hotkey
            .parse::<Shortcut>()
            .unwrap_or_else(|_| "Ctrl+Shift+J".parse().expect("default shortcut must parse"));
        let json_hotkey_button = gtk::Button::with_label(&initial_json_hotkey.to_string());
        json_hotkey_button.set_can_focus(true);
        let json_hotkey_status = gtk::Label::new(Some("点击后按下新快捷键；Esc 取消"));
        json_hotkey_status.set_xalign(0.0);
        let json_hotkey_candidate = Rc::new(RefCell::new(initial_json_hotkey));
        let json_capture_active = Rc::new(Cell::new(false));

        json_hotkey_button.connect_clicked({
            let json_capture_active = json_capture_active.clone();
            move |button| {
                json_capture_active.set(true);
                button.set_label("请按下快捷键…");
                button.grab_focus();
            }
        });
        json_hotkey_button.connect_key_press_event({
            let json_capture_active = json_capture_active.clone();
            let json_hotkey_candidate = json_hotkey_candidate.clone();
            let json_hotkey_status = json_hotkey_status.clone();
            move |button, event| {
                if !json_capture_active.get() {
                    return gtk::glib::Propagation::Proceed;
                }
                let key_name = event
                    .keyval()
                    .name()
                    .map(|name| name.to_string())
                    .unwrap_or_default();
                match capture_from_gdk(&key_name, event.state()) {
                    Ok(CaptureResult::Updated(shortcut)) => {
                        *json_hotkey_candidate.borrow_mut() = shortcut;
                        json_capture_active.set(false);
                        button.set_label(&shortcut.to_string());
                        set_status(&json_hotkey_status, "#2e7d32", "快捷键待保存");
                    }
                    Ok(CaptureResult::Cancelled) => {
                        json_capture_active.set(false);
                        button.set_label(&json_hotkey_candidate.borrow().to_string());
                        set_status(&json_hotkey_status, "#666666", "已取消快捷键捕获");
                    }
                    Ok(CaptureResult::Ignored) => {}
                    Err(error) => set_status(&json_hotkey_status, "#c62828", &error.to_string()),
                }
                gtk::glib::Propagation::Stop
            }
        });

        Self {
            auto_sync_switch,
            sync_interval_entry,
            notify_success_switch,
            notify_failure_switch,
            notify_create_switch,
            hotkey_button,
            hotkey_status,
            hotkey_candidate,
            capture_active,
            json_hotkey_button,
            json_hotkey_status,
            json_hotkey_candidate,
            json_capture_active,
        }
    }

    fn create_switch_row(label_text: &str, switch: &gtk::Switch) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let label = gtk::Label::new(Some(label_text));
        label.set_xalign(0.0);
        row.pack_start(&label, true, true, 0);
        row.pack_start(switch, false, false, 0);
        row
    }

    pub fn snippet_hotkey(&self) -> String {
        self.hotkey_candidate.borrow().to_string()
    }

    pub fn json_hotkey(&self) -> String {
        self.json_hotkey_candidate.borrow().to_string()
    }

    pub fn show_success(&self, message: &str) {
        set_status(&self.hotkey_status, "#2e7d32", message);
    }

    pub fn show_error(&self, message: &str) {
        set_status(&self.hotkey_status, "#c62828", message);
    }
}

fn set_status(label: &gtk::Label, color: &str, message: &str) {
    let escaped = gtk::glib::markup_escape_text(message);
    label.set_markup(&format!("<span foreground=\"{color}\">{escaped}</span>"));
}

fn gdk_modifiers(state: gtk::gdk::ModifierType) -> Option<Modifiers> {
    let mut modifiers = Modifiers::empty();
    if state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
        modifiers.insert(Modifiers::CONTROL);
    }
    if state.contains(gtk::gdk::ModifierType::MOD1_MASK) {
        modifiers.insert(Modifiers::ALT);
    }
    if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
        modifiers.insert(Modifiers::SHIFT);
    }
    if state.contains(gtk::gdk::ModifierType::SUPER_MASK)
        || state.contains(gtk::gdk::ModifierType::META_MASK)
    {
        modifiers.insert(Modifiers::SUPER);
    }
    (!modifiers.is_empty()).then_some(modifiers)
}

fn code_from_gdk_name(name: &str) -> Result<Option<Code>, HotkeyError> {
    let normalized = name.to_ascii_uppercase();
    let code = match normalized.as_str() {
        "ESCAPE" => Code::Escape,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "LEFT" => Code::ArrowLeft,
        "RIGHT" => Code::ArrowRight,
        "UP" => Code::ArrowUp,
        "DOWN" => Code::ArrowDown,
        "HOME" => Code::Home,
        "END" => Code::End,
        "PAGE_UP" => Code::PageUp,
        "PAGE_DOWN" => Code::PageDown,
        "SPACE" => Code::Space,
        "RETURN" | "KP_ENTER" => Code::Enter,
        "TAB" => Code::Tab,
        "BACKSPACE" => Code::Backspace,
        "DELETE" => Code::Delete,
        "SHIFT_L" | "SHIFT_R" | "CONTROL_L" | "CONTROL_R" | "ALT_L" | "ALT_R" | "META_L"
        | "META_R" | "SUPER_L" | "SUPER_R" => return Ok(None),
        _ => return Err(HotkeyError::InvalidSyntax(format!("暂不支持按键 {name}"))),
    };
    Ok(Some(code))
}

fn capture_from_gdk(
    key_name: &str,
    state: gtk::gdk::ModifierType,
) -> Result<CaptureResult, HotkeyError> {
    capture_shortcut(gdk_modifiers(state), code_from_gdk_name(key_name)?)
}

impl ConfigTab for GeneralTab {
    fn title(&self) -> &str {
        "通用设置"
    }

    fn build(&self, container: &gtk::Box) {
        // Sync settings
        let sync_frame = gtk::Frame::new(Some("同步设置"));
        let sync_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        sync_box.set_margin_top(8);
        sync_box.set_margin_bottom(8);
        sync_box.set_margin_start(8);
        sync_box.set_margin_end(8);

        sync_box.pack_start(
            &Self::create_switch_row("启动时自动同步", &self.auto_sync_switch),
            false,
            false,
            0,
        );

        let interval_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let interval_label = gtk::Label::new(Some("同步间隔:"));
        interval_label.set_xalign(0.0);
        interval_row.pack_start(&interval_label, true, true, 0);
        interval_row.pack_start(&self.sync_interval_entry, false, false, 0);
        let sec_label = gtk::Label::new(Some("秒"));
        interval_row.pack_start(&sec_label, false, false, 0);
        sync_box.pack_start(&interval_row, false, false, 0);

        sync_frame.add(&sync_box);
        container.pack_start(&sync_frame, false, false, 0);

        // Notification settings
        let notify_frame = gtk::Frame::new(Some("通知设置"));
        let notify_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        notify_box.set_margin_top(8);
        notify_box.set_margin_bottom(8);
        notify_box.set_margin_start(8);
        notify_box.set_margin_end(8);

        notify_box.pack_start(
            &Self::create_switch_row("同步成功时通知", &self.notify_success_switch),
            false,
            false,
            0,
        );
        notify_box.pack_start(
            &Self::create_switch_row("同步失败时通知", &self.notify_failure_switch),
            false,
            false,
            0,
        );
        notify_box.pack_start(
            &Self::create_switch_row("创建 Snippet 时通知", &self.notify_create_switch),
            false,
            false,
            0,
        );

        notify_frame.add(&notify_box);
        container.pack_start(&notify_frame, false, false, 0);

        let hotkey_frame = gtk::Frame::new(Some("代码片段快捷键"));
        let hotkey_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        hotkey_box.set_margin_top(8);
        hotkey_box.set_margin_bottom(8);
        hotkey_box.set_margin_start(8);
        hotkey_box.set_margin_end(8);
        hotkey_box.pack_start(&self.hotkey_button, false, false, 0);
        hotkey_box.pack_start(&self.hotkey_status, false, false, 0);
        hotkey_frame.add(&hotkey_box);
        container.pack_start(&hotkey_frame, false, false, 0);

        let json_hotkey_frame = gtk::Frame::new(Some("JSON 对话框快捷键"));
        let json_hotkey_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        json_hotkey_box.set_margin_top(8);
        json_hotkey_box.set_margin_bottom(8);
        json_hotkey_box.set_margin_start(8);
        json_hotkey_box.set_margin_end(8);
        json_hotkey_box.pack_start(&self.json_hotkey_button, false, false, 0);
        json_hotkey_box.pack_start(&self.json_hotkey_status, false, false, 0);
        json_hotkey_frame.add(&json_hotkey_box);
        container.pack_start(&json_hotkey_frame, false, false, 0);
    }

    fn on_save(&self) -> Result<(), String> {
        let interval_text = self.sync_interval_entry.text().to_string();
        let interval: u64 = interval_text
            .parse()
            .map_err(|_| "同步间隔必须是数字".to_string())?;
        if interval < 10 {
            return Err("同步间隔不能小于 10 秒".to_string());
        }
        self.snippet_hotkey()
            .parse::<Shortcut>()
            .map_err(|error| error.to_string())?;
        let json_hotkey = self
            .json_hotkey()
            .parse::<Shortcut>()
            .map_err(|error| error.to_string())?;
        if self.snippet_hotkey() == json_hotkey.to_string() {
            return Err("代码片段和 JSON 对话框不能使用相同快捷键".to_string());
        }
        Ok(())
    }

    fn on_load(&self) {
        let config = Config::load().unwrap_or_default();
        let general = &config.general;

        self.auto_sync_switch.set_active(general.auto_sync);
        self.sync_interval_entry
            .set_text(&general.sync_interval.to_string());
        self.notify_success_switch
            .set_active(general.notify_on_success);
        self.notify_failure_switch
            .set_active(general.notify_on_failure);
        self.notify_create_switch
            .set_active(general.notify_on_create);
        if let Ok(shortcut) = general.snippet_hotkey.parse::<Shortcut>() {
            *self.hotkey_candidate.borrow_mut() = shortcut;
            self.hotkey_button.set_label(&shortcut.to_string());
        } else {
            self.show_error("当前代码片段快捷键配置无效，请重新设置");
        }
        if let Ok(shortcut) = general.json_hotkey.parse::<Shortcut>() {
            *self.json_hotkey_candidate.borrow_mut() = shortcut;
            self.json_hotkey_button.set_label(&shortcut.to_string());
        } else {
            set_status(
                &self.json_hotkey_status,
                "#c62828",
                "当前 JSON 快捷键配置无效，请重新设置",
            );
        }
        self.capture_active.set(false);
        self.json_capture_active.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{CaptureResult, Shortcut};
    use gtk::gdk::ModifierType;

    #[test]
    fn general_tab_maps_ctrl_shift_s() {
        let result =
            capture_from_gdk("s", ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK).unwrap();

        assert_eq!(
            result,
            CaptureResult::Updated("Ctrl+Shift+S".parse::<Shortcut>().unwrap())
        );
    }

    #[test]
    fn general_tab_maps_standalone_f8() {
        assert_eq!(
            capture_from_gdk("F8", ModifierType::empty()).unwrap(),
            CaptureResult::Updated("F8".parse::<Shortcut>().unwrap())
        );
    }

    #[test]
    fn general_tab_rejects_shift_only_letter() {
        assert!(capture_from_gdk("a", ModifierType::SHIFT_MASK).is_err());
    }

    #[test]
    fn general_tab_escape_cancels_capture() {
        assert_eq!(
            capture_from_gdk("Escape", ModifierType::empty()).unwrap(),
            CaptureResult::Cancelled
        );
    }
}
