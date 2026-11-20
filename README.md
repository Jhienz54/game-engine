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
- `examples/topdown-demo` — playable demo.

## Running the demo

    cargo run -p topdown-demo

## Environment

Tested on Lubuntu with Mesa llvmpipe (software GL). wgpu’s GL backend is enabled by default on Linux and is used as a fallback when Vulkan isn’t available.. Expect playable (30–60 fps) — not
fast — performance for 2D content.

## Determinism

Deterministic **on the same machine and toolchain**: fixed timestep,
no wall-clock leaks into simulation. Cross-platform bit-exact
determinism is not guaranteed (floating point may differ across CPUs).

## Choosing the wgpu backend

By default, all wgpu backends are enabled. On Linux this means Vulkan
is preferred and GL/GLES is the fallback. To force a specific backend
(useful on machines without a real GPU):

    WGPU_BACKEND=gl cargo run -p topdown-demo

Other values: `vulkan`, `metal`, `dx12`, `dx11`, `webgpu`. Combine with
commas for a priority list, e.g. `WGPU_BACKEND=vulkan,gl`. The engine
logs which adapter and backend it actually selected at `INFO` level
during startup — check that line first if a window opens but stays
black.