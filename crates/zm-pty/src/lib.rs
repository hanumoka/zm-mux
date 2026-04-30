use portable_pty::{CommandBuilder, MasterPty, PtySize, SlavePty, native_pty_system};
use std::io::{Read, Write};
use zm_core::{ZmError, ZmResult};

pub struct ZmPtyProcess {
    reader: Option<Box<dyn Read + Send>>,
    writer: Box<dyn Write + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    _slave: Box<dyn SlavePty + Send>,
}

pub fn spawn_pty(rows: u16, cols: u16, cmd: CommandBuilder) -> ZmResult<ZmPtyProcess> {
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
    let cmd = CommandBuilder::new_default_prog();
    spawn_pty(rows, cols, cmd)
}

impl ZmPtyProcess {
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
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

    #[test]
    fn pty_spawn_and_read() {
        #[cfg(windows)]
        let cmd = CommandBuilder::new("cmd.exe");
        #[cfg(not(windows))]
        let cmd = CommandBuilder::new("bash");

        let mut proc = spawn_pty(24, 80, cmd).expect("spawn should succeed");
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
        #[cfg(windows)]
        let cmd = CommandBuilder::new("cmd.exe");
        #[cfg(not(windows))]
        let cmd = CommandBuilder::new("bash");

        let mut proc = spawn_pty(24, 80, cmd).expect("spawn");
        std::thread::sleep(std::time::Duration::from_millis(500));

        let result = proc.write_input(b"echo test\r\n");
        assert!(result.is_ok(), "write_input should succeed");

        proc.kill().ok();
    }

    #[test]
    fn pty_kill() {
        #[cfg(windows)]
        let cmd = CommandBuilder::new("cmd.exe");
        #[cfg(not(windows))]
        let cmd = CommandBuilder::new("bash");

        let mut proc = spawn_pty(24, 80, cmd).expect("spawn");
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
