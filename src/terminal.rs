use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtyPair, PtySize};

use crate::MainWindow;

/// The grid is fixed to match what TUIs expect (80 columns) and what fits the
/// 640x480 screen in the Departure Mono font.
pub const GRID_COLS: usize = 80;
pub const GRID_ROWS: usize = 22;

type WeakWindow = slint::Weak<MainWindow>;

/// A running interactive program attached to a pty.
struct TerminalHost {
    _master: Box<dyn MasterPty>,
    writer: Box<dyn Write + Send>,
    /// Process group of the child; the whole tree can be signalled via `-pgid`.
    group: i32,
}

/// Owns (at most) one interactive session at a time. Shared between the UI
/// (spawn/write), the joypad thread (L2+R2 kill) and the reader thread.
pub struct TerminalManager {
    host: Mutex<Option<TerminalHost>>,
}

impl TerminalManager {
    pub fn new() -> Arc<Self> {
        Arc::new(TerminalManager {
            host: Mutex::new(None),
        })
    }

    pub fn is_running(&self) -> bool {
        self.host.lock().unwrap().is_some()
    }

    /// Spawn `sh -c <exec>` attached to a pty, then relay its output to the
    /// UI as a screen grid.
    pub fn spawn(self: &Arc<Self>, exec: &str, setting_id: i32, weak_window: WeakWindow) {
        let pty = native_pty_system();
        let PtyPair { master, slave } = match pty.openpty(PtySize {
            rows: GRID_ROWS as u16,
            cols: GRID_COLS as u16,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(pair) => pair,
            Err(err) => {
                eprintln!("openpty error: {err}");
                set_running(&weak_window, false, 0);
                return;
            }
        };

        let mut builder = CommandBuilder::new("sh");
        builder.arg("-c");
        builder.arg(exec);

        let mut child = match slave.spawn_command(builder) {
            Ok(child) => child,
            Err(err) => {
                eprintln!("pty spawn error: {err}");
                set_running(&weak_window, false, 0);
                return;
            }
        };

        let mut reader = match master.try_clone_reader() {
            Ok(reader) => reader,
            Err(err) => {
                eprintln!("pty reader error: {err}");
                set_running(&weak_window, false, 0);
                return;
            }
        };
        let writer = match master.take_writer() {
            Ok(writer) => writer,
            Err(err) => {
                eprintln!("pty writer error: {err}");
                set_running(&weak_window, false, 0);
                return;
            }
        };

        let group = master
            .process_group_leader()
            .unwrap_or_else(|| child.process_id().unwrap_or(0) as i32);

        *self.host.lock().unwrap() = Some(TerminalHost {
            _master: master,
            writer,
            group,
        });

        set_running(&weak_window, true, setting_id);

        // Reader thread: turn raw pty bytes into a grid of characters and push
        // the serialized screen to the UI whenever it changes.
        let manager = self.clone();
        thread::spawn(move || {
            let mut screen = Screen::new();
            let mut buffer = [0u8; 4096];
            let mut last = String::new();
            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                screen.feed(&buffer[..n]);
                let text = screen.as_text();
                if text != last {
                    publish(&weak_window, &text);
                    last = text;
                }
            }
            let _ = child.wait();
            eprintln!("interactive session ended");
            manager.finish(&weak_window);
        });
    }

    /// Forward input (a key event or escape sequence) to the running program.
    pub fn write(&self, text: &str) {
        let mut guard = self.host.lock().unwrap();
        if let Some(host) = guard.as_mut() {
            let _ = host.writer.write_all(text.as_bytes());
            let _ = host.writer.flush();
        }
    }

    /// Terminate the running program (L2+R2 combo) and tear down the session.
    pub fn kill(&self, weak_window: &WeakWindow) {
        let host = self.host.lock().unwrap().take();
        if let Some(host) = host {
            eprintln!("terminating interactive program");
            unsafe {
                libc::kill(-host.group, libc::SIGTERM);
            }
            thread::sleep(Duration::from_millis(200));
            unsafe {
                libc::kill(-host.group, libc::SIGKILL);
            }
        } else {
            eprintln!("requested to terminate interactive program but none is running");
        }
        self.finish(weak_window);
    }

    /// Hard-kill any running program; used when the launcher itself is about
    /// to exit, so the program cannot outlive it.
    pub fn terminate_all(&self) {
        if let Some(host) = self.host.lock().unwrap().take() {
            unsafe {
                libc::kill(-host.group, libc::SIGKILL);
            }
        }
    }

    /// Clear the session state from the UI. Safe to call more than once.
    fn finish(&self, weak_window: &WeakWindow) {
        set_running(weak_window, false, 0);
    }
}

fn set_running(weak_window: &WeakWindow, active: bool, setting_id: i32) {
    let weak_window = weak_window.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(window) = weak_window.upgrade() {
            window.set_terminal_active(active);
            window.set_terminal_id(setting_id);
            if !active {
                window.set_terminal_output(String::new().into());
            }
        }
    })
    .ok();
}

fn publish(weak_window: &WeakWindow, text: &str) {
    let weak_window = weak_window.clone();
    let text: slint::SharedString = text.into();
    slint::invoke_from_event_loop(move || {
        if let Some(window) = weak_window.upgrade() {
            window.set_terminal_output(text.clone());
        }
    })
    .ok();
}

/// A minimal VT text screen that keeps a fixed-size character grid and
/// understands just enough CSI sequences for line-oriented CUIs/TUIs
/// (cursor addressing, erase, and line feeds).
struct Screen {
    cells: Vec<u8>,
    row: usize,
    col: usize,
    esc: bool,
    csi: Option<Vec<u8>>,
}

impl Screen {
    fn new() -> Self {
        Screen {
            cells: vec![b' '; GRID_COLS * GRID_ROWS],
            row: 0,
            col: 0,
            esc: false,
            csi: None,
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.step(b);
        }
    }

    fn step(&mut self, b: u8) {
        if let Some(params) = self.csi.as_mut() {
            if (0x40..=0x7e).contains(&b) {
                let params = self.csi.take().unwrap();
                self.csi_exec(&params, b);
            } else {
                params.push(b);
            }
            return;
        }

        if self.esc {
            self.esc = false;
            if b == b'[' {
                self.csi = Some(Vec::new());
            }
            return;
        }

        match b {
            0x1b => self.esc = true,
            b'\r' => self.col = 0,
            b'\n' => self.newline(),
            b'\x08' => {
                if self.col > 0 {
                    self.col -= 1;
                }
            }
            b'\t' => {
                self.col = (self.col / 8 + 1) * 8;
                if self.col >= GRID_COLS {
                    self.col = GRID_COLS - 1;
                }
            }
            b if b < 0x20 => {}
            b => self.put(b),
        }
    }

    fn idx(&self, row: usize, col: usize) -> usize {
        row * GRID_COLS + col
    }

    fn put(&mut self, b: u8) {
        if self.row < GRID_ROWS {
            let idx = self.row * GRID_COLS + self.col;
            self.cells[idx] = b;
        }
        self.col += 1;
        if self.col >= GRID_COLS {
            self.col = 0;
            self.newline();
        }
    }

    fn newline(&mut self) {
        if self.row + 1 >= GRID_ROWS {
            self.scroll_up();
        } else {
            self.row += 1;
        }
    }

    fn scroll_up(&mut self) {
        self.cells.drain(..GRID_COLS);
        self.cells.extend(std::iter::repeat_n(b' ', GRID_COLS));
    }

    fn erase_line(&mut self, mode: i32) {
        let row = self.row * GRID_COLS;
        match mode {
            0 => {
                for c in self.col..GRID_COLS {
                    self.cells[row + c] = b' ';
                }
            }
            1 => {
                for c in 0..=self.col {
                    self.cells[row + c] = b' ';
                }
            }
            _ => {
                for c in 0..GRID_COLS {
                    self.cells[row + c] = b' ';
                }
            }
        }
    }

    fn erase_screen(&mut self, mode: i32) {
        match mode {
            0 => {
                let row = self.row * GRID_COLS;
                for c in self.col..GRID_COLS {
                    self.cells[row + c] = b' ';
                }
                for r in self.row + 1..GRID_ROWS {
                    self.cells[r * GRID_COLS..(r + 1) * GRID_COLS].fill(b' ');
                }
            }
            1 => {
                let row = self.row * GRID_COLS;
                for c in 0..=self.col {
                    self.cells[row + c] = b' ';
                }
                for r in 0..self.row {
                    self.cells[r * GRID_COLS..(r + 1) * GRID_COLS].fill(b' ');
                }
            }
            _ => self.cells.fill(b' '),
        }
    }

    fn csi_exec(&mut self, accum: &[u8], final_byte: u8) {
        if matches!(final_byte, b'm' | b'h' | b'l' | b'r' | b's' | b'u' | b't' | b'Z') {
            return;
        }
        let p = parse_params(accum);
        let get = |i: usize| p.get(i).copied().unwrap_or(1);
        match final_byte {
            b'H' | b'f' => {
                self.row = ((get(0) - 1).max(0) as usize).min(GRID_ROWS - 1);
                self.col = ((get(1) - 1).max(0) as usize).min(GRID_COLS - 1);
            }
            b'A' => self.row = self.row.saturating_sub(get(0) as usize),
            b'B' => self.row = (self.row + get(0) as usize).min(GRID_ROWS - 1),
            b'C' => self.col = (self.col + get(0) as usize).min(GRID_COLS - 1),
            b'D' => self.col = self.col.saturating_sub(get(0) as usize),
            b'G' => self.col = ((get(0) - 1).max(0) as usize).min(GRID_COLS - 1),
            b'd' => self.row = ((get(0) - 1).max(0) as usize).min(GRID_ROWS - 1),
            b'J' => self.erase_screen(p.first().copied().unwrap_or(0)),
            b'K' => self.erase_line(p.first().copied().unwrap_or(0)),
            _ => {}
        }
    }

    fn as_text(&self) -> String {
        let mut s = String::with_capacity(GRID_COLS * GRID_ROWS + GRID_ROWS);
        for r in 0..GRID_ROWS {
            if r > 0 {
                s.push('\n');
            }
            s.extend(
                self.cells[self.idx(r, 0)..self.idx(r, GRID_COLS)]
                    .iter()
                    .map(|&b| b as char),
            );
        }
        s
    }
}

/// Split a CSI parameter accumulator into integers. Missing values and empty
/// requests default to 1, matching the VT specification.
fn parse_params(accum: &[u8]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut cur: i32 = 0;
    let mut has_cur = false;
    for &b in accum {
        if b.is_ascii_digit() {
            cur = cur * 10 + (b - b'0') as i32;
            has_cur = true;
        } else if b == b';' {
            out.push(if has_cur { cur } else { 1 });
            cur = 0;
            has_cur = false;
        }
    }
    if has_cur {
        out.push(cur);
    } else if out.is_empty() {
        out.push(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(screen: &Screen, row: usize) -> String {
        screen.cells[screen.idx(row, 0)..screen.idx(row, GRID_COLS)]
            .iter()
            .map(|&b| b as char)
            .collect()
    }

    #[test]
    fn writes_and_wraps() {
        let mut s = Screen::new();
        s.feed(b"hello");
        assert!(line(&s, 0).starts_with("hello"));
        s.feed(b"\r\nworld");
        assert!(line(&s, 1).starts_with("world"));
    }

    #[test]
    fn csi_cursor_positions() {
        let mut s = Screen::new();
        s.feed(b"\x1b[2;3H");
        assert_eq!(s.row, 1);
        assert_eq!(s.col, 2);
        s.feed(b"A");
        assert!(line(&s, 1).starts_with("  A"));
    }

    #[test]
    fn csi_scrolls_on_clear_keep_position() {
        let mut s = Screen::new();
        s.feed(b"\x1b[2J");
        assert!(line(&s, 0).trim().is_empty());
        assert!(line(&s, GRID_ROWS - 1).trim().is_empty());
    }

    #[test]
    fn newline_scrolls() {
        let mut s = Screen::new();
        for _ in 0..GRID_ROWS {
            s.feed(b"\n");
        }
        for r in 0..GRID_ROWS {
            assert_eq!(line(&s, r), " ".repeat(GRID_COLS));
        }
        assert_eq!(s.row, GRID_ROWS - 1);
    }

    #[test]
    fn parse_params_defaults() {
        assert_eq!(parse_params(b""), vec![1]);
        assert_eq!(parse_params(b"12"), vec![12]);
        assert_eq!(parse_params(b"2;3"), vec![2, 3]);
        assert_eq!(parse_params(b"?"), vec![1]);
    }
}