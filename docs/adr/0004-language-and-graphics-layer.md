# 0004 Language and graphics layer
Status: proposed
Date: 2026-10-10
Source: background report §9, Recommendations 6; constitution CON-02, CON-26

## Context
The project is built largely by an agent, so compile-time feedback and headless testability weigh heavily. The graphics API choice is the hardest to reverse once shaders and the render graph exist.

## Options (with trade-offs)
1. **Rust + wgpu.** Strong compile-time checks, Cargo tooling, D3D12/Vulkan/Metal, headless on WARP and lavapipe (wgpu's own CI pattern). No sparse residency; bindless less mature; API churn each major version; mesh shaders and ray queries still evolving.
2. Rust + ash (raw Vulkan). All features, large unsafe surface.
3. C++20 + Vulkan/D3D12. Best tooling, UB-prone, weaker agent verification.
4. Engines (Bevy, UE5, Godot, Unity). Poor fit for a sphere-first, agent-built, own-generator project (report §9).

## Decision
Option 1. **The seam to wgpu is `render`'s public API**, not an internal HAL: no crate except `render` depends on wgpu, `render` does not re-export wgpu, and its public types (`GpuContext`, executors, frames) expose no wgpu types. A raw-Vulkan/D3D12 backend can then replace wgpu without touching anything above `render`. wgpu is already a HAL, so an internal abstraction over it would cost code and slow every new pass for a swap that may never happen; it is built only if a second backend becomes real.
- wgpu is **pinned** (30.0.1 at M0) and upgraded deliberately at milestone boundaries, recorded in `docs/learnings.md`. An upgrade is its own change: goldens stay byte-identical on WARP and lavapipe, or the owner re-blesses them before other work builds on the upgrade.
- Shaders are WGSL validated through naga.
- Backends: DX12 and Vulkan are compiled in (Windows and Linux, CON-26). Metal and macOS are out of scope.
- **M2 gate outcome (2026-10-10): wgpu kept.** SPIKE-01 found what the GPU-driven path needs (indirect indexed draws, first instance, multi-draw count, binding arrays, immediates, timestamps) on WARP and the RTX 3080 Ti; the M2 report found no blocker.

## Consequences
- M0 confirmed wgpu 30 runs headless on WARP (DX12) on this machine; the Linux lavapipe path is verified by CI.
- Sparse textures are not available: software virtual texturing or per-tile texture-array slices (the main constraint on M4–M5 texturing).
- Mesh shaders and ray queries stay optional; nothing in the plan depends on them (meshlets are revisited after the M2 gate, ADR 0014). f64 in shaders is not needed (CON-11).
- `cargo xtask layering` checks that only `render` depends on wgpu. Follow-up (queued): remove `pub use wgpu`, make the `GpuContext` fields private with accessors for adapter facts.

## Licences of adopted code or techniques
wgpu: MIT OR Apache-2.0. Dependencies are checked by `cargo deny` (CON-27).
