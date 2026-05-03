use portable_pty::{CommandBuilder, MasterPty, PtySize, SlavePty, native_pty_system};
use std::io::{Read, Write};
use zm_core::{ShellConfig, ZmError, ZmResult};

pub struct ZmPtyProcess {
    reader: Option<Box<dyn Read + Send>>,
    writer: Box<dyn Write + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    _slave: Box<dyn SlavePty + Send>,
}

/// Spawn a PTY running the shell described by `shell`.  An empty
/// `program` falls back to `portable_pty::CommandBuilder::new_default_prog`
/// which picks `cmd.exe` on Windows and `$SHELL` (or `/bin/sh`) on
/// POSIX hosts.
pub fn spawn_pty(
    rows: u16,
    cols: u16,
    shell: &ShellConfig,
    env_vars: &[(&str, &str)],
    cwd: Option<&str>,
) -> ZmResult<ZmPtyProcess> {
    let mut cmd = if shell.program.is_empty() {
        #[cfg(windows)]
        {
            let mut c = CommandBuilder::new("cmd.exe");
            c.arg("/K");
            c.arg("chcp 65001>nul");
            c
        }
        #[cfg(not(windows))]
        {
            CommandBuilder::new_default_prog()
        }
    } else {
        let mut c = CommandBuilder::new(&shell.program);
        for arg in &shell.args {
            c.arg(arg);
        }
        c
    };
    for &(k, v) in env_vars {
        cmd.env(k, v);
    }
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| ZmError::Pty(e.to_string()))?;

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| ZmError::Pty(e.to_string()))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| ZmError::Pty(e.to_string()))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| ZmError::Pty(e.to_string()))?;

    Ok(ZmPtyProcess {
        reader: Some(reader),
        writer,
        child,
        _master: pair.master,
        _slave: pair.slave,
    })
}

pub fn spawn_default_shell(rows: u16, cols: u16) -> ZmResult<ZmPtyProcess> {
    spawn_pty(rows, cols, &ShellConfig::default(), &[], None)
}

impl ZmPtyProcess {
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
    }

    pub fn has_reader(&self) -> bool {
        self.reader.is_some()
    }

    pub fn write_input(&mut self, data: &[u8]) -> ZmResult<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn try_wait(&mut self) -> ZmResult<Option<u32>> {
        match self.child.try_wait() {
            Ok(Some(status)) => Ok(Some(status.exit_code())),
            Ok(None) => Ok(None),
            Err(e) => Err(ZmError::Pty(e.to_string())),
        }
    }

    pub fn kill(&mut self) -> ZmResult<()> {
        self.child.kill().map_err(|e| ZmError::Pty(e.to_string()))
    }

    pub fn resize(&self, rows: u16, cols: u16) -> ZmResult<()> {
        self._master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ZmError::Pty(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // Uses bare cmd.exe without chcp 65001 — tests only check basic
    // PTY I/O, not UTF-8 encoding. Production spawn_pty with empty
    // ShellConfig uses cmd.exe /K chcp 65001>nul on Windows.
    fn test_shell() -> ShellConfig {
        #[cfg(windows)]
        let program = "cmd.exe".to_string();
        #[cfg(not(windows))]
        let program = "bash".to_string();
        ShellConfig {
            program,
            args: Vec::new(),
        }
    }

    #[test]
    fn pty_spawn_and_read() {
        let shell = test_shell();
        let mut proc = spawn_pty(24, 80, &shell, &[], None).expect("spawn should succeed");
        let reader = proc.take_reader().expect("reader");

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            match reader.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let _ = tx.send(n);
                }
                _ => {
                    let _ = tx.send(0);
                }
            }
        });

        let bytes_read = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap_or(0);

        assert!(
            bytes_read > 0,
            "PTY should produce initial output from shell"
        );
        proc.kill().ok();
    }

    #[test]
    fn pty_write_does_not_error() {
        let shell = test_shell();
        let mut proc = spawn_pty(24, 80, &shell, &[], None).expect("spawn");
        std::thread::sleep(std::time::Duration::from_millis(500));

        let result = proc.write_input(b"echo test\r\n");
        assert!(result.is_ok(), "write_input should succeed");

        proc.kill().ok();
    }

    #[test]
    fn pty_kill() {
        let shell = test_shell();
        let mut proc = spawn_pty(24, 80, &shell, &[], None).expect("spawn");
        assert!(
            proc.try_wait().unwrap().is_none(),
            "process should be running"
        );

        proc.kill().expect("kill should succeed");
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            proc.try_wait().unwrap().is_some(),
            "process should have exited"
        );
    }
}
