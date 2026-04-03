# Covermint

`covermint` shows the current media cover art as a wallpaper-adjacent **Wayland layer-shell surface**.

Instead of editing your wallpaper file, it opens a small GTK window on the `background` or `bottom` layer so the artwork feels pinned to the desktop and stays behind normal app windows.

## Status

This repo is still an early spike, but it already works for the basic flow:

- polls `playerctl` for playback status and `mpris:artUrl`
- downloads the current artwork
- renders it with GTK4
- places it on a selected monitor
- keeps it behind normal windows via `gtk4-layer-shell`
- hides itself when nothing useful is playing

## Current features

- monitor selection via `--monitor auto|internal|external|<name>`
- monitor discovery via `--list-monitors`
- player discovery via `--list-players`
- layer selection via `--layer background|bottom`
- sizing via `--size`, `--width`, and `--height`
- placement presets via `--placement`
- per-axis offsets via `--offset-x` / `--offset-y`
- `--margin` shorthand for matching X/Y offsets
- translucent border styling via `--border-width`, `--border-color`, and `--corner-radius`
- artwork transitions via `--transition` and `--transition-ms`, with eased timing
- local artwork caching for repeated remote URLs, with `--no-cache` support when desired
- support for `file://` artwork URLs exposed by MPRIS players
- player selection via `--player` (defaults to `auto`)
- configurable polling interval via `--poll-seconds`
- optional paused-state visibility via `--show-paused`

## Requirements

### Runtime

- Linux Wayland session
- a compositor with `layer-shell` support
- `playerctl` in `PATH`
- an MPRIS-compatible player exposing `mpris:artUrl`
- network access for remote artwork URLs

### Build

- recent Rust toolchain
- GTK 4.8+ development libraries
- `gtk4-layer-shell` development libraries

Package names vary by distro, so the README intentionally stays generic instead of assuming one specific setup.

## Build and run

```bash
cargo run --release -- --list-monitors
cargo run --release -- --list-players
cargo run --release -- --monitor auto
```

Useful examples:

```bash
cargo run --release -- --monitor auto --layer background
cargo run --release -- --monitor internal --placement top-left --margin 32
cargo run --release -- --monitor HDMI-A-1 --placement center --offset-y -40
cargo run --release -- --monitor HDMI-A-1 --width 520 --height 420 --placement bottom-right --offset-x 64 --offset-y 64
cargo run --release -- --monitor auto --border-width 2 --border-color 'rgba(255,255,255,0.28)' --corner-radius 18
cargo run --release -- --monitor auto --transition fade --transition-ms 220
cargo run --release -- --monitor auto --transition flip --transition-ms 220
cargo run --release -- --monitor auto --player spotify --poll-seconds 2
cargo run --release -- --monitor auto --player auto
cargo run --release -- --monitor auto --show-paused
cargo run --release -- --monitor auto --no-cache
```

## CLI reference

```text
--monitor auto|internal|external|<name>
                           Pick a monitor by alias, connector, or matching description
--player auto|<name>        MPRIS player name passed to playerctl; auto uses the active/default player
--size <px>                 Shorthand for setting both --width and --height
--width <px>                Artwork width in pixels
--height <px>               Artwork height in pixels
--placement <preset>        One of: top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right
--offset-x <px>             Horizontal offset; positive moves inward from the chosen side or away from center
--offset-y <px>             Vertical offset; positive moves inward from the chosen side or away from center
--margin <px>               Shorthand for setting both --offset-x and --offset-y
--border-width <px>         Border width in pixels
--border-color <css-color>  Border color, including alpha-capable values like rgba(...)
--corner-radius <px>        Corner radius in pixels
--transition none|fade|flip Artwork transition style
--transition-ms <n>         Transition duration in milliseconds
--poll-seconds <n>          Refresh interval
--show-paused               Keep the last artwork visible while playback is paused
--no-cache                  Disable remote artwork cache reads and writes
--layer background|bottom   Choose the layer-shell layer
--list-monitors             Print detected monitors and exit
--list-players              Print detected MPRIS player names and exit
```

## Current limitations

- the app polls instead of reacting to MPRIS signals
- placement follows monitor changes on the polling interval, not instantly via display event subscriptions
- some players, including Spotify, often expose artwork around `640x640`
- automatic player selection depends on `playerctl`'s active/default player behavior
- paused artwork stays hidden unless `--show-paused` is enabled
- the cache is local-only and uses a lightweight retention policy rather than a configurable eviction system when enabled
- only `http`, `https`, and `file` artwork URLs are supported right now
- `flip` is a GTK-friendly horizontal squeeze / swap effect with subtle spring easing rather than a true 3D compositor transform
- more transitions can be added on top of the transition hook
- more advanced styling controls are still pending beyond border/radius polish

## Running as a user service

An example systemd user unit is included at:

- `contrib/systemd/covermint.service`

Suggested setup:

```bash
mkdir -p ~/.config/systemd/user
cp contrib/systemd/covermint.service ~/.config/systemd/user/
$EDITOR ~/.config/systemd/user/covermint.service
systemctl --user daemon-reload
systemctl --user enable --now covermint.service
```

You will probably want to customize the `ExecStart=` line for your monitor, placement, size, transition settings, and binary path.

## Ticket tracking with Beads

This project now uses **Beads** for local ticket tracking.

Useful commands:

```bash
br list
br ready
br show sp-czm
br show sp-czm.2
```

Seeded tickets:

- `sp-czm.1` — remove system-specific assumptions and odd dependencies ✅
- `sp-czm.2` — add custom placement controls ✅
- `sp-czm.3` — add an extensible transition system ✅
- `sp-czm.4` — support borders with transparency ✅
- `sp-czm.5` — improve custom sizing controls ✅
- `sp-czm.6` — write a polished README ✅
- `sp-czm.7` — rename the project to Covermint
- `sp-czm.8` — add flip transition mode ✅
- `sp-czm.11` — cache artwork locally ✅
- `sp-czm.12` — add example user service ✅
- `sp-czm.13` — trim artwork cache ✅
- `sp-czm.15` — support file:// artwork URLs ✅
- `sp-czm.16` — add internal/external monitor aliases ✅
- `sp-czm.17` — optionally keep artwork visible while paused ✅
- `sp-czm.18` — add configurable corner radius ✅
- `sp-czm.19` — add player discovery command ✅
- `sp-czm.20` — allow disabling remote artwork cache ✅

To add more work:

```bash
br create --title "Your feature here" --type feature --priority P2
```

## Near-term roadmap

The next round of improvements is focused on making `covermint` feel less like a machine-specific spike and more like a configurable desktop widget:

1. rename cleanup
2. portability cleanup
3. placement controls
4. small but extensible transitions
5. border and transparency styling
6. better sizing UX
7. continued documentation cleanup
