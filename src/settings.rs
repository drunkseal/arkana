use std::{process::Output, thread, time::Duration};

use crate::MainWindow;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingKind {
    /// A normal launcher entry: shows its title and runs its command.
    Command,
    /// A UI-only display: the on-screen volume bar.
    Volume,
    /// A UI-only display: the on-screen brightness bar.
    Brightness,
}

#[derive(Debug, Clone, Copy)]
pub struct SettingsEntry {
    pub id: u32,
    pub title: &'static str,
    pub exec: &'static str,
    /// Marks an interactive program (e.g. `evtest`). Instead of capturing its
    /// output and displaying it, it takes over the display (the launcher exits
    /// to release DRM) and is terminated by the L2+R2 combo.
    pub interactive: bool,
    pub kind: SettingKind,
}

/// The single source of truth for settings: the title shown in the UI and the
/// command to run are defined together here.
pub const SETTINGS: &[SettingsEntry] = &[
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

pub fn is_interactive(id: u32) -> bool {
    SETTINGS
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.interactive)
        .unwrap_or(false)
}

/// Whether the entry is a UI-only display with no command to run.
pub fn is_ui_only(id: u32) -> bool {
    SETTINGS
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.kind != SettingKind::Command)
        .unwrap_or(false)
}

pub fn find_command(id: u32) -> Option<&'static str> {
    SETTINGS
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.exec)
}

pub fn execute_command(id: u32) -> String {
    let Some(entry) = SETTINGS.iter().find(|entry| entry.id == id) else {
        eprintln!("unknown settings command id: {id}");
        return String::from("(unknown command)");
    };

    // UI-only views (volume/brightness bars) have no command to run.
    if entry.kind != SettingKind::Command {
        return String::new();
    }

    let output: Result<Output, std::io::Error> = std::process::Command::new("sh")
        .arg("-c")
        .arg(entry.exec)
        .output();

    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).into_owned(),
        Err(err) => {
            eprintln!("failed to run settings command: {err}");
            String::from("(command failed)")
        }
    }
}

pub fn register_battery_watcher(weak_window: slint::Weak<MainWindow>) {
    thread::spawn(move || {
        let battery_manager = match battery::Manager::new() {
            Ok(manager) => manager,
            Err(err) => {
                eprintln!("battery manager init error: {err}");
                return;
            }
        };

        loop {
            let mut found = false;
            match battery_manager.batteries() {
                Ok(batteries) => {
                    for battery in batteries.flatten() {
                        found = true;
                        let percentage = (battery.state_of_charge().value * 100.0) as i32;
                        let weak_window = weak_window.clone();
                        slint::invoke_from_event_loop(move || {
                            if let Some(window) = weak_window.upgrade() {
                                window.set_battery_level(percentage);
                            }
                        })
                        .ok();
                    }
                }
                Err(err) => eprintln!("battery enumeration error: {err}"),
            }

            if !found {
                // No battery present; there is nothing to watch.
                return;
            }

            thread::sleep(Duration::from_secs(10));
        }
    });
}