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

- monitor selection via `--monitor auto|<name>`
- monitor discovery via `--list-monitors`
- layer selection via `--layer background|bottom`
- square artwork sizing via `--size`
- edge offset via `--margin`
- player selection via `--player`
- configurable polling interval via `--poll-seconds`

## Requirements

### Runtime

- Linux Wayland session
- a compositor with `layer-shell` support
- `playerctl` in `PATH`
- an MPRIS-compatible player exposing `mpris:artUrl`
- network access for the artwork URL

### Build

- recent Rust toolchain
- GTK4 development libraries
- `gtk4-layer-shell` development libraries

Package names vary by distro, so the README intentionally stays generic instead of assuming one specific setup.

## Build and run

```bash
cargo run --release -- --list-monitors
cargo run --release -- --monitor auto
```

Useful examples:

```bash
cargo run --release -- --monitor auto --layer background
cargo run --release -- --monitor eDP-1 --layer bottom
cargo run --release -- --monitor HDMI-A-1 --size 520 --margin 64
cargo run --release -- --monitor auto --player spotify --poll-seconds 2
```

## CLI reference

```text
--monitor auto|<name>       Pick a monitor by connector or matching description
--player <name>             MPRIS player name passed to playerctl
--size <px>                 Square artwork size in pixels
--margin <px>               Distance from the anchored screen edges
--poll-seconds <n>          Refresh interval
--layer background|bottom   Choose the layer-shell layer
--list-monitors             Print detected monitors and exit
```

## Current limitations

- placement is currently fixed to the bottom-right corner
- the app polls instead of reacting to MPRIS signals
- some players, including Spotify, often expose artwork around `640x640`
- there is no artwork cache yet
- there are no transitions, border controls, or advanced styling yet

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

- `sp-czm.1` — remove system-specific assumptions and odd dependencies
- `sp-czm.2` — add custom placement controls
- `sp-czm.3` — add an extensible transition system
- `sp-czm.4` — support borders with transparency
- `sp-czm.5` — improve custom sizing controls
- `sp-czm.6` — write a polished README ✅
- `sp-czm.7` — rename the project to Covermint

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
