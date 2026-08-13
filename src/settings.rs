use std::{process::Output, thread};

use crate::MainWindow;

#[derive(Debug, Clone, Copy)]
pub enum CommandId {
    Poweroff = 1,
    Reboot = 2,
    IPInfo = 3,
}

impl From<u32> for CommandId {
    fn from(value: u32) -> Self {
        match value {
            1 => CommandId::Poweroff,
            2 => CommandId::Reboot,
            3 => CommandId::IPInfo,
            _ => panic!("Invalid command id"),
        }
    }
}

pub fn execute_command(command: CommandId) -> String {
    let output: Output = match command {
        CommandId::Poweroff => std::process::Command::new("poweroff").output().unwrap(),
        CommandId::Reboot => std::process::Command::new("reboot").output().unwrap(),
        CommandId::IPInfo => std::process::Command::new("ip").arg("a").output().unwrap(),
    };
    String::from_utf8(output.stdout).unwrap()
}

pub fn register_battery_watcher(weak_window: slint::Weak<MainWindow>) {
    thread::spawn(move || -> Result<(), battery::Error> {
        let battery_manager = battery::Manager::new().unwrap();
        loop {
            if let Some(Ok(battery)) = battery_manager.batteries()?.next() {
                let percentage = (battery.state_of_charge().value * 100.0) as i32;
                let weak_window = weak_window.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak_window.upgrade() {
                        window.set_battery_level(percentage);
                    }
                })
                .unwrap();
                thread::sleep(std::time::Duration::from_secs(10));
            }
        }
    });
}
