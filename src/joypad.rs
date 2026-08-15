use std::sync::Arc;
use std::thread;

use gilrs::{Axis, Button, EventType, Gilrs};
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};

use crate::terminal::TerminalManager;
use crate::MainWindow;

const STICK_THRESHOLD: f32 = 0.5;

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    None,
    Left,
    Right,
    Up,
    Down,
}

pub fn register_joypad(
    weak_window: slint::Weak<MainWindow>,
    terminal: Arc<TerminalManager>,
) {
    thread::spawn(move || {
        let mut gilrs = match Gilrs::new() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("gilrs init error: {e}");
                return;
            }
        };

        let mut stick_x = Dir::None;
        let mut stick_y = Dir::None;
        let mut l2 = false;
        let mut r2 = false;

        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::ButtonPressed(button, _) => match button {
                    Button::LeftTrigger2 => l2 = true,
                    Button::RightTrigger2 => r2 = true,
                    _ => {
                        if let Some(key) = button_to_key(button) {
                            send_key(&weak_window, key, true);
                        }
                    }
                },
                EventType::ButtonReleased(button, _) => match button {
                    Button::LeftTrigger2 => l2 = false,
                    Button::RightTrigger2 => r2 = false,
                    _ => {
                        if let Some(key) = button_to_key(button) {
                            send_key(&weak_window, key, false);
                        }
                    }
                },
                EventType::AxisChanged(axis, value, _) => match axis {
                    Axis::LeftStickX => {
                        let new_dir = dir_from_x(value);
                        handle_stick(&weak_window, &mut stick_x, new_dir);
                    }
                    Axis::LeftStickY => {
                        let new_dir = dir_from_y(value);
                        handle_stick(&weak_window, &mut stick_y, new_dir);
                    }
                    _ => {}
                },
                _ => {}
            }

            // L2+R2 together terminates a running interactive program and
            // returns to the settings list.
            if l2 && r2 && terminal.is_running() {
                terminal.kill(&weak_window);
                l2 = false;
                r2 = false;
            }
        }
    });
}

fn dir_from_x(value: f32) -> Dir {
    if value < -STICK_THRESHOLD {
        Dir::Left
    } else if value > STICK_THRESHOLD {
        Dir::Right
    } else {
        Dir::None
    }
}

fn dir_from_y(value: f32) -> Dir {
    if value < -STICK_THRESHOLD {
        Dir::Up
    } else if value > STICK_THRESHOLD {
        Dir::Down
    } else {
        Dir::None
    }
}

fn handle_stick(weak_window: &slint::Weak<MainWindow>, current: &mut Dir, new: Dir) {
    if *current == new {
        return;
    }

    let (release, press) = match (*current, new) {
        (Dir::Left, Dir::None) => (Some(Key::LeftArrow), None),
        (Dir::Right, Dir::None) => (Some(Key::RightArrow), None),
        (Dir::Up, Dir::None) => (Some(Key::UpArrow), None),
        (Dir::Down, Dir::None) => (Some(Key::DownArrow), None),
        (Dir::Left, Dir::Right) => (Some(Key::LeftArrow), Some(Key::RightArrow)),
        (Dir::Right, Dir::Left) => (Some(Key::RightArrow), Some(Key::LeftArrow)),
        (Dir::Up, Dir::Down) => (Some(Key::UpArrow), Some(Key::DownArrow)),
        (Dir::Down, Dir::Up) => (Some(Key::DownArrow), Some(Key::UpArrow)),
        (Dir::None, dir) => (
            None,
            match dir {
                Dir::Left => Some(Key::LeftArrow),
                Dir::Right => Some(Key::RightArrow),
                Dir::Up => Some(Key::UpArrow),
                Dir::Down => Some(Key::DownArrow),
                _ => None,
            },
        ),
        _ => (None, None),
    };

    if let Some(key) = release {
        send_key(weak_window, key, false);
    }
    if let Some(key) = press {
        send_key(weak_window, key, true);
    }

    *current = new;
}

fn button_to_key(button: Button) -> Option<Key> {
    match button {
        Button::DPadUp => Some(Key::UpArrow),
        Button::DPadDown => Some(Key::DownArrow),
        Button::DPadLeft => Some(Key::LeftArrow),
        Button::DPadRight => Some(Key::RightArrow),
        Button::South => Some(Key::Return),
        Button::East => Some(Key::Backspace),
        Button::Start => Some(Key::Space),
        Button::Select => Some(Key::Menu),
        _ => None,
    }
}

fn send_key(weak_window: &slint::Weak<MainWindow>, key: Key, pressed: bool) {
    let weak_window = weak_window.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(window) = weak_window.upgrade() {
            let event = if pressed {
                WindowEvent::KeyPressed { text: key.into() }
            } else {
                WindowEvent::KeyReleased { text: key.into() }
            };
            window.window().dispatch_event(event);
        }
    })
    .ok();
}
