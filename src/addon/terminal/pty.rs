//! The shell process behind a session: a ConPTY, a reader thread feeding the
//! emulator, and a writer for keystrokes.
//!
//! A session exists only while it is open: spawning is the only way to start a
//! process, and dropping the session kills it. Reading and parsing run on the
//! thread, never on the UI thread.
//!
//! ponytail: the child is killed with `Child::kill` (no console control event, so
//! a shell's own exit hooks do not run); the reader thread exits when the pty closes.
//! upgrade: signal the process group and join the thread on close.

use super::ansi::Emulator;
use async_channel::Sender;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Size a session starts at, before the panel is measured.
const START_COLS: u16 = 100;
const START_ROWS: u16 = 24;

/// The pty's input side: keystrokes from the UI and report answers from the reader
/// thread both go out here.
type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

/// The shells the panel offers, in the order the picker lists them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// `PowerShell` is the shell's own name, so the variant keeps it.
#[allow(clippy::enum_variant_names)]
pub enum Shell {
    PowerShell,
    Cmd,
    GitBash,
}

impl Shell {
    pub const ALL: [Shell; 3] = [Shell::PowerShell, Shell::Cmd, Shell::GitBash];

    pub fn label(self) -> &'static str {
        match self {
            Shell::PowerShell => "PowerShell",
            Shell::Cmd => "cmd",
            Shell::GitBash => "Git Bash",
        }
    }

    /// The program to run and its arguments.
    fn command(self) -> (String, Vec<String>) {
        match self {
            Shell::PowerShell => ("powershell.exe".into(), vec!["-NoLogo".into()]),
            Shell::Cmd => ("cmd.exe".into(), vec![]),
            Shell::GitBash => (
                git_bash().to_string_lossy().into_owned(),
                vec!["--login".into(), "-i".into()],
            ),
        }
    }
}

/// Git Bash where its installer puts it, else `bash.exe` from PATH (which may not
/// exist; the session then reports the spawn error in the panel).
fn git_bash() -> PathBuf {
    let candidates = [
        std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("Git/bin/bash.exe")),
        std::env::var_os("ProgramFiles(x86)").map(|p| PathBuf::from(p).join("Git/bin/bash.exe")),
        std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Programs/Git/bin/bash.exe")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("bash.exe"))
}

/// One running shell.
pub struct Session {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    shell: Shell,
    emulator: Arc<Mutex<Emulator>>,
    size: (u16, u16),
    alive: Arc<AtomicBool>,
}

impl Session {
    /// Start `shell` in `cwd`. `wake` is signalled whenever output arrives.
    pub fn spawn(shell: Shell, cwd: Option<&Path>, wake: Sender<()>) -> io::Result<Self> {
        let (program, args) = shell.command();
        let pty = native_pty_system();
        let pair = pty.openpty(size(START_COLS, START_ROWS)).map_err(failed)?;
        let mut command = CommandBuilder::new(&program);
        command.args(args);
        if let Some(dir) = cwd {
            command.cwd(dir);
        }
        let child = pair.slave.spawn_command(command).map_err(failed)?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(failed)?;
        let writer = Arc::new(Mutex::new(pair.master.take_writer().map_err(failed)?));
        let emulator = Arc::new(Mutex::new(Emulator::new(
            START_COLS as usize,
            START_ROWS as usize,
        )));
        let alive = Arc::new(AtomicBool::new(true));
        let handle = emulator.clone();
        let flag = alive.clone();
        let reader_writer = writer.clone();
        std::thread::Builder::new()
            .name("smithy-terminal".into())
            .spawn(move || read_loop(reader, handle, flag, wake, reader_writer))
            .map_err(failed)?;
        Ok(Self {
            master: pair.master,
            child,
            writer,
            shell,
            emulator,
            size: (START_COLS, START_ROWS),
            alive,
        })
    }

    pub fn shell(&self) -> Shell {
        self.shell
    }

    /// The screen, shared with the reader thread.
    pub fn emulator(&self) -> Arc<Mutex<Emulator>> {
        self.emulator.clone()
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    /// Tell the shell and the grid about a new panel size.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if (cols, rows) == self.size || cols == 0 || rows == 0 {
            return;
        }
        self.size = (cols, rows);
        let _ = self.master.resize(size(cols, rows));
        if let Ok(mut emulator) = self.emulator.lock() {
            emulator.grid_mut().resize(cols as usize, rows as usize);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_loop(
    mut reader: Box<dyn Read + Send>,
    emulator: Arc<Mutex<Emulator>>,
    alive: Arc<AtomicBool>,
    wake: Sender<()>,
    writer: SharedWriter,
) {
    let mut buf = [0u8; 8192];
    while alive.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                // The screen answers a report request (e.g. ConPTY's cursor query)
                // before the lock is released, then the pty gets the answer back.
                let reply = match emulator.lock() {
                    Ok(mut emulator) => {
                        emulator.feed(&buf[..n]);
                        emulator.take_reply()
                    }
                    Err(_) => Vec::new(),
                };
                if !reply.is_empty() {
                    if let Ok(mut writer) = writer.lock() {
                        let _ = writer.write_all(&reply);
                        let _ = writer.flush();
                    }
                }
                // Capacity 1: pending wake-ups coalesce, a full channel means the
                // UI has not drawn the last one yet and does not need another.
                let _ = wake.try_send(());
            }
        }
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn failed(err: impl std::fmt::Display) -> io::Error {
    io::Error::other(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shell_has_a_label_and_a_command() {
        for shell in Shell::ALL {
            assert!(!shell.label().is_empty());
            assert!(!shell.command().0.is_empty());
        }
        assert_eq!(Shell::Cmd.command().0, "cmd.exe");
    }
}
