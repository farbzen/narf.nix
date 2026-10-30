//! Runs narf inside a pseudo terminal and mirrors its screen.

use std::{
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, ExitStatus, MasterPty, PtySize, native_pty_system};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const SCROLLBACK: usize = 10_000;

pub struct Runner {
    pub parser: Arc<Mutex<vt100::Parser>>,
    pub exit: Option<ExitStatus>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    rows: u16,
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl Runner {
    pub fn spawn(program: &Path, args: &[String], rows: u16, cols: u16) -> Result<Self> {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let pair = native_pty_system()
            .openpty(pty_size(rows, cols))
            .context("could not open a pty")?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.env("TERM", "xterm-256color");
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("could not start {}", program.display()))?;
        // Only the child may hold the slave, otherwise the reader never sees EOF.
        drop(pair.slave);

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK)));
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let sink = Arc::clone(&parser);
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n @ 1..) = reader.read(&mut buf) {
                sink.lock().unwrap().process(&buf[..n]);
            }
        });

        Ok(Self {
            parser,
            exit: None,
            master: pair.master,
            writer,
            child,
            rows,
        })
    }

    pub fn running(&self) -> bool {
        self.exit.is_none()
    }

    /// Picks up the exit status once the child has finished.
    pub fn poll(&mut self) {
        if self.exit.is_none() {
            self.exit = self.child.try_wait().ok().flatten();
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        self.rows = rows;
        let _ = self.master.resize(pty_size(rows, cols));
        self.parser
            .lock()
            .unwrap()
            .screen_mut()
            .set_size(rows, cols);
    }

    /// Scrolls the view by half a page; positive values go back in history.
    pub fn scroll(&mut self, pages: i32) {
        let mut parser = self.parser.lock().unwrap();
        let screen = parser.screen_mut();
        let step = (self.rows / 2).max(1) as i64 * pages as i64;
        let offset = (screen.scrollback() as i64 + step).max(0) as usize;
        screen.set_scrollback(offset);
    }

    pub fn send_key(&mut self, key: KeyEvent) {
        if let Some(bytes) = key_bytes(key) {
            self.parser.lock().unwrap().screen_mut().set_scrollback(0);
            let _ = self.writer.write_all(&bytes);
            let _ = self.writer.flush();
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        if self.running() {
            let _ = self.child.kill();
        }
    }
}

/// Encodes a key press the way a terminal (xterm) would send it.
fn key_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let bytes = match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let c = c.to_ascii_lowercase();
            if !c.is_ascii_lowercase() {
                return None;
            }
            vec![c as u8 - b'a' + 1]
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        _ => return None,
    };
    Some(bytes)
}
