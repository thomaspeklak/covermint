# Changelog

## [v0.3.0] — 2026-04-10

### Features
- Add configurable live lyrics panel with runtime toggle, caching, and layouts (544b946)

### Bug Fixes
- fix: stabilize lyrics layout behavior and satisfy clippy (9a9d129)

## [v0.2.0] — 2026-04-08

### Features
- feat(metadata): add playback position template fields (6db19ae)
- feat(ui): add CRT-style splash glitch and power-off shutdown (7a5f0fc)
- Refactor app modules and add signal-driven MPRIS backend (f373095)

### Bug Fixes
- fix(ui): smooth position ticks without retriggering metadata animation (828c48b)

### Chores / Other
- refactor(ui): extract playback clock and timestamp helpers (084961a)
- perf(build): shrink binary via release profile and ureq (b9f6907)

## [0.1.1] — 2026-04-07

### Bug Fixes
- fix(ci): build `gtk4-layer-shell` from source when `libgtk4-layer-shell-dev` is unavailable on Ubuntu 24.04 runners

### Chores / Other
- chore: bump crate version to 0.1.1

## [v0.1.0] — 2026-04-07

### Features
- Add full config.toml support and stabilize metadata overlay rendering (0a4d168)
- feat: add edge-anchored cover transition (b7be579)
- feat: add edge-anchored slide transition (7c86b78)
- feat: add hinge transition prototype (46b60d9)
- feat: add startup splash screen (6e8b063)
- feat: add bounded artwork cache controls (93d8368)
- feat: add covermint logo asset (60c51f4)
- feat: allow monitor selection by index (3e836bf)
- feat: add artwork opacity control (3478a8b)
- feat: add remote artwork cache toggle (ff8e719)
- feat: add player listing command (ebae317)
- feat: add rounded artwork corners (70317b8)
- feat: add paused artwork visibility option (4a0a97f)
- feat: add monitor selection aliases (47aed58)
- feat: support local artwork file urls (c18bef8)
- feat: trim artwork cache (f83b48b)
- feat: cache artwork locally (974b18f)
- feat: refresh placement during polling (c4f1655)
- feat: add eased transition timing (eda4458)
- feat: add flip artwork transition (395c952)
- feat: add fade artwork transitions (6dbd641)
- feat: make player selection portable by default (ef970fe)
- feat: add configurable artwork borders (a8004a4)
- feat: add width and height sizing (da29c3c)
- feat: add placement controls (8ffea9d)

### Bug Fixes
- fix: stabilize metadata overlay layout and artwork fit controls (bcc13c3)
- fix: satisfy clippy large-enum-variant in StartupAction (772431e)
- fix: recover after wallpaper background restacks (6aa8bb1)
- fix: scale splash logo within the artwork panel (c147d30)
- fix: keep the startup splash above first artwork briefly (32c2553)
- fix: prefer active players for auto selection (31248bb)
- fix: make startup splash a one-shot placeholder (aa143a4)
- fix: polish rounded artwork corners (8e0eee2)
- fix: constrain artwork to the configured frame (f753fed)
- fix: keep artwork window size stable (73b7e34)
- fix: avoid moving transition source into closure (8077b0a)
- fix: use gtk box append for artwork container (68c823e)

### Chores / Other
- chore: add pi release prompt and release workflow (f2ecad3)
- docs: refresh splash and renderer troubleshooting notes (cabb4eb)
- chore: close completed roadmap beads (5a04747)
- refactor: distill artwork and placement helpers (4414915)
- refactor: distill cli and monitor helpers (0de1c64)
- docs: polish example systemd unit (884f13f)
- docs: add example systemd user service (a9874c8)
- refactor: distill transition and playerctl helpers (213724e)
- chore: bootstrap covermint (dcd7d4c)
