use std::fs;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub fn game_pid_file() -> String {
    std::env::var("ARKANA_GAME_PIDFILE").unwrap_or_else(|_| "/run/arkana-game.pid".to_string())
}

pub fn launch_game(exec: &str) -> bool {
    let launcher_pid = std::process::id();
    let script = format!("while kill -0 {launcher_pid} 2>/dev/null; do sleep 0.2; done\n{exec}");

    let devnull_in = fs::File::open("/dev/null").ok();
    let devnull_out = fs::OpenOptions::new().write(true).open("/dev/null").ok();
    let devnull_err = fs::OpenOptions::new().write(true).open("/dev/null").ok();

    let mut cmd = Command::new("sh");
    cmd.args(["-c", &script]);

    if let Some(f) = devnull_in {
        cmd.stdin(Stdio::from(f));
    }
    if let Some(f) = devnull_out {
        cmd.stdout(Stdio::from(f));
    }
    if let Some(f) = devnull_err {
        cmd.stderr(Stdio::from(f));
    }

    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    match cmd.spawn() {
        Ok(child) => {
            let pid = child.id();
            let pidfile = game_pid_file();
            let _ = fs::write(pidfile, pid.to_string());
            true
        }
        Err(err) => {
            eprintln!("launch_game: spawn failed: {err}");
            false
        }
    }
}
