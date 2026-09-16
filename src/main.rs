use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use slint::ComponentHandle;

mod audio;
mod covers;
mod entries;
mod launch;
mod settings;
mod terminal;

slint::include_modules!();

fn terminal_key_to_bytes(text: &str) -> &str {
    match text {
        "ArrowUp" => "\x1b[A",
        "ArrowDown" => "\x1b[B",
        "ArrowRight" => "\x1b[C",
        "ArrowLeft" => "\x1b[D",
        "\r" => "\r",
        "\x7f" | "\x08" => "\x7f",
        _ => text,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let terminal_manager = Arc::new(terminal::TerminalManager::new());

    audio::audio_init();

    let weak_window = main_window.as_weak();

    // Wire settings commands
    {
        let tm = terminal_manager.clone();
        let weak = weak_window.clone();
        main_window.on_settings_run_command(move |command_id| {
            let id = command_id as u32;
            if settings::is_ui_only(id) {
                return;
            }

            if settings::is_interactive(id) {
                if let Some(cmd) = settings::find_command(id) {
                    tm.spawn(cmd, command_id, weak.clone());
                }
            } else if let Some(w) = weak.upgrade() {
                let output = settings::execute_command(id);
                w.set_settings_command_output(output.into());
            }
        });
    }

    // Wire terminal input
    {
        let tm = terminal_manager.clone();
        main_window.on_terminal_key(move |text| {
            let bytes = terminal_key_to_bytes(text.as_str());
            tm.write(bytes);
        });
    }

    // Wire terminal exit
    {
        let tm = terminal_manager.clone();
        let weak = weak_window.clone();
        main_window.on_terminal_exit(move || {
            tm.terminate(&weak);
        });
    }

    // Wire navigation audio
    main_window.on_play_navigation_sound(|| {
        audio::audio_play();
    });

    // Battery monitoring
    let mut battery_watcher = settings::BatteryWatcher::new();
    {
        let weak = weak_window.clone();
        battery_watcher.start(move |level| {
            let weak = weak.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(w) = weak.upgrade() {
                    w.set_battery_level(level);
                }
            })
            .ok();
        });
    }

    // Game library discovery
    let mut game_dirs = Vec::new();
    if let Ok(env) = std::env::var("GAME_DIRS") {
        for dir in env.split(':') {
            if !dir.is_empty() {
                game_dirs.push(PathBuf::from(dir));
            }
        }
    }
    let games = entries::load_games(&game_dirs);

    // Populate settings list
    let settings_list = settings::get_settings();
    let settings_data: Vec<SettingData> = settings_list
        .iter()
        .map(|s| SettingData {
            setting_id: s.id as i32,
            title: s.title.into(),
            kind: s.kind as i32,
        })
        .collect();
    let settings_model = Rc::new(slint::VecModel::from(settings_data));
    main_window.set_settings_data_array(settings_model.into());

    // Game launch callback
    let pending_game = Arc::new(Mutex::new(None));
    {
        let games_for_launch = games.clone();
        let pending = pending_game.clone();
        main_window.on_launch_game(move |index| {
            if index >= 0 && (index as usize) < games_for_launch.len() {
                let game = &games_for_launch[index as usize];
                if !game.exec.is_empty() {
                    *pending.lock().unwrap() = Some(game.exec.clone());
                    slint::quit_event_loop().ok();
                }
            }
        });
    }

    // Cover art model and state
    let cover_model = covers::CoverState::build_model(&games);
    let cover_state = covers::CoverState::init(games, cover_model.clone());
    main_window.set_game_data_array(cover_model.into());

    {
        let state_for_change = cover_state.clone();
        main_window.on_current_game_changed(move |index| {
            if index >= 0 {
                state_for_change.borrow_mut().set_current(index as usize);
            }
        });
    }

    // Run the UI
    let run_res = main_window.run();

    // Shutdown cleanup
    terminal_manager.terminate_all();
    battery_watcher.stop();

    // If a game launch was requested, hand off to it
    let pending = pending_game.lock().unwrap().take();
    if let Some(exec) = pending {
        launch::launch_game(&exec);
    }

    run_res
}
