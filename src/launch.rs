use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Where the PID of the shell waiting to run a launched game is stored.
///
/// Configurable via the `ARKANA_GAME_PIDFILE` environment variable; the OpenRC
/// supervisor exports the same default so it can watch this file to know when
/// to restart the launcher. It must not start a new launcher until the game
/// process has exited, otherwise the fresh launcher would grab the DRM/display
/// device back from the game.
fn game_pid_file() -> PathBuf {
    std::env::var_os("ARKANA_GAME_PIDFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/arkana-game.pid"))
}

/// Launch a game and exit this process.
///
/// The launcher must fully exit so the DRM/display device is released for the
/// game. We spawn a detached shell that waits until the launcher process is
/// gone, then runs the game. The shell's PID is recorded in [`game_pid_file`]
/// so the OpenRC supervisor can postpone restarting the launcher until after
/// the game has finished.
pub fn launch_game(exec: &str) -> Result<(), String> {
    let launcher_pid = std::process::id();

    let script = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done\n{exec}",
        pid = launcher_pid
    );

    let child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;

    std::fs::write(game_pid_file(), child.id().to_string()).ok();
    Ok(())
}