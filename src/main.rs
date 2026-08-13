mod covers;
mod entries;
mod joypad;
mod launch;
mod settings;

use std::env;
use std::rc::Rc;
use std::sync::Mutex;

use slint::ModelRc;

slint::include_modules!();

static PENDING_LAUNCH: Mutex<Option<String>> = Mutex::new(None);

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    let weak_window = main_window.as_weak();
    main_window.on_settings_run_command(move |command_id| {
        if let Some(window) = weak_window.upgrade() {
            window.set_settings_command_output(
                settings::execute_command((command_id as u32).into()).into(),
            )
        }
    });

    let weak_window = main_window.as_weak();
    settings::register_battery_watcher(weak_window);

    let weak_window = main_window.as_weak();
    joypad::register_joypad(weak_window);

    let game_dirs: Vec<std::path::PathBuf> = env::var("GAME_DIRS")
        .unwrap_or_default()
        .split(':')
        .map(std::path::PathBuf::from)
        .collect();

    let games = Rc::new(entries::load_games(game_dirs));

    let games_for_launch = games.clone();
    main_window.on_launch_game(move |index| {
        if let Some(game) = games_for_launch.get(index as usize)
            && !game.exec.is_empty()
        {
            *PENDING_LAUNCH.lock().unwrap() = Some(game.exec.clone());
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

    if let Some(exec) = PENDING_LAUNCH.lock().unwrap().take() {
        launch::launch_game(&exec)?;
    }
    Ok(())
}