use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyManager, HotKeyState};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyError {
    InvalidSyntax(String),
    MissingPrimaryModifier,
    ManagerInitialization(String),
    Registration(String),
    Unregistration(String),
    Rollback(String),
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSyntax(message) => write!(f, "快捷键格式无效：{message}"),
            Self::MissingPrimaryModifier => {
                write!(f, "除 F1-F12 外，快捷键必须包含 Ctrl、Alt 或 Super/Win")
            }
            Self::ManagerInitialization(message) => write!(f, "快捷键服务初始化失败：{message}"),
            Self::Registration(message) => write!(f, "快捷键绑定失败：{message}"),
            Self::Unregistration(message) => write!(f, "旧快捷键解绑失败：{message}"),
            Self::Rollback(message) => write!(f, "快捷键回滚失败：{message}"),
        }
    }
}

impl std::error::Error for HotkeyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shortcut {
    modifiers: Modifiers,
    code: Code,
}

impl Shortcut {
    pub fn new(modifiers: Option<Modifiers>, code: Code) -> Result<Self, HotkeyError> {
        let modifiers = normalize_modifiers(modifiers.unwrap_or_else(Modifiers::empty));
        let is_function_key = matches!(
            code,
            Code::F1
                | Code::F2
                | Code::F3
                | Code::F4
                | Code::F5
                | Code::F6
                | Code::F7
                | Code::F8
                | Code::F9
                | Code::F10
                | Code::F11
                | Code::F12
        );
        let has_primary =
            modifiers.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER);
        if !is_function_key && !has_primary {
            return Err(HotkeyError::MissingPrimaryModifier);
        }

        Ok(Self { modifiers, code })
    }

    pub fn modifiers(self) -> Option<Modifiers> {
        (!self.modifiers.is_empty()).then_some(self.modifiers)
    }

    pub fn to_hotkey(self) -> HotKey {
        HotKey::new(self.modifiers(), self.code)
    }
}

impl FromStr for Shortcut {
    type Err = HotkeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let hotkey = value
            .parse::<HotKey>()
            .map_err(|error| HotkeyError::InvalidSyntax(error.to_string()))?;
        Self::new(Some(hotkey.mods), hotkey.key)
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.contains(Modifiers::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.contains(Modifiers::ALT) {
            parts.push("Alt".to_string());
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            parts.push("Shift".to_string());
        }
        if self.modifiers.contains(Modifiers::SUPER) {
            parts.push("Super".to_string());
        }
        parts.push(code_label(self.code));
        write!(f, "{}", parts.join("+"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureResult {
    Updated(Shortcut),
    Cancelled,
    Ignored,
}

pub fn capture_shortcut(
    modifiers: Option<Modifiers>,
    code: Option<Code>,
) -> Result<CaptureResult, HotkeyError> {
    let Some(code) = code else {
        return Ok(CaptureResult::Ignored);
    };
    if code == Code::Escape {
        return Ok(CaptureResult::Cancelled);
    }
    Shortcut::new(modifiers, code).map(CaptureResult::Updated)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HotkeySettingsStatus {
    Success,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct HotkeySettingsState {
    current: Shortcut,
    candidate: Shortcut,
    capturing: bool,
    status: Option<HotkeySettingsStatus>,
}

impl HotkeySettingsState {
    pub fn new(current: Shortcut) -> Self {
        Self {
            current,
            candidate: current,
            capturing: false,
            status: None,
        }
    }

    pub fn current(&self) -> Shortcut {
        self.current
    }

    pub fn candidate(&self) -> Shortcut {
        self.candidate
    }

    pub fn is_capturing(&self) -> bool {
        self.capturing
    }

    pub fn status(&self) -> Option<&HotkeySettingsStatus> {
        self.status.as_ref()
    }

    pub fn begin_capture(&mut self) {
        self.capturing = true;
        self.status = None;
    }

    pub fn handle_capture(&mut self, result: Result<CaptureResult, HotkeyError>) {
        match result {
            Ok(CaptureResult::Updated(shortcut)) => {
                self.candidate = shortcut;
                self.capturing = false;
                self.status = None;
            }
            Ok(CaptureResult::Cancelled) => {
                self.capturing = false;
                self.status = None;
            }
            Ok(CaptureResult::Ignored) => {}
            Err(error) => {
                self.status = Some(HotkeySettingsStatus::Error(error.to_string()));
            }
        }
    }

    pub fn mark_success(&mut self) {
        self.current = self.candidate;
        self.capturing = false;
        self.status = Some(HotkeySettingsStatus::Success);
    }

    pub fn mark_failure(&mut self, message: impl Into<String>) {
        self.capturing = false;
        self.status = Some(HotkeySettingsStatus::Error(message.into()));
    }
}

fn normalize_modifiers(mut modifiers: Modifiers) -> Modifiers {
    if modifiers.contains(Modifiers::META) {
        modifiers.remove(Modifiers::META);
        modifiers.insert(Modifiers::SUPER);
    }
    modifiers & (Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::SUPER)
}

fn code_label(code: Code) -> String {
    let raw = code.to_string();
    raw.strip_prefix("Key").unwrap_or(&raw).to_string()
}

pub(crate) trait HotkeyBackend {
    fn register(&mut self, hotkey: HotKey) -> Result<(), String>;
    fn unregister(&mut self, hotkey: HotKey) -> Result<(), String>;
}

impl HotkeyBackend for GlobalHotKeyManager {
    fn register(&mut self, hotkey: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::register(self, hotkey).map_err(|error| error.to_string())
    }

    fn unregister(&mut self, hotkey: HotKey) -> Result<(), String> {
        GlobalHotKeyManager::unregister(self, hotkey).map_err(|error| error.to_string())
    }
}

pub struct HotkeyService<B: HotkeyBackend = GlobalHotKeyManager> {
    backend: B,
    current: Shortcut,
}

impl HotkeyService<GlobalHotKeyManager> {
    pub fn new(initial: Shortcut) -> Result<Self, HotkeyError> {
        let backend = GlobalHotKeyManager::new()
            .map_err(|error| HotkeyError::ManagerInitialization(error.to_string()))?;
        Self::with_backend(backend, initial)
    }
}

impl<B: HotkeyBackend> HotkeyService<B> {
    fn with_backend(mut backend: B, initial: Shortcut) -> Result<Self, HotkeyError> {
        backend
            .register(initial.to_hotkey())
            .map_err(HotkeyError::Registration)?;
        Ok(Self {
            backend,
            current: initial,
        })
    }

    pub fn current(&self) -> Shortcut {
        self.current
    }

    pub fn current_id(&self) -> u32 {
        self.current.to_hotkey().id()
    }

    pub fn rebind(&mut self, candidate: Shortcut) -> Result<(), HotkeyError> {
        if candidate == self.current {
            return Ok(());
        }

        let old_hotkey = self.current.to_hotkey();
        let new_hotkey = candidate.to_hotkey();
        self.backend
            .register(new_hotkey)
            .map_err(HotkeyError::Registration)?;

        if let Err(error) = self.backend.unregister(old_hotkey) {
            if let Err(rollback_error) = self.backend.unregister(new_hotkey) {
                return Err(HotkeyError::Rollback(format!(
                    "旧快捷键解绑失败（{error}），且新快捷键撤销失败（{rollback_error}）"
                )));
            }
            return Err(HotkeyError::Unregistration(error));
        }

        self.current = candidate;
        Ok(())
    }
}

pub fn should_open_snippets(current_id: u32, event_id: u32, state: HotKeyState) -> bool {
    current_id == event_id && state == HotKeyState::Pressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::hotkey::{Code, Modifiers};
    use global_hotkey::HotKeyState;

    #[derive(Default)]
    struct FakeBackend {
        registered: Vec<HotKey>,
        calls: Vec<String>,
        fail_register_id: Option<u32>,
        fail_unregister_id: Option<u32>,
    }

    impl HotkeyBackend for FakeBackend {
        fn register(&mut self, hotkey: HotKey) -> Result<(), String> {
            self.calls.push(format!("register:{}", hotkey.id()));
            if self.fail_register_id == Some(hotkey.id()) {
                return Err("register rejected".to_string());
            }
            self.registered.push(hotkey);
            Ok(())
        }

        fn unregister(&mut self, hotkey: HotKey) -> Result<(), String> {
            self.calls.push(format!("unregister:{}", hotkey.id()));
            if self.fail_unregister_id == Some(hotkey.id()) {
                return Err("unregister rejected".to_string());
            }
            self.registered.retain(|item| item.id() != hotkey.id());
            Ok(())
        }
    }

    fn shortcut(modifiers: Modifiers, code: Code) -> Shortcut {
        Shortcut::new(Some(modifiers), code).unwrap()
    }

    #[test]
    fn function_keys_are_valid_without_modifiers() {
        for key in [Code::F1, Code::F6, Code::F12] {
            assert!(Shortcut::new(None, key).is_ok());
        }
    }

    #[test]
    fn ordinary_keys_require_a_primary_modifier() {
        assert!(Shortcut::new(None, Code::KeyA).is_err());
        assert!(Shortcut::new(Some(Modifiers::SHIFT), Code::KeyA).is_err());
        assert!(Shortcut::new(Some(Modifiers::CONTROL), Code::KeyA).is_ok());
        assert!(Shortcut::new(Some(Modifiers::ALT), Code::Digit1).is_ok());
        assert!(Shortcut::new(Some(Modifiers::SUPER), Code::KeyS).is_ok());
    }

    #[test]
    fn shortcut_display_round_trips() {
        let shortcut =
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyS).unwrap();

        let text = shortcut.to_string();

        assert_eq!(text, "Ctrl+Shift+S");
        assert_eq!(text.parse::<Shortcut>().unwrap(), shortcut);
    }

    #[test]
    fn stored_default_parses_and_is_canonical() {
        let shortcut = "Ctrl+Shift+S".parse::<Shortcut>().unwrap();

        assert_eq!(shortcut.to_string(), "Ctrl+Shift+S");
    }

    #[test]
    fn invalid_or_weak_shortcut_strings_are_rejected() {
        assert!("not-a-shortcut".parse::<Shortcut>().is_err());
        assert!("Shift+A".parse::<Shortcut>().is_err());
        assert!("A".parse::<Shortcut>().is_err());
    }

    #[test]
    fn capture_normalizes_modifiers_and_updates_supported_keys() {
        let captured =
            capture_shortcut(Some(Modifiers::CONTROL | Modifiers::META), Some(Code::KeyA)).unwrap();

        assert_eq!(
            captured,
            CaptureResult::Updated(shortcut(Modifiers::CONTROL | Modifiers::SUPER, Code::KeyA,))
        );
        assert_eq!(
            capture_shortcut(Some(Modifiers::ALT), Some(Code::Digit7)).unwrap(),
            CaptureResult::Updated(shortcut(Modifiers::ALT, Code::Digit7))
        );
        assert_eq!(
            capture_shortcut(None, Some(Code::F8)).unwrap(),
            CaptureResult::Updated(shortcut(Modifiers::empty(), Code::F8))
        );
    }

    #[test]
    fn capture_escape_cancels_without_replacing_candidate() {
        assert_eq!(
            capture_shortcut(Some(Modifiers::CONTROL), Some(Code::Escape)).unwrap(),
            CaptureResult::Cancelled
        );
    }

    #[test]
    fn capture_modifier_only_event_is_ignored() {
        assert_eq!(
            capture_shortcut(Some(Modifiers::CONTROL | Modifiers::SHIFT), None).unwrap(),
            CaptureResult::Ignored
        );
    }

    #[test]
    fn capture_rejects_weak_shortcuts() {
        assert_eq!(
            capture_shortcut(Some(Modifiers::SHIFT), Some(Code::KeyA)),
            Err(HotkeyError::MissingPrimaryModifier)
        );
    }

    #[test]
    fn hotkey_settings_capture_updates_candidate_and_escape_cancels() {
        let current = shortcut(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyS);
        let replacement = shortcut(Modifiers::ALT, Code::F8);
        let mut state = HotkeySettingsState::new(current);

        state.begin_capture();
        state.handle_capture(Ok(CaptureResult::Updated(replacement)));

        assert_eq!(state.candidate(), replacement);
        assert!(!state.is_capturing());

        state.begin_capture();
        state.handle_capture(Ok(CaptureResult::Cancelled));

        assert_eq!(state.candidate(), replacement);
        assert!(!state.is_capturing());
    }

    #[test]
    fn hotkey_settings_success_commits_candidate() {
        let current = shortcut(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyS);
        let replacement = shortcut(Modifiers::ALT, Code::F8);
        let mut state = HotkeySettingsState::new(current);

        state.begin_capture();
        state.handle_capture(Ok(CaptureResult::Updated(replacement)));
        state.mark_success();

        assert_eq!(state.current(), replacement);
        assert_eq!(state.status(), Some(&HotkeySettingsStatus::Success));
    }

    #[test]
    fn hotkey_settings_failure_preserves_current_and_candidate() {
        let current = shortcut(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyS);
        let replacement = shortcut(Modifiers::ALT, Code::F8);
        let mut state = HotkeySettingsState::new(current);

        state.begin_capture();
        state.handle_capture(Ok(CaptureResult::Updated(replacement)));
        state.mark_failure("快捷键已被占用");

        assert_eq!(state.current(), current);
        assert_eq!(state.candidate(), replacement);
        assert_eq!(
            state.status(),
            Some(&HotkeySettingsStatus::Error("快捷键已被占用".to_string()))
        );
    }

    #[test]
    fn hotkey_settings_capture_error_keeps_previous_candidate() {
        let current = shortcut(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyS);
        let mut state = HotkeySettingsState::new(current);

        state.begin_capture();
        state.handle_capture(Err(HotkeyError::MissingPrimaryModifier));

        assert_eq!(state.candidate(), current);
        assert!(state.is_capturing());
        assert!(matches!(
            state.status(),
            Some(HotkeySettingsStatus::Error(_))
        ));
    }

    #[test]
    fn successful_rebind_replaces_current_shortcut() {
        let original = shortcut(Modifiers::CONTROL, Code::KeyS);
        let replacement = shortcut(Modifiers::ALT, Code::F8);
        let mut service = HotkeyService::with_backend(FakeBackend::default(), original).unwrap();

        service.rebind(replacement).unwrap();

        assert_eq!(service.current(), replacement);
        assert_eq!(service.backend.registered, vec![replacement.to_hotkey()]);
        assert_eq!(
            service.backend.calls,
            vec![
                format!("register:{}", original.to_hotkey().id()),
                format!("register:{}", replacement.to_hotkey().id()),
                format!("unregister:{}", original.to_hotkey().id()),
            ]
        );
    }

    #[test]
    fn failed_new_registration_keeps_original_shortcut() {
        let original = shortcut(Modifiers::CONTROL, Code::KeyS);
        let replacement = shortcut(Modifiers::ALT, Code::F8);
        let backend = FakeBackend {
            fail_register_id: Some(replacement.to_hotkey().id()),
            ..FakeBackend::default()
        };
        let mut service = HotkeyService::with_backend(backend, original).unwrap();

        assert!(service.rebind(replacement).is_err());

        assert_eq!(service.current(), original);
        assert_eq!(service.backend.registered, vec![original.to_hotkey()]);
    }

    #[test]
    fn failed_old_unregistration_rolls_back_new_registration() {
        let original = shortcut(Modifiers::CONTROL, Code::KeyS);
        let replacement = shortcut(Modifiers::ALT, Code::F8);
        let backend = FakeBackend {
            fail_unregister_id: Some(original.to_hotkey().id()),
            ..FakeBackend::default()
        };
        let mut service = HotkeyService::with_backend(backend, original).unwrap();

        assert!(service.rebind(replacement).is_err());

        assert_eq!(service.current(), original);
        assert_eq!(service.backend.registered, vec![original.to_hotkey()]);
        assert_eq!(
            service.backend.calls.last(),
            Some(&format!("unregister:{}", replacement.to_hotkey().id()))
        );
    }

    #[test]
    fn only_matching_pressed_event_opens_snippets() {
        assert!(should_open_snippets(7, 7, HotKeyState::Pressed));
        assert!(!should_open_snippets(7, 8, HotKeyState::Pressed));
        assert!(!should_open_snippets(7, 7, HotKeyState::Released));
    }
}
