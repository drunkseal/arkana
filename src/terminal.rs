use std::sync::Mutex;

use crate::MainWindow;

pub const GRID_COLS: usize = 80;
pub const GRID_ROWS: usize = 22;

pub struct Screen {
    cells: [u8; GRID_COLS * GRID_ROWS],
    row: usize,
    col: usize,
    esc: bool,
    csi: bool,
    csi_params: Vec<u8>,
}

impl Screen {
    pub fn new() -> Self {
        Self {
            cells: [b' '; GRID_COLS * GRID_ROWS],
            row: 0,
            col: 0,
            esc: false,
            csi: false,
            csi_params: Vec::new(),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        for &b in data {
            self.step(b);
        }
    }

    fn step(&mut self, b: u8) {
        if self.csi {
            if (0x40..=0x7e).contains(&b) {
                self.csi = false;
                let params = std::mem::take(&mut self.csi_params);
                self.csi_exec(&params, b);
            } else {
                self.csi_params.push(b);
            }
            return;
        }

        if self.esc {
            self.esc = false;
            if b == b'[' {
                self.csi = true;
                self.csi_params.clear();
            }
            return;
        }

        match b {
            0x1b => self.esc = true,
            b'\r' => self.col = 0,
            b'\n' => self.newline(),
            0x08 => {
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
            _ => {
                if b >= 0x20 {
                    self.put(b);
                }
            }
        }
    }

    fn put(&mut self, b: u8) {
        if self.row < GRID_ROWS {
            self.cells[self.row * GRID_COLS + self.col] = b;
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
        self.cells.copy_within(GRID_COLS..GRID_COLS * GRID_ROWS, 0);
        self.cells[GRID_COLS * (GRID_ROWS - 1)..GRID_COLS * GRID_ROWS].fill(b' ');
    }

    fn erase_line(&mut self, mode: i32) {
        let row_start = self.row * GRID_COLS;
        match mode {
            0 => {
                self.cells[row_start + self.col..row_start + GRID_COLS].fill(b' ');
            }
            1 => {
                self.cells[row_start..=(row_start + self.col).min(row_start + GRID_COLS - 1)]
                    .fill(b' ');
            }
            _ => {
                self.cells[row_start..row_start + GRID_COLS].fill(b' ');
            }
        }
    }

    fn erase_screen(&mut self, mode: i32) {
        match mode {
            0 => {
                self.erase_line(0);
                for r in (self.row + 1)..GRID_ROWS {
                    self.cells[r * GRID_COLS..(r + 1) * GRID_COLS].fill(b' ');
                }
            }
            1 => {
                self.erase_line(1);
                for r in 0..self.row {
                    self.cells[r * GRID_COLS..(r + 1) * GRID_COLS].fill(b' ');
                }
            }
            _ => {
                self.cells.fill(b' ');
            }
        }
    }

    fn csi_exec(&mut self, params: &[u8], final_byte: u8) {
        if matches!(
            final_byte,
            b'm' | b'h' | b'l' | b'r' | b's' | b'u' | b't' | b'Z'
        ) {
            return;
        }

        let p = parse_params(params);
        let get = |i: usize| -> usize {
            if i < p.len() {
                p[i].max(1) as usize
            } else {
                1
            }
        };

        match final_byte {
            b'H' | b'f' => {
                self.row = get(0).saturating_sub(1).min(GRID_ROWS - 1);
                self.col = get(1).saturating_sub(1).min(GRID_COLS - 1);
            }
            b'A' => {
                self.row = self.row.saturating_sub(get(0));
            }
            b'B' => {
                self.row = (self.row + get(0)).min(GRID_ROWS - 1);
            }
            b'C' => {
                self.col = (self.col + get(0)).min(GRID_COLS - 1);
            }
            b'D' => {
                self.col = self.col.saturating_sub(get(0));
            }
            b'G' => {
                self.col = get(0).saturating_sub(1).min(GRID_COLS - 1);
            }
            b'd' => {
                self.row = get(0).saturating_sub(1).min(GRID_ROWS - 1);
            }
            b'J' => {
                self.erase_screen(p.first().copied().unwrap_or(0));
            }
            b'K' => {
                self.erase_line(p.first().copied().unwrap_or(0));
            }
            _ => {}
        }
    }

    pub fn as_text(&self) -> String {
        let mut s = String::with_capacity(GRID_COLS * GRID_ROWS + GRID_ROWS);
        for r in 0..GRID_ROWS {
            if r > 0 {
                s.push('\n');
            }
            let row_data = &self.cells[r * GRID_COLS..(r + 1) * GRID_COLS];
            s.push_str(&String::from_utf8_lossy(row_data));
        }
        s
    }
}

fn parse_params(data: &[u8]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut cur = 0;
    let mut has_cur = false;

    for &b in data {
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

struct Session {
    master_fd: i32,
    process_group: libc::pid_t,
}

pub struct TerminalManager {
    session: Mutex<Option<Session>>,
    reader_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            reader_thread: Mutex::new(None),
        }
    }

    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    pub fn spawn(&self, exec: &str, setting_id: i32, weak_window: slint::Weak<MainWindow>) {
        let mut session_lock = self.session.lock().unwrap();
        if session_lock.is_some() {
            eprintln!("terminal already running");
            return;
        }

        let mut master_fd: libc::c_int = -1;
        let mut slave_fd: libc::c_int = -1;
        let mut ws = libc::winsize {
            ws_row: GRID_ROWS as u16,
            ws_col: GRID_COLS as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let res = unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut ws,
            )
        };

        if res < 0 {
            eprintln!("openpty error: errno {}", std::io::Error::last_os_error());
            let weak = weak_window.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_terminal_active(false);
                    w.set_terminal_id(0);
                }
            })
            .ok();
            return;
        }

        let child = unsafe { libc::fork() };
        if child < 0 {
            eprintln!("fork error: errno {}", std::io::Error::last_os_error());
            unsafe {
                libc::close(master_fd);
                libc::close(slave_fd);
            }
            return;
        }

        if child == 0 {
            unsafe {
                libc::close(master_fd);
                libc::setsid();
                libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);
                libc::dup2(slave_fd, libc::STDIN_FILENO);
                libc::dup2(slave_fd, libc::STDOUT_FILENO);
                libc::dup2(slave_fd, libc::STDERR_FILENO);
                if slave_fd > 2 {
                    libc::close(slave_fd);
                }

                let sh = std::ffi::CString::new("sh").unwrap();
                let c = std::ffi::CString::new("-c").unwrap();
                let cmd = std::ffi::CString::new(exec).unwrap();
                libc::execlp(
                    sh.as_ptr(),
                    sh.as_ptr(),
                    c.as_ptr(),
                    cmd.as_ptr(),
                    std::ptr::null::<libc::c_char>(),
                );
                libc::_exit(127);
            }
        }

        unsafe {
            libc::close(slave_fd);
            libc::setpgid(child, child);
        }

        *session_lock = Some(Session {
            master_fd,
            process_group: child,
        });

        let weak_ui = weak_window.clone();
        slint::invoke_from_event_loop(move || {
            if let Some(w) = weak_ui.upgrade() {
                w.set_terminal_active(true);
                w.set_terminal_id(setting_id);
            }
        })
        .ok();

        let weak_reader = weak_window.clone();
        let handle = std::thread::spawn(move || {
            let mut screen = Screen::new();
            let mut buffer = [0u8; 4096];
            let mut last_text = String::new();

            loop {
                let n = unsafe {
                    libc::read(
                        master_fd,
                        buffer.as_mut_ptr() as *mut libc::c_void,
                        buffer.len(),
                    )
                };
                if n <= 0 {
                    break;
                }

                screen.feed(&buffer[..n as usize]);
                let text = screen.as_text();
                if text != last_text {
                    last_text = text.clone();
                    let weak = weak_reader.clone();
                    let text_shared: slint::SharedString = text.into();
                    slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak.upgrade() {
                            w.set_terminal_output(text_shared);
                        }
                    })
                    .ok();
                }
            }

            let mut status: libc::c_int = 0;
            unsafe {
                libc::waitpid(child, &mut status, 0);
            }
            eprintln!("interactive session ended");

            let weak = weak_reader.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_terminal_active(false);
                    w.set_terminal_id(0);
                    w.set_terminal_output("".into());
                }
            })
            .ok();
        });

        *self.reader_thread.lock().unwrap() = Some(handle);
    }

    pub fn write(&self, text: &str) {
        let lock = self.session.lock().unwrap();
        if let Some(ref session) = *lock {
            unsafe {
                libc::write(
                    session.master_fd,
                    text.as_ptr() as *const libc::c_void,
                    text.len(),
                );
                libc::tcdrain(session.master_fd);
            }
        }
    }

    pub fn terminate(&self, weak_window: &slint::Weak<MainWindow>) {
        let session = self.session.lock().unwrap().take();
        if let Some(session) = session {
            eprintln!("terminating interactive program");
            unsafe {
                libc::kill(-session.process_group, libc::SIGTERM);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
            unsafe {
                libc::kill(-session.process_group, libc::SIGKILL);
                libc::close(session.master_fd);
            }
        } else {
            eprintln!("requested to terminate interactive program but none is running");
        }

        let handle = self.reader_thread.lock().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }

        let weak = weak_window.clone();
        slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade() {
                w.set_terminal_active(false);
                w.set_terminal_id(0);
                w.set_terminal_output("".into());
            }
        })
        .ok();
    }

    pub fn terminate_all(&self) {
        let session = self.session.lock().unwrap().take();
        if let Some(session) = session {
            unsafe {
                libc::kill(-session.process_group, libc::SIGKILL);
                libc::close(session.master_fd);
            }
        }

        let handle = self.reader_thread.lock().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_basic() {
        let mut screen = Screen::new();
        screen.feed(b"Hello World");
        let text = screen.as_text();
        assert!(text.starts_with("Hello World"));
    }

    #[test]
    fn test_screen_cursor_and_newlines() {
        let mut screen = Screen::new();
        screen.feed(b"Line 1\r\nLine 2");
        let text = screen.as_text();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("Line 1"));
        assert!(lines[1].starts_with("Line 2"));
    }

    #[test]
    fn test_screen_ansi_cursor() {
        let mut screen = Screen::new();
        // CSI 2;5H -> move to row 2 (index 1), col 5 (index 4)
        screen.feed(b"\x1b[2;5HTest");
        let text = screen.as_text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(&lines[1][4..8], "Test");
    }
}
