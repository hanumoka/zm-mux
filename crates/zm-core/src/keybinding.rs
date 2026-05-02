use std::str::FromStr;

/// Modifier bitfield, deliberately winit-agnostic.  zm-app converts
/// `winit::keyboard::ModifiersState` into this so `zm-core` keeps its
/// dependency graph small.  Order: Ctrl=1, Shift=2, Alt=4, Super=8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModBits(u8);

impl ModBits {
    pub const EMPTY: Self = Self(0);
    pub const CTRL: Self = Self(0b0001);
    pub const SHIFT: Self = Self(0b0010);
    pub const ALT: Self = Self(0b0100);
    pub const SUPER: Self = Self(0b1000);

    pub const fn empty() -> Self {
        Self::EMPTY
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for ModBits {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ModBits {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Backend-agnostic key identity.  Mirrors the subset of keys we accept
/// in keybindings.  Plain printable characters are `Char`; special keys
/// have their own variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyDef {
    Char(char),
    Tab,
    Enter,
    Escape,
    Backspace,
    Space,
    PageUp,
    PageDown,
    Home,
    End,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub mods: ModBits,
    pub key: KeyDef,
}

impl KeyBinding {
    /// Match against a runtime event.  `Char` comparison is case-insensitive
    /// — when Shift is held the platform may upcase the character, so the
    /// binding `"Ctrl+Shift+D"` should match either 'd' or 'D'.
    pub fn matches(&self, mods: ModBits, key: &KeyDef) -> bool {
        if self.mods != mods {
            return false;
        }
        match (&self.key, key) {
            (KeyDef::Char(a), KeyDef::Char(b)) => a.eq_ignore_ascii_case(b),
            (a, b) => a == b,
        }
    }
}

impl FromStr for KeyBinding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err("empty keybinding string".to_string());
        }
        let mut mods = ModBits::empty();
        let mut key: Option<KeyDef> = None;
        for raw in s.split('+') {
            let part = raw.trim();
            if part.is_empty() {
                return Err(format!("empty token in keybinding: {s:?}"));
            }
            match part {
                "Ctrl" | "Control" => mods |= ModBits::CTRL,
                "Shift" => mods |= ModBits::SHIFT,
                "Alt" => mods |= ModBits::ALT,
                "Super" | "Win" | "Cmd" | "Meta" => mods |= ModBits::SUPER,
                _ => {
                    let kd = parse_key_token(part)?;
                    if key.replace(kd).is_some() {
                        return Err(format!("multiple keys in binding: {s:?}"));
                    }
                }
            }
        }
        match key {
            Some(k) => Ok(Self { mods, key: k }),
            None => Err(format!("no key in binding: {s:?}")),
        }
    }
}

fn parse_key_token(t: &str) -> Result<KeyDef, String> {
    if t.chars().count() == 1 {
        return Ok(KeyDef::Char(t.chars().next().unwrap()));
    }
    Ok(match t {
        "Tab" => KeyDef::Tab,
        "Enter" | "Return" => KeyDef::Enter,
        "Escape" | "Esc" => KeyDef::Escape,
        "Backspace" | "BackSpace" => KeyDef::Backspace,
        "Space" => KeyDef::Space,
        "PageUp" | "PgUp" => KeyDef::PageUp,
        "PageDown" | "PgDn" => KeyDef::PageDown,
        "Home" => KeyDef::Home,
        "End" => KeyDef::End,
        "Up" | "ArrowUp" => KeyDef::ArrowUp,
        "Down" | "ArrowDown" => KeyDef::ArrowDown,
        "Left" | "ArrowLeft" => KeyDef::ArrowLeft,
        "Right" | "ArrowRight" => KeyDef::ArrowRight,
        _ => return Err(format!("unknown key token: {t:?}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let kb: KeyBinding = "Ctrl+T".parse().unwrap();
        assert_eq!(kb.mods, ModBits::CTRL);
        assert_eq!(kb.key, KeyDef::Char('T'));
    }

    #[test]
    fn parse_ctrl_shift() {
        let kb: KeyBinding = "Ctrl+Shift+D".parse().unwrap();
        assert_eq!(kb.mods, ModBits::CTRL | ModBits::SHIFT);
        assert_eq!(kb.key, KeyDef::Char('D'));
    }

    #[test]
    fn parse_named_key() {
        let kb: KeyBinding = "Alt+Left".parse().unwrap();
        assert_eq!(kb.mods, ModBits::ALT);
        assert_eq!(kb.key, KeyDef::ArrowLeft);
    }

    #[test]
    fn parse_ctrl_tab() {
        let kb: KeyBinding = "Ctrl+Tab".parse().unwrap();
        assert_eq!(kb.mods, ModBits::CTRL);
        assert_eq!(kb.key, KeyDef::Tab);
    }

    #[test]
    fn parse_aliases() {
        assert_eq!(
            "Control+Return".parse::<KeyBinding>().unwrap(),
            "Ctrl+Enter".parse::<KeyBinding>().unwrap()
        );
        assert_eq!(
            "Win+T".parse::<KeyBinding>().unwrap(),
            "Super+T".parse::<KeyBinding>().unwrap()
        );
    }

    #[test]
    fn parse_rejects_empty() {
        assert!("".parse::<KeyBinding>().is_err());
        assert!("Ctrl++T".parse::<KeyBinding>().is_err());
        assert!("Ctrl+".parse::<KeyBinding>().is_err());
    }

    #[test]
    fn parse_rejects_multiple_keys() {
        assert!("Ctrl+T+W".parse::<KeyBinding>().is_err());
    }

    #[test]
    fn parse_rejects_only_modifiers() {
        assert!("Ctrl+Shift".parse::<KeyBinding>().is_err());
    }

    #[test]
    fn matches_case_insensitive_char() {
        let kb: KeyBinding = "Ctrl+T".parse().unwrap();
        assert!(kb.matches(ModBits::CTRL, &KeyDef::Char('t')));
        assert!(kb.matches(ModBits::CTRL, &KeyDef::Char('T')));
        assert!(!kb.matches(ModBits::CTRL | ModBits::SHIFT, &KeyDef::Char('T')));
    }

    #[test]
    fn matches_named_exact() {
        let kb: KeyBinding = "Alt+Left".parse().unwrap();
        assert!(kb.matches(ModBits::ALT, &KeyDef::ArrowLeft));
        assert!(!kb.matches(ModBits::ALT, &KeyDef::ArrowRight));
        assert!(!kb.matches(ModBits::EMPTY, &KeyDef::ArrowLeft));
    }

    #[test]
    fn modbits_contains() {
        let m = ModBits::CTRL | ModBits::SHIFT;
        assert!(m.contains(ModBits::CTRL));
        assert!(m.contains(ModBits::SHIFT));
        assert!(!m.contains(ModBits::ALT));
    }
}
