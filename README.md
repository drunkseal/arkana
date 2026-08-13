# Arkana

A minimal, keyboard- and gamepad-driven launcher for a handheld Linux gaming
device. It runs on bare DRM/KMS with no compositor, so it can hand the display
straight to a game and take it back afterwards.

Built with [Slint](https://slint.dev), it shows your installed games as a
cover-art carousel, launches them on demand, and returns to the menu when the
game exits.

![Languages](https://img.shields.io/badge/language-Rust-orange)
![Platform](https://img.shields.io/badge/platform-Linux%20(KMS)%20-%23800)

## Features

- **Cover-art carousel** — navigate your library left/right with a circular
  winding window. Covers are decoded on a background thread and downscaled, so
  navigation never stutters even with large libraries.
- **Settings drawer** — toggle with a button press to reach Power Off, Reboot,
  and IP Info.
- **Gamepad support** — full D-pad / analog-stick + button mapping via `gilrs`,
  translated into synthetic key events.
- **DRM handoff** — launching a game lets the launcher process exit so the game
  gets exclusive access to the display, then a supervised restart brings the
  menu back after the game finishes.
- **OpenRC integration** — a ready-made init script keeps the launcher alive
  and defers its restart until the launched game has exited.

## Requirements

- Linux (the KMS backend has no Windows/macOS support)
- A system that boots to a console/DRM as above — no X11 or Wayland compositor
  required. A Wayland backend is also enabled for development.
- Rust with a recent stable toolchain. `slint` is pulled from git, so you need
  `git` available at build time.

The default `cargo build` works for development on a Wayland desktop. For the
target device (e.g. an ARM handheld), build with `cross` — see
[Cross.toml](Cross.toml) for the configured targets.

## Building

```sh
cargo build --release
```

For an ARM/other device using `cross`:

```sh
cross build --release --target aarch64-unknown-linux-gnu
```

## Configuration

Games are discovered from a **list of directories** given in the `GAME_DIRS`
environment variable (colon-separated). Each directory is scanned for `.toml`
files, one per game:

```toml
name = "Sonic Adventure"
exec = "sonic-adventure"
cover = "/games/sonic/cover.png"
```

- `name` — display title shown under the cover.
- `exec` — the command to launch. It runs through a shell, so arguments (and
  shell syntax) are allowed.
- `cover` — optional path to an image (PNG/JPEG). Without one, an empty
  placeholder cover is shown.

Example:

```sh
GAME_DIRS=/games:/opt/arkana/roms arkana
```

If no directories or games are found, the launcher shows a fallback entry.

## Controls

| Action                  | Keyboard     | Gamepad                    |
| ----------------------- | ------------ | -------------------------- |
| Navigate games          | ← / →        | D-Pad or left stick        |
| Open/close settings     | `Space`      | Start / Select             |
| Launch selected game    | `Return`     | A (South)                  |
| Run settings command    | `Return`     | A (South)                  |

In the settings screen (IP Info), `↑`/`↓` scroll the output.

## Running as a service

Arkana is designed to be the only thing on screen, supervised forever. Install
the binary, then use the provided OpenRC service:

```sh
cp target/<triple>/release/arkana /usr/bin/arkana
cp packaging/arkana.initd /etc/init.d/arkana
chmod +x /etc/init.d/arkana
rc-update add arkana default
rc-service arkana start
```

See [packaging/README.md](packaging/README.md) for details on the DRM handoff
and how the service coordinates with a launched game.

## Repository layout

```
src/
  main.rs        Bootstrap + UI callbacks
  covers.rs      Cover decoding, caching, and the winding window
  entries.rs     Game discovery and .toml parsing
  joypad.rs      Gamepad → synthetic key events
  launch.rs      Game launch + DRM handoff
  settings.rs    Settings commands and battery monitoring
ui/              Slint UI (main window, slide view, game/settings views)
packaging/       OpenRC init script
```