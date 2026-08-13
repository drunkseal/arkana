use std::{process::Output, thread, time::Duration};

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
    let output: Result<Output, std::io::Error> = match command {
        CommandId::Poweroff => std::process::Command::new("poweroff").output(),
        CommandId::Reboot => std::process::Command::new("reboot").output(),
        CommandId::IPInfo => std::process::Command::new("ip").arg("a").output(),
    };

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