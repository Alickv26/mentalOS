use crate::error::{MentalOSError, Result};
use directories::BaseDirs;
use gtk4::gdk;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct ShortcutDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub default: &'static str,
}

pub const SHORTCUT_DEFINITIONS: [ShortcutDefinition; 14] = [
    ShortcutDefinition {
        id: "focus_input",
        label: "Focus input",
        description: "Move focus to the main prompt input.",
        default: "Ctrl+L",
    },
    ShortcutDefinition {
        id: "open_launcher",
        label: "Open app launcher",
        description: "Open the installed applications launcher dialog.",
        default: "Ctrl+K",
    },
    ShortcutDefinition {
        id: "open_memory_browser",
        label: "Open memory browser",
        description: "Browse stored conversation memory sessions.",
        default: "Ctrl+Shift+M",
    },
    ShortcutDefinition {
        id: "open_tasks_browser",
        label: "Open tasks browser",
        description: "Open the tracked tasks dialog.",
        default: "Ctrl+Shift+J",
    },
    ShortcutDefinition {
        id: "open_sync_selection",
        label: "Open sync selection",
        description: "Open conversation sync selection dialog.",
        default: "Ctrl+Shift+S",
    },
    ShortcutDefinition {
        id: "new_chat",
        label: "Start new chat",
        description: "Clear the current conversation and start a new chat.",
        default: "Ctrl+N",
    },
    ShortcutDefinition {
        id: "toggle_high_contrast",
        label: "Toggle high contrast mode",
        description: "Toggle high-contrast UI colors for readability.",
        default: "Ctrl+Shift+H",
    },
    ShortcutDefinition {
        id: "font_increase",
        label: "Increase font size",
        description: "Increase UI font size.",
        default: "Ctrl+Plus",
    },
    ShortcutDefinition {
        id: "font_decrease",
        label: "Decrease font size",
        description: "Decrease UI font size.",
        default: "Ctrl+Minus",
    },
    ShortcutDefinition {
        id: "font_reset",
        label: "Reset font size",
        description: "Reset UI font size to normal.",
        default: "Ctrl+0",
    },
    ShortcutDefinition {
        id: "show_help",
        label: "Show shortcuts help",
        description: "Open the keyboard shortcuts help dialog.",
        default: "Ctrl+Slash",
    },
    ShortcutDefinition {
        id: "manage_shortcuts",
        label: "Manage shortcuts",
        description: "Open shortcut configuration settings.",
        default: "Ctrl+Comma",
    },
    ShortcutDefinition {
        id: "show_onboarding",
        label: "Show onboarding tutorial",
        description: "Open the first-run onboarding tutorial dialog.",
        default: "Ctrl+Shift+T",
    },
    ShortcutDefinition {
        id: "emergency_stop",
        label: "Emergency stop",
        description: "Immediately stop active AI operations.",
        default: "Ctrl+Shift+Q",
    },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutBindings {
    #[serde(default)]
    bindings: HashMap<String, String>,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        for def in SHORTCUT_DEFINITIONS {
            bindings.insert(def.id.to_string(), def.default.to_string());
        }
        Self { bindings }
    }
}

impl ShortcutBindings {
    pub fn load_or_default() -> Result<Self> {
        let path = shortcuts_path()?;
        if !path.exists() {
            let defaults = Self::default();
            let _ = defaults.save();
            return Ok(defaults);
        }

        let contents = fs::read_to_string(&path)?;
        let mut loaded: ShortcutBindings = toml::from_str(&contents)?;
        loaded.fill_missing_defaults();
        Ok(loaded)
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = shortcuts_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let serialized = toml::to_string_pretty(self).map_err(|err| {
            MentalOSError::ConfigInvalid(format!("Failed to serialize shortcuts: {err}"))
        })?;
        fs::write(&path, serialized)?;
        Ok(path)
    }

    pub fn get(&self, action_id: &str) -> String {
        self.bindings
            .get(action_id)
            .cloned()
            .unwrap_or_else(|| default_for(action_id).to_string())
    }

    pub fn set(&mut self, action_id: &str, combo: &str) {
        self.bindings
            .insert(action_id.to_string(), combo.trim().to_string());
    }

    pub fn matches(&self, action_id: &str, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        let combo = self.get(action_id);
        matches_shortcut(&combo, key, modifiers)
    }

    fn fill_missing_defaults(&mut self) {
        for def in SHORTCUT_DEFINITIONS {
            self.bindings
                .entry(def.id.to_string())
                .or_insert_with(|| def.default.to_string());
        }
    }
}

pub fn validate_shortcut(combo: &str) -> Result<()> {
    let _ = parse_shortcut(combo)?;
    Ok(())
}

fn default_for(action_id: &str) -> &'static str {
    SHORTCUT_DEFINITIONS
        .iter()
        .find(|d| d.id == action_id)
        .map(|d| d.default)
        .unwrap_or("Ctrl+K")
}

fn shortcuts_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or_else(|| MentalOSError::ConfigInvalid("No home dir".into()))?;
    Ok(base.config_dir().join("mentalOS").join("shortcuts.toml"))
}

#[derive(Debug)]
struct ParsedShortcut {
    ctrl: bool,
    shift: bool,
    alt: bool,
    key_token: String,
}

fn parse_shortcut(combo: &str) -> Result<ParsedShortcut> {
    let mut cleaned: String = combo.chars().filter(|c| !c.is_whitespace()).collect();
    if !cleaned.contains('+') {
        let lower = cleaned.to_ascii_lowercase();
        for prefix in ["ctrl", "control", "shift", "alt", "mod1"] {
            if lower.starts_with(prefix) && cleaned.len() > prefix.len() {
                let insert_at = prefix.len();
                if cleaned.chars().nth(insert_at) != Some('+') {
                    cleaned.insert(insert_at, '+');
                }
                break;
            }
        }
    }
    if cleaned.is_empty() {
        return Err(MentalOSError::ConfigInvalid(
            "Shortcut cannot be empty".to_string(),
        ));
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut key_token: Option<String> = None;

    for part in cleaned.split('+') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let lower = p.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "mod1" => alt = true,
            _ => {
                if key_token.is_some() {
                    return Err(MentalOSError::ConfigInvalid(format!(
                        "Invalid shortcut '{}': multiple key tokens",
                        combo
                    )));
                }
                key_token = Some(lower);
            }
        }
    }

    let token = key_token.ok_or_else(|| {
        MentalOSError::ConfigInvalid(format!("Invalid shortcut '{}': missing key token", combo))
    })?;

    if !is_supported_key_token(&token) {
        return Err(MentalOSError::ConfigInvalid(format!(
            "Unsupported key token '{}' in '{}'",
            token, combo
        )));
    }

    Ok(ParsedShortcut {
        ctrl,
        shift,
        alt,
        key_token: token,
    })
}

fn is_supported_key_token(token: &str) -> bool {
    match token {
        "?" | "question" | "/" | "slash" | "plus" | "+" | "equal" | "=" | "minus" | "-"
        | "comma" | "," | "escape" | "esc" | "tab" | "space" | "kp_divide" => true,
        _ => {
            if token.len() == 1 {
                return token.chars().all(|c| c.is_ascii_alphanumeric());
            }
            if let Some(number) = token.strip_prefix('f') {
                return number
                    .parse::<u8>()
                    .map(|n| (1..=24).contains(&n))
                    .unwrap_or(false);
            }
            false
        }
    }
}

fn matches_shortcut(combo: &str, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    let parsed = match parse_shortcut(combo) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
    let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
    let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);

    if parsed.ctrl != ctrl || parsed.alt != alt {
        return false;
    }
    if parsed.shift && !shift {
        return false;
    }
    if shift && !parsed.shift && !allows_implicit_shift(&parsed.key_token) {
        return false;
    }

    key_matches(&parsed.key_token, key, shift)
}

fn allows_implicit_shift(token: &str) -> bool {
    matches!(
        token,
        "?" | "question" | "/" | "slash" | "plus" | "+" | "equal" | "="
    )
}

fn key_matches(token: &str, key: gdk::Key, shift: bool) -> bool {
    match token {
        "?" | "question" => key == gdk::Key::question || (key == gdk::Key::slash && shift),
        "/" | "slash" => {
            key == gdk::Key::slash || key == gdk::Key::question || key == gdk::Key::KP_Divide
        }
        "plus" | "+" => key == gdk::Key::plus || key == gdk::Key::equal,
        "equal" | "=" => key == gdk::Key::equal || key == gdk::Key::plus,
        "minus" | "-" => key == gdk::Key::minus,
        "comma" | "," => key == gdk::Key::comma,
        "escape" | "esc" => key == gdk::Key::Escape,
        "tab" => key == gdk::Key::Tab,
        "space" => key == gdk::Key::space,
        "kp_divide" => key == gdk::Key::KP_Divide,
        other => key
            .name()
            .map(|name| name.as_str().eq_ignore_ascii_case(other))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvOverride {
        _guard: MutexGuard<'static, ()>,
        old_xdg_config_home: Option<String>,
    }

    impl EnvOverride {
        fn xdg_config_home(path: &std::path::Path) -> Self {
            let guard = ENV_LOCK
                .lock()
                .expect("environment lock should not be poisoned");
            let old_xdg_config_home = std::env::var("XDG_CONFIG_HOME").ok();

            // SAFETY: tests hold ENV_LOCK for the full override lifetime.
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", path);
            }

            Self {
                _guard: guard,
                old_xdg_config_home,
            }
        }
    }

    impl Drop for EnvOverride {
        fn drop(&mut self) {
            // SAFETY: tests hold ENV_LOCK for the full override lifetime.
            unsafe {
                match &self.old_xdg_config_home {
                    Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                    None => std::env::remove_var("XDG_CONFIG_HOME"),
                }
            }
        }
    }

    #[test]
    fn slash_shortcut_matches_question_key() {
        assert!(matches_shortcut(
            "Ctrl+Slash",
            gdk::Key::question,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
        ));
    }

    #[test]
    fn validates_defaults() {
        for def in SHORTCUT_DEFINITIONS {
            validate_shortcut(def.default).expect("default shortcut should be valid");
        }
    }

    #[test]
    fn compact_format_parses_without_plus_separator() {
        validate_shortcut("Ctrl/").expect("Ctrl/ should be accepted");
        validate_shortcut("Ctrl?").expect("Ctrl? should be accepted");
        validate_shortcut("ShiftTab").expect("ShiftTab should be accepted");
    }

    #[test]
    fn rejects_invalid_shortcuts() {
        assert!(validate_shortcut("").is_err());
        assert!(validate_shortcut("Ctrl").is_err());
        assert!(validate_shortcut("Ctrl+Alt+Shift").is_err());
        assert!(validate_shortcut("Ctrl+Shift+FooBar").is_err());
        assert!(validate_shortcut("Ctrl+A+B").is_err());
    }

    #[test]
    fn question_shortcut_matches_slash_with_shift() {
        assert!(matches_shortcut(
            "Ctrl+Question",
            gdk::Key::slash,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
        ));
    }

    #[test]
    fn slash_shortcut_matches_numpad_divide() {
        assert!(matches_shortcut(
            "Ctrl+Slash",
            gdk::Key::KP_Divide,
            gdk::ModifierType::CONTROL_MASK,
        ));
    }

    #[test]
    fn implicit_shift_is_allowed_for_shifted_symbols() {
        assert!(matches_shortcut(
            "Ctrl+Equal",
            gdk::Key::plus,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
        ));
        assert!(matches_shortcut(
            "Ctrl+Slash",
            gdk::Key::question,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
        ));
    }

    #[test]
    fn unexpected_shift_is_rejected_for_plain_keys() {
        assert!(!matches_shortcut(
            "Ctrl+L",
            gdk::Key::l,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
        ));
    }

    #[test]
    fn missing_binding_falls_back_to_default() {
        let bindings = ShortcutBindings::default();
        assert_eq!(bindings.get("missing_action"), "Ctrl+K");
    }

    #[test]
    fn shortcuts_round_trip_save_and_load_from_temp_config_dir() {
        let temp = tempdir().expect("temp dir should be created");
        let _env = EnvOverride::xdg_config_home(temp.path());

        let mut bindings = ShortcutBindings::default();
        bindings.set("show_help", "Ctrl+Question");
        let saved_path = bindings.save().expect("shortcuts should be saved");
        assert!(saved_path.exists(), "saved shortcuts file should exist");

        let loaded = ShortcutBindings::load_or_default().expect("shortcuts should load");
        assert_eq!(loaded.get("show_help"), "Ctrl+Question");
    }

    #[test]
    fn loading_partial_shortcuts_file_fills_missing_defaults() {
        let temp = tempdir().expect("temp dir should be created");
        let _env = EnvOverride::xdg_config_home(temp.path());

        let path = shortcuts_path().expect("shortcuts path should resolve");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("shortcuts parent should be created");
        }
        std::fs::write(
            &path,
            r#"
[bindings]
show_help = "Ctrl+Question"
"#,
        )
        .expect("partial shortcuts file should be written");

        let loaded = ShortcutBindings::load_or_default().expect("shortcuts should load");
        assert_eq!(loaded.get("show_help"), "Ctrl+Question");
        assert_eq!(loaded.get("manage_shortcuts"), "Ctrl+Comma");
    }

    #[test]
    fn core_help_and_onboarding_shortcuts_are_present_and_valid() {
        let bindings = ShortcutBindings::default();
        assert_eq!(bindings.get("show_help"), "Ctrl+Slash");
        assert_eq!(bindings.get("show_onboarding"), "Ctrl+Shift+T");
        assert_eq!(bindings.get("open_tasks_browser"), "Ctrl+Shift+J");
        assert_eq!(bindings.get("open_sync_selection"), "Ctrl+Shift+S");
        validate_shortcut(&bindings.get("show_help")).expect("help shortcut should be valid");
        validate_shortcut(&bindings.get("show_onboarding"))
            .expect("onboarding shortcut should be valid");
        validate_shortcut(&bindings.get("open_tasks_browser"))
            .expect("tasks shortcut should be valid");
        validate_shortcut(&bindings.get("open_sync_selection"))
            .expect("sync selection shortcut should be valid");
    }
}
