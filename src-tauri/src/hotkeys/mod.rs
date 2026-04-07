use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

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

pub fn parse_shortcut(shortcut: &str) -> Result<Shortcut, String> {
    shortcut
        .parse::<Shortcut>()
        .map_err(|err| format!("invalid hotkey '{shortcut}': {err}"))
}

pub fn replace_registration(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    replace_registration_with_fallbacks(app, shortcut, &[]).map(|_| ())
}

pub fn replace_registration_with_fallbacks(
    app: &AppHandle,
    primary_shortcut: &str,
    fallback_shortcuts: &[String],
) -> Result<HotkeyRegistration, String> {
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|err| format!("failed to clear old hotkeys: {err}"))?;

    let mut candidates = Vec::with_capacity(fallback_shortcuts.len() + 1);
    candidates.push(primary_shortcut.to_string());
    for candidate in fallback_shortcuts {
        if !candidates.iter().any(|existing| existing == candidate) {
            candidates.push(candidate.clone());
        }
    }

    let mut errors = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let parsed = match parse_shortcut(candidate) {
            Ok(parsed) => parsed,
            Err(err) => {
                errors.push(err);
                continue;
            }
        };

        match manager.register(parsed) {
            Ok(()) => {
                return Ok(HotkeyRegistration {
                    selected_hotkey: candidate.clone(),
                    mode: if index == 0 {
                        RegistrationMode::Primary
                    } else {
                        RegistrationMode::Fallback
                    },
                });
            }
            Err(err) => {
                errors.push(format!("failed to register hotkey '{candidate}': {err}"));
            }
        }
    }

    Err(format!(
        "failed to register any global hotkey candidate: {}",
        errors.join(" | ")
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_shortcut;

    #[test]
    fn parses_default_rewinder_shortcut() {
        assert!(parse_shortcut("Ctrl+Option+R").is_ok());
    }
}
