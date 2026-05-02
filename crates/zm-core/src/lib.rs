pub mod config;
pub mod keybinding;
pub use config::{
    ColorsConfig, Config, FontConfig, KeyBindingsConfig, ParsedKeyBindings, ScrollbackConfig,
    ShellConfig,
};
pub use keybinding::{KeyBinding, KeyDef, ModBits};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZmError {
    #[error("PTY error: {0}")]
    Pty(String),

    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("Render error: {0}")]
    Render(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type ZmResult<T> = Result<T, ZmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = ZmError::Pty("test".into());
        assert_eq!(err.to_string(), "PTY error: test");
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let zm_err: ZmError = io_err.into();
        assert!(zm_err.to_string().contains("missing"));
    }
}
