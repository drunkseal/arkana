use std::process::{Command, Stdio};

/// Path where the PID of the shell waiting to run a launched game is stored.
///
/// The OpenRC supervisor reads this to know when to restart the launcher: it
/// must not start a new launcher until the game process has exited, otherwise
/// the fresh launcher would grab the DRM/display device back from the game.
const GAME_PID_FILE: &str = "/run/arkana-game.pid";

/// Launch a game and exit this process.
///
/// The launcher must fully exit so the DRM/display device is released for the
/// game. We spawn a detached shell that waits until the launcher process is
/// gone, then runs the game. The shell's PID is recorded in [`GAME_PID_FILE`]
/// so the OpenRC supervisor can postpone restarting the launcher until after
/// the game has finished.
pub fn launch_game(exec: &str) -> Result<(), String> {
    let launcher_pid = std::process::id();

    let script = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.1; done\n{exec}",
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

    std::fs::write(GAME_PID_FILE, child.id().to_string()).ok();
    Ok(())
}