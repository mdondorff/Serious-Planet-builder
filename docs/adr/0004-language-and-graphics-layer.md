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
Option 1 with a thin render-HAL seam: `render` talks to wgpu through a small internal interface (device, queue, buffers, textures, pipelines, passes), so a raw-Vulkan/D3D12 backend can replace it without touching `frame` or above. wgpu is **pinned** (30.0.1 at M0) and upgraded deliberately at milestone boundaries, recorded in `docs/learnings.md`. Shaders are WGSL validated through naga. Re-evaluated at the M2 go/no-go gate against the GPU-driven terrain spike.

## Consequences
- M0 confirmed wgpu 30 runs headless on WARP (DX12) on this machine; the Linux lavapipe path is verified by CI.
- Sparse textures are not available: software virtual texturing or per-tile texture-array slices.
- The HAL seam is introduced when the first real pass is written (M2), not before; M0 code calls wgpu directly inside `render` only.

## Licences of adopted code or techniques
wgpu: MIT OR Apache-2.0. Dependencies are checked by `cargo deny` (CON-27).
