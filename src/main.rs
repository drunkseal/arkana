mod audio;
mod covers;
mod entries;
mod joypad;
mod launch;
mod settings;
mod terminal;

use std::env;
use std::rc::Rc;
use std::sync::Mutex;

use slint::ModelRc;

slint::include_modules!();

enum Pending {
    Game(String),
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Map a Slint key event string to the byte sequence sent to the pty.
fn terminal_key_to_bytes(text: &str) -> &str {
    match text {
        "ArrowUp" => "\x1b[A",
        "ArrowDown" => "\x1b[B",
        "ArrowRight" => "\x1b[C",
        "ArrowLeft" => "\x1b[D",
        "\r" => "\r",
        "\u{7f}" | "\u{8}" => "\u{7f}",
        other => other,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    let weak_window = main_window.as_weak();
    let terminal_manager = terminal::TerminalManager::new();

    audio::init();

    let terminal_for_shell = terminal_manager.clone();
    let weak_shell = weak_window.clone();
    main_window.on_settings_run_command(move |command_id| {
        if settings::is_ui_only(command_id as u32) {
            return;
        }
        if settings::is_interactive(command_id as u32) {
            if let Some(exec) = settings::find_command(command_id as u32) {
                terminal_for_shell.spawn(exec, command_id, weak_shell.clone());
            }
        } else if let Some(window) = weak_window.upgrade() {
            window.set_settings_command_output(
                settings::execute_command(command_id as u32).into(),
            )
        }
    });

    let terminal_for_keys = terminal_manager.clone();
    main_window.on_terminal_key(move |text| {
        terminal_for_keys.write(terminal_key_to_bytes(text.as_str()));
    });

    let terminal_for_exit = terminal_manager.clone();
    let weak_exit = main_window.as_weak();
    main_window.on_terminal_exit(move || {
        terminal_for_exit.kill(&weak_exit);
    });

    let weak_window = main_window.as_weak();
    settings::register_battery_watcher(weak_window);

    main_window.on_play_navigation_sound(|| audio::play());

    let weak_window = main_window.as_weak();
    joypad::register_joypad(weak_window, terminal_manager.clone());

    let game_dirs: Vec<std::path::PathBuf> = env::var("GAME_DIRS")
        .unwrap_or_default()
        .split(':')
        .map(std::path::PathBuf::from)
        .collect();

    let games = Rc::new(entries::load_games(game_dirs));

    let settings_model = Rc::new(slint::VecModel::from(
        settings::SETTINGS
            .iter()
            .map(|entry| crate::SettingData {
                setting_id: entry.id as i32,
                title: entry.title.into(),
                kind: match entry.kind {
                    settings::SettingKind::Command => 0,
                    settings::SettingKind::Volume => 1,
                    settings::SettingKind::Brightness => 2,
                },
            })
            .collect::<Vec<_>>(),
    ));
    main_window.set_settings_data_array(ModelRc::from(settings_model));

    let games_for_launch = games.clone();
    main_window.on_launch_game(move |index| {
        if let Some(game) = games_for_launch.get(index as usize)
            && !game.exec.is_empty()
        {
            *PENDING.lock().unwrap() = Some(Pending::Game(game.exec.clone()));
            let _ = slint::quit_event_loop();
        }
    });

    let model = covers::CoverState::build_model(&games);
    let covers = covers::CoverState::init(games, model.clone());
    main_window.set_game_data_array(ModelRc::from(model));

    let covers = covers.clone();
    main_window.on_current_game_changed(move |index| {
        covers.set_current(index as usize);
    });

    main_window.run()?;

    // Never let an interactive program outlive the launcher.
    terminal_manager.terminate_all();

    match PENDING.lock().unwrap().take() {
        Some(Pending::Game(exec)) => launch::launch_game(&exec)?,
        None => {}
    }
    Ok(())
}