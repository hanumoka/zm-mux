use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
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
    } else {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg).join("zm-mux"))
        } else {
            std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".config/zm-mux"))
        }
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
    }
}
