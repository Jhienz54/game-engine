# myengine

A small, explicit 2D game engine in Rust. Not a Bevy clone: no plugin
system, no ECS core — a scene arena of nodes with generational IDs,
an explicit game loop, and a small API surface.

## Status

v0.1 in progress. See the milestone roadmap in `docs/roadmap.md`
(once it exists) or in the milestone commit messages.

## Structure

- `crates/engine-core` — pure data: scene arena, transforms, collisions, input state, texture handles. No OS, no GPU.
- `crates/engine-platform` — winit window + input.
- `crates/engine-render` — wgpu 2D renderer.
- `crates/engine-assets` — file I/O + image decoding.
- `crates/engine-audio` — audio interface (stub in v0.1).
- `crates/engine` — facade crate. Games depend on this one.
- `games/fear-cards` — playable demo.

## Running the demo

    cargo run -p topdown-demo

## Environment

Tested on Lubuntu with Mesa lavapipe (software Vulkan). Expect
playable (30–60 fps) — not fast — performance for 2D content.

**Use Vulkan, not GL.** GL + EGL fails with `EGL_BAD_CONFIG` on
this machine (and many Mesa setups); Vulkan lavapipe works reliably.
If you don't have Vulkan drivers installed:

    sudo apt install mesa-vulkan-drivers vulkan-tools

## Determinism

Deterministic **on the same machine and toolchain**: fixed timestep,
no wall-clock leaks into simulation. Cross-platform bit-exact
determinism is not guaranteed (floating point may differ across CPUs).

## Choosing the wgpu backend

By default, all wgpu backends are enabled. On Linux this means Vulkan
is preferred; GL/GLES is the fallback.

On machines without a real GPU, **force Vulkan** so wgpu doesn't
waste time probing the broken GL path:

    WGPU_BACKEND=vulkan cargo run -p fear-cards

Combine with commas for a priority list, e.g. `WGPU_BACKEND=vulkan,gl`.
The engine logs which adapter and backend it actually selected at `INFO`
level during startup — check the `wgpu adapter:` line first if a window
opens but stays black.