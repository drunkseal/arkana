# Arkana

A minimal, keyboard- and gamepad-driven launcher for a handheld Linux gaming
device. It ships with two Slint backends: bare **DRM/KMS**, so on the handheld
it acts as the only UI and can hand the display straight to a game and take it
back afterwards, and **Wayland**, so the same binary also runs as a regular
app under a compositor.

Built with [Slint](https://slint.dev) (C++ API), it shows your installed games
as a cover-art carousel, launches them on demand, and returns to the menu when
the game exits.

Currently the UI layout is fixed at **640x480**, so for now Arkana targets
**R36S** handhelds. Making the resolution configurable is planned.

![Languages](https://img.shields.io/badge/language-C%2B%2B-blue)
![Build](https://img.shields.io/badge/build-meson-green)
![Platform](https://img.shields.io/badge/platform-Linux%20(KMS%20%2F%20Wayland)%20-orange)

## Features

- **Cover-art carousel** -- navigate your library left/right with a circular
  winding window. Covers are decoded on a background thread and downscaled, so
  navigation never stutters even with large libraries.
- **Settings drawer** -- toggle with a button press to reach Power Off, Reboot,
  and IP Info.
- **Gamepad support** -- full D-pad / analog-stick + button mapping via evdev,
  translated into synthetic key events.
- **DRM handoff** -- launching a game lets the launcher process exit so the game
  gets exclusive access to the display, then a supervised restart brings the
  menu back after the game finishes.
- **OpenRC integration** -- a ready-made init script keeps the launcher alive
  and defers its restart until the launched game has exited.
- **Terminal support** -- interactive terminal widget for running commands
  directly from the settings drawer.
- **Audio** -- optional ALSA-based navigation sounds.

## Requirements

- Linux (neither backend supports Windows/macOS)
- Either bare DRM/KMS output (typical on the handheld) or a Wayland compositor
- A C++23 compiler (GCC 14+ or Clang 17+)
- [Meson](https://mesonbuild.com/) build system
- [Ninja](https://ninja-build.org/) backend
- [CMake](https://cmake.org/) (for building the Slint C++ library)
- [Rust](https://www.rust-lang.org/) toolchain (for the Slint compiler)
- `pkg-config`

### Build dependencies (Ubuntu/Debian)

```sh
sudo apt install build-essential meson ninja-build cmake pkg-config \
  libfontconfig-dev libfreetype-dev libdrm-dev libgbm-dev libseat-dev \
  libinput-dev libxkbcommon-dev libwayland-dev libasound2-dev linux-headers-generic
```

### Build dependencies (Alpine)

```sh
sudo apk add build-base meson ninja cmake pkgconf \
  fontconfig-dev freetype-dev libdrm-dev mesa-dev libseat-dev \
  libinput-dev libxkbcommon-dev wayland-dev alsa-lib-dev linux-headers
```

## Building

```sh
meson setup builddir
meson compile -C builddir
```

The binary will be at `builddir/arkana`.

Meson handles building the Slint C++ library (via `cmake.subproject`) and
the Slint compiler (via Cargo) automatically. A Rust toolchain is required.

### Build options

- `-Dalsa=true/false` -- enable/disable ALSA navigation sounds (default: true)

```sh
meson setup builddir -Dalsa=false
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

- `name` -- display title shown under the cover.
- `exec` -- the command to launch. It runs through a shell, so arguments (and
  shell syntax) are allowed.
- `cover` -- optional path to an image (PNG/JPEG). Without one, an empty
  placeholder cover is shown.

Example:

```sh
GAME_DIRS=/games:/opt/arkana/roms arkana
```

If no directories or games are found, the launcher shows a fallback entry.

## Controls

| Action                  | Keyboard     | Gamepad                    |
| ----------------------- | ------------ | -------------------------- |
| Navigate games          | Left / Right | D-Pad or left stick        |
| Open/close settings     | `Space`      | Start / Select             |
| Launch selected game    | `Return`     | A (South)                  |
| Run settings command    | `Return`     | A (South)                  |

In the settings screen (IP Info), Up/Down scroll the output.

## Running as a service

Arkana is designed to be the only thing on screen, supervised forever. Install
the binary, then use the provided OpenRC service:

```sh
cp builddir/src/arkana /usr/bin/arkana
cp openrc/arkana.service /etc/init.d/arkana
chmod +x /etc/init.d/arkana
rc-update add arkana default
rc-service arkana start
```

See [openrc/README.md](openrc/README.md) for details on the DRM handoff and
how the service coordinates with a launched game.

## Repository layout

```
src/
  main.cpp          Bootstrap + UI callbacks
  covers.cpp/h      Cover decoding, caching, and the winding window
  entries.cpp/h     Game discovery and .toml parsing
  joypad.cpp/h      Gamepad -> synthetic key events
  launch.cpp/h      Game launch + DRM handoff
  settings.cpp/h    Settings commands and battery monitoring
  terminal.cpp/h    Interactive terminal widget
  audio.cpp/h       ALSA navigation sounds
ui/                 Slint UI definitions (main window, slide view, game/settings views)
openrc/             OpenRC service and packaging docs
assets/             Static assets (sounds, images)
subprojects/        Vendored dependencies (slint source)
```

## CI/CD

The [GitHub Actions workflow](.github/workflows/build-aarch64.yml) builds
release binaries for both **aarch64-gnu** and **aarch64-musl** targets.
Trigger it manually from the Actions tab. Binaries are published as a
GitHub Release tagged with the version from `meson.build`.
