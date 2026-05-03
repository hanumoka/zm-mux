use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::keybinding::KeyBinding;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".to_string(),
            size: 16.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindingsConfig {
    pub new_tab: String,
    pub close_tab: String,
    pub close_pane: String,
    pub split_horizontal: String,
    pub split_vertical: String,
    pub next_tab: String,
    pub prev_tab: String,
    pub focus_left: String,
    pub focus_right: String,
    pub focus_up: String,
    pub focus_down: String,
    pub search: String,
    pub copy: String,
    pub paste: String,
    pub select_all: String,
}

impl Default for KeyBindingsConfig {
    fn default() -> Self {
        Self {
            new_tab: "Ctrl+T".to_string(),
            close_tab: "Ctrl+Shift+W".to_string(),
            close_pane: "Ctrl+Shift+P".to_string(),
            split_horizontal: "Ctrl+Shift+D".to_string(),
            split_vertical: "Ctrl+Shift+E".to_string(),
            next_tab: "Ctrl+Tab".to_string(),
            prev_tab: "Ctrl+Shift+Tab".to_string(),
            focus_left: "Alt+Left".to_string(),
            focus_right: "Alt+Right".to_string(),
            focus_up: "Alt+Up".to_string(),
            focus_down: "Alt+Down".to_string(),
            search: "Ctrl+Shift+F".to_string(),
            copy: "Ctrl+Shift+C".to_string(),
            paste: "Ctrl+Shift+V".to_string(),
            select_all: "Ctrl+Shift+A".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedKeyBindings {
    pub new_tab: KeyBinding,
    pub close_tab: KeyBinding,
    pub close_pane: KeyBinding,
    pub split_horizontal: KeyBinding,
    pub split_vertical: KeyBinding,
    pub next_tab: KeyBinding,
    pub prev_tab: KeyBinding,
    pub focus_left: KeyBinding,
    pub focus_right: KeyBinding,
    pub focus_up: KeyBinding,
    pub focus_down: KeyBinding,
    pub search: KeyBinding,
    pub copy: KeyBinding,
    pub paste: KeyBinding,
    pub select_all: KeyBinding,
}

impl KeyBindingsConfig {
    /// Parse all string entries into structured `KeyBinding`s.  Stops at the
    /// first error and reports which field failed so a malformed config is
    /// easy to debug.
    pub fn parse(&self) -> Result<ParsedKeyBindings, String> {
        let p = |name: &str, s: &str| -> Result<KeyBinding, String> {
            s.parse()
                .map_err(|e| format!("keybindings.{name} = {s:?}: {e}"))
        };
        Ok(ParsedKeyBindings {
            new_tab: p("new_tab", &self.new_tab)?,
            close_tab: p("close_tab", &self.close_tab)?,
            close_pane: p("close_pane", &self.close_pane)?,
            split_horizontal: p("split_horizontal", &self.split_horizontal)?,
            split_vertical: p("split_vertical", &self.split_vertical)?,
            next_tab: p("next_tab", &self.next_tab)?,
            prev_tab: p("prev_tab", &self.prev_tab)?,
            focus_left: p("focus_left", &self.focus_left)?,
            focus_right: p("focus_right", &self.focus_right)?,
            focus_up: p("focus_up", &self.focus_up)?,
            focus_down: p("focus_down", &self.focus_down)?,
            search: p("search", &self.search)?,
            copy: p("copy", &self.copy)?,
            paste: p("paste", &self.paste)?,
            select_all: p("select_all", &self.select_all)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorsConfig {
    pub background: String,
    pub foreground: String,
}

impl Default for ColorsConfig {
    fn default() -> Self {
        Self {
            background: "#1a1a2e".to_string(),
            foreground: "#e0e0e0".to_string(),
        }
    }
}

impl ColorsConfig {
    pub fn background_rgb(&self) -> (u8, u8, u8) {
        parse_hex_color(&self.background).unwrap_or((0x1a, 0x1a, 0x2e))
    }

    pub fn foreground_rgb(&self) -> (u8, u8, u8) {
        parse_hex_color(&self.foreground).unwrap_or((0xe0, 0xe0, 0xe0))
    }
}

fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    /// Empty = use platform default (`portable_pty::CommandBuilder::new_default_prog`).
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollbackConfig {
    pub max_lines: usize,
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self { max_lines: 10_000 }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub keybindings: KeyBindingsConfig,
    pub colors: ColorsConfig,
    pub shell: ShellConfig,
    pub scrollback: ScrollbackConfig,
}

impl Config {
    pub fn load() -> Self {
        let path = match config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => {
                let cfg = Self::default();
                let _ = cfg.write_default(&path);
                cfg
            }
        }
    }

    fn write_default(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).unwrap_or_default();
        std::fs::write(path, text)
    }
}

fn config_path() -> Option<PathBuf> {
    let dir = config_dir()?;
    Some(dir.join("config.toml"))
}

fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|v| PathBuf::from(v).join("zm-mux"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|v| PathBuf::from(v).join("Library/Application Support/zm-mux"))
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        Some(PathBuf::from(xdg).join("zm-mux"))
    } else {
        std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".config/zm-mux"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_font_is_jetbrains_mono() {
        let cfg = Config::default();
        assert_eq!(cfg.font.family, "JetBrains Mono");
        assert!(cfg.font.size > 0.0);
    }

    #[test]
    fn round_trip_toml() {
        let cfg = Config::default();
        let s = toml::to_string(&cfg).unwrap();
        let cfg2: Config = toml::from_str(&s).unwrap();
        assert_eq!(cfg.font.family, cfg2.font.family);
        assert_eq!(cfg.font.size, cfg2.font.size);
        assert_eq!(cfg.keybindings.new_tab, cfg2.keybindings.new_tab);
        assert_eq!(cfg.colors.background, cfg2.colors.background);
        assert_eq!(cfg.scrollback.max_lines, cfg2.scrollback.max_lines);
    }

    #[test]
    fn default_keybindings_parse_clean() {
        let cfg = KeyBindingsConfig::default();
        let parsed = cfg.parse().expect("default keybindings must parse");
        assert_eq!(parsed.new_tab, "Ctrl+T".parse().unwrap());
        assert_eq!(parsed.close_tab, "Ctrl+Shift+W".parse().unwrap());
        assert_eq!(parsed.close_pane, "Ctrl+Shift+P".parse().unwrap());
        assert_eq!(parsed.focus_left, "Alt+Left".parse().unwrap());
    }

    #[test]
    fn malformed_keybinding_reports_field_name() {
        let mut cfg = KeyBindingsConfig::default();
        cfg.new_tab = "NotAKey".to_string();
        let err = cfg.parse().unwrap_err();
        assert!(err.contains("new_tab"));
        assert!(err.contains("NotAKey"));
    }

    #[test]
    fn colors_hex_round_trip() {
        let cfg = ColorsConfig::default();
        assert_eq!(cfg.background_rgb(), (0x1a, 0x1a, 0x2e));
        assert_eq!(cfg.foreground_rgb(), (0xe0, 0xe0, 0xe0));

        let custom = ColorsConfig {
            background: "#102030".to_string(),
            foreground: "abcdef".to_string(),
        };
        assert_eq!(custom.background_rgb(), (0x10, 0x20, 0x30));
        assert_eq!(custom.foreground_rgb(), (0xab, 0xcd, 0xef));

        // Malformed → defaults.
        let bad = ColorsConfig {
            background: "nope".to_string(),
            foreground: "#12".to_string(),
        };
        assert_eq!(bad.background_rgb(), (0x1a, 0x1a, 0x2e));
        assert_eq!(bad.foreground_rgb(), (0xe0, 0xe0, 0xe0));
    }

    #[test]
    fn old_config_with_only_font_still_loads() {
        // Backward compat: a config file written before keybindings/colors/
        // shell/scrollback existed should still deserialize via serde(default).
        let toml_str = r#"
[font]
family = "Custom Font"
size = 14.0
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.font.family, "Custom Font");
        assert_eq!(cfg.font.size, 14.0);
        // Defaults for the new sections.
        assert_eq!(cfg.keybindings.new_tab, "Ctrl+T");
        assert_eq!(cfg.scrollback.max_lines, 10_000);
    }
}
