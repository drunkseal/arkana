use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SettingKind {
    Command = 0,
    Volume = 1,
    Brightness = 2,
}

pub struct SettingsEntry {
    pub id: u32,
    pub title: &'static str,
    pub exec: &'static str,
    pub interactive: bool,
    pub kind: SettingKind,
}

pub fn get_settings() -> &'static [SettingsEntry] {
    static SETTINGS: &[SettingsEntry] = &[
        SettingsEntry {
            id: 1,
            title: "Power Off",
            exec: "poweroff",
            interactive: false,
            kind: SettingKind::Command,
        },
        SettingsEntry {
            id: 2,
            title: "Reboot",
            exec: "reboot",
            interactive: false,
            kind: SettingKind::Command,
        },
        SettingsEntry {
            id: 3,
            title: "IP Info",
            exec: "ip a",
            interactive: false,
            kind: SettingKind::Command,
        },
        SettingsEntry {
            id: 4,
            title: "Input Test",
            exec: "evtest",
            interactive: true,
            kind: SettingKind::Command,
        },
        SettingsEntry {
            id: 5,
            title: "Volume",
            exec: "",
            interactive: false,
            kind: SettingKind::Volume,
        },
        SettingsEntry {
            id: 6,
            title: "Brightness",
            exec: "",
            interactive: false,
            kind: SettingKind::Brightness,
        },
    ];
    SETTINGS
}

pub fn is_interactive(id: u32) -> bool {
    get_settings()
        .iter()
        .find(|s| s.id == id)
        .map_or(false, |s| s.interactive)
}

pub fn is_ui_only(id: u32) -> bool {
    get_settings()
        .iter()
        .find(|s| s.id == id)
        .map_or(false, |s| s.kind != SettingKind::Command)
}

pub fn find_command(id: u32) -> Option<&'static str> {
    get_settings()
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.exec)
        .filter(|e| !e.is_empty())
}

pub fn execute_command(id: u32) -> String {
    let Some(entry) = get_settings().iter().find(|s| s.id == id) else {
        return String::new();
    };

    if entry.kind != SettingKind::Command || entry.exec.is_empty() {
        return String::new();
    }

    let cmd_str = format!("{} 2>&1", entry.exec);
    let output = Command::new("sh").args(["-c", &cmd_str]).output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => "(command failed)".to_string(),
    }
}

fn find_battery_capacity_path() -> Option<PathBuf> {
    let read_dir = fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in read_dir.flatten() {
        let cap = entry.path().join("capacity");
        if cap.is_file() {
            return Some(cap);
        }
    }
    None
}

pub struct BatteryWatcher {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BatteryWatcher {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }

    pub fn start<F>(&mut self, callback: F)
    where
        F: Fn(i32) + Send + 'static,
    {
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        self.thread = Some(std::thread::spawn(move || {
            let Some(capacity_path) = find_battery_capacity_path() else {
                return;
            };

            while running.load(Ordering::Relaxed) {
                if let Ok(content) = fs::read_to_string(&capacity_path) {
                    if let Ok(capacity) = content.trim().parse::<i32>() {
                        if (0..=100).contains(&capacity) {
                            callback(capacity);
                        }
                    }
                }

                for _ in 0..100 {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for BatteryWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
