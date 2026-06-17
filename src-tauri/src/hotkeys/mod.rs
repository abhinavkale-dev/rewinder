#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationMode {
    Primary,
    Fallback,
}

#[derive(Debug, Clone)]
pub struct HotkeyRegistration {
    pub selected_hotkey: String,
    pub mode: RegistrationMode,
}
