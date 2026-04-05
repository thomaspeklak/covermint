# Covermint 3D-ish transition notes

## Goal

Push Covermint past simple crossfades without committing to a full GL rendering path too early.

## What ships now

Covermint now has two GTK-friendly pseudo-3D transitions:

- `flip` — horizontal squeeze / swap with restrained spring easing
- `hinge` — a top-anchored card-fold feel using width + height compression and a springy settle

Both work inside the existing GTK picture stage, so they keep the current compatibility profile and do not require a GL scene graph.

## Why not jump straight to GL

For the current app shape, a GL-specific path is not yet justified:

- the widget is small and static most of the time
- startup and compatibility matter more than maximum visual fidelity
- the existing GTK layer-shell path remains simpler to package and reason about
- pseudo-3D motion already adds useful continuity without introducing a second renderer

## When a GL path *would* be justified

A dedicated GL transition layer becomes more attractive if Covermint wants:

- true perspective rotation
- dynamic lighting / parallax
- depth-based shadows
- shader-driven distortion
- more cinematic transition choreography across larger artwork surfaces

At that point, the likely shape is:

1. keep `fade`, `flip`, and `hinge` as the compatibility path
2. add an optional GL-backed transition mode for richer hardware-accelerated motion
3. fall back automatically when the GL path is unavailable or undesirable

## Reduced-motion / compatibility fallback

For users or environments that want less motion:

- use `--transition fade` for the softest motion
- use `--transition none` to remove motion entirely

Those modes remain the baseline fallback if future higher-fidelity transitions become more ambitious.
