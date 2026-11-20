# Roadmap

Milestones are strict: each must build, test green, and demo its
feature before the next begins. Milestone numbering is locked — new
work goes into a new milestone rather than renumbering.

| # | Milestone | Delivers |
|---|---|---|
| M0 | Workspace skeleton | Crates, deps, `hello` demo, CI-able build |
| M1 | Core model | Arena, `NodeId`, hierarchy, transforms, interp snapshot, despawn queue, RON round-trip with validation |
| M2 | Platform + renderer bootstrap | winit window, wgpu device/queue/surface, clear color, resize, Escape-quits. **Validates the GL backend on Mesa llvmpipe.** |
| M3 | Input + fixed-timestep loop | `InputState` snapshot, winit→input mapping, fixed-step accumulator with clamped dt |
| M4 | Assets (`engine-assets` crate) | Synchronous load, `image` decode, cache by path, `TextureHandle` + `TextureMeta` in core |
| M5 | Collision | `Aabb`, `Circle`, overlap queries, `Collider` on nodes, unit tests |
| M6 | Sprites + camera + upload path | Batched 2D sprite pipeline, `Camera2D`, layer/z sort, `upload_texture(handle, decoded)`, interpolation alpha threaded through `draw()` |
| M7 | Debug draw + bitmap text + overlay | Line/rect/circle primitives, embedded 8×8 bitmap font, FPS/draw-call/entity overlay toggled F1 |
| M8 | Game loop integration | `Game` trait, `engine::run`, documented lifecycle order, interpolation plumbed end-to-end |
| M9 | Topdown demo | Zelda-style player, AABB collision resolution, spawning pickups, collision overlay (F2) |
| M10 | Audio stub | `AudioBackend` trait, no-op impl |

## Post-v0.1 (not scheduled)

Scene save/load in RON from the game side, hot reload, tilemaps, text
formatting, sprite animation, 3D renderer module, editor.