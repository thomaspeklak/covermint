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

- flexible monitor targeting and discovery, including `auto`, `internal`, `external`, numeric indices, and field-based name matching
- fixed artwork frame sizing and placement controls for presets, per-axis offsets, and symmetric margins
- styling controls for borders, rounded corners, layer selection, and overall artwork opacity
- `none`, `fade`, and `flip` transitions with configurable timing and eased motion
- local caching for remote artwork plus support for `file://` artwork URLs exposed by MPRIS players
- player selection/discovery, configurable polling, and optional paused-state visibility

For the exact flag surface, use the **CLI reference** below.

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
cargo run --release -- --monitor 0 --placement top-left --margin 32
cargo run --release -- --monitor internal --placement top-left --margin 32
cargo run --release -- --monitor HDMI-A-1 --placement center --offset-y -40
cargo run --release -- --monitor HDMI-A-1 --width 520 --height 420 --placement bottom-right --offset-x 64 --offset-y 64
cargo run --release -- --monitor auto --border-width 2 --border-color 'rgba(255,255,255,0.28)' --corner-radius 18 --opacity 0.92
cargo run --release -- --monitor auto --transition fade --transition-ms 220
cargo run --release -- --monitor auto --transition flip --transition-ms 220
cargo run --release -- --monitor auto --player vlc --poll-seconds 2
cargo run --release -- --monitor auto --player auto
cargo run --release -- --monitor auto --show-paused
cargo run --release -- --monitor auto --no-cache
```

## CLI reference

This is the authoritative per-flag reference; the earlier sections stay higher level on purpose.

```text
--monitor auto|internal|external|<index>|#<index>|<name>
                           Pick a monitor by alias, list index (0 or #0), connector, manufacturer, or model substring
--player auto|<name>        MPRIS player name passed to playerctl; auto uses the active/default player
--size <px>                 Shorthand for setting both --width and --height
--width <px>                Artwork width in pixels
--height <px>               Artwork height in pixels
--placement <preset>        One of: top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right
                           Also accepts aliases like tl, tc, tr, cl, cr, bl, bc, br, and middle
--offset-x <px>             Horizontal offset; positive moves inward from the chosen side or away from center
--offset-y <px>             Vertical offset; positive moves inward from the chosen side or away from center
--margin <px>               Shorthand for setting both --offset-x and --offset-y
--border-width <px>         Border width in pixels
--border-color <css-color>  Border color, including alpha-capable values like rgba(...)
--corner-radius <px>        Corner radius in pixels
--opacity <0.0-1.0>         Overall artwork opacity
--transition none|fade|flip Artwork transition style
--transition-ms <n>         Transition duration in milliseconds
--poll-seconds <n>          Refresh interval
--show-paused               Keep the last artwork visible while playback is paused
--no-cache                  Disable remote artwork cache reads and writes
--layer background|bottom   Choose the layer-shell layer
--list-monitors             Print detected monitors and exit
--list-players              Print detected MPRIS player names and exit
--help, -h                  Print usage and exit successfully
```

`auto` prefers an internal monitor and otherwise falls back to the first detected monitor. `external` prefers the first non-internal monitor and otherwise also falls back to the first detected monitor. If an explicit monitor selector cannot be resolved, Covermint lets the compositor choose and logs that fallback. Use `--list-monitors` to see the connector and model/manufacturer strings that matching can target.

## Current limitations

- the app polls instead of reacting to MPRIS signals
- placement follows monitor changes on the polling interval, not instantly via display event subscriptions
- some players, including Spotify, often expose artwork around `640x640`
- artwork is scaled to the configured frame size in both directions; tune it with `--size`, `--width`, and `--height`
- automatic player selection depends on `playerctl`'s active/default player behavior
- paused artwork stays hidden unless `--show-paused` is enabled
- the cache is local-only and uses a lightweight retention policy rather than a configurable eviction system when enabled
- only `http`, `https`, and `file` artwork URLs are supported right now
- `flip` is a GTK-friendly horizontal squeeze / swap effect with subtle spring easing rather than a true 3D compositor transform
- more transitions can be added on top of the transition hook
- more advanced styling controls are still pending beyond border/radius/opacity polish

## Running as a user service

An example systemd user unit is included at:

- `contrib/systemd/covermint.service`

Suggested setup:

```bash
cargo install --path . --root ~/.local
mkdir -p ~/.config/systemd/user
cp contrib/systemd/covermint.service ~/.config/systemd/user/
$EDITOR ~/.config/systemd/user/covermint.service
systemctl --user daemon-reload
systemctl --user enable --now covermint.service
```

The example unit uses `%h/.local/bin/covermint`, which resolves to `~/.local/bin/covermint` for a user service. You will probably want to customize the `ExecStart=` line for your monitor, placement, size, transition settings, and binary path.

## Ticket tracking with Beads

This project uses **Beads** for local ticket tracking.

Useful commands:

```bash
br list
br ready
br show sp-czm
br show <id>
```

The live Beads backlog is the source of truth, so prefer those commands over copying ticket status into the README.

To add more work:

```bash
br create --title "Your feature here" --type feature --priority P2
```
