# Arkana OpenRC packaging

## Files

- `arkana.initd` — OpenRC service that keeps the launcher running at all
  times, restarting it after a launched game exits.

## Why this exists

The launcher owns the DRM/display device while it is alive. Launching a game
makes the launcher process **exit** (releasing the device) and spawns a
detached shell that waits for the launcher to be fully gone, then runs the
game. This service supervises the launcher: whenever it exits it waits for
the launched game to finish before starting a fresh launcher, so the running
game keeps exclusive access to the display.

## Install

```sh
# Install the launcher binary
cp target/<triple>/release/arkana /usr/bin/arkana

# Install the init script and enable it
cp packaging/arkana.initd /etc/init.d/arkana
chmod +x /etc/init.d/arkana
rc-update add arkana default
rc-service arkana start
```

## Configuration

- `ARKANA_BIN` — path to the launcher binary (default `/usr/bin/arkana`).
- `ARKANA_GAME_PIDFILE` — where the launcher records the PID of the shell
  waiting to run the game (default `/run/arkana-game.pid`). The init script
  uses the same default, so no configuration is normally required.
