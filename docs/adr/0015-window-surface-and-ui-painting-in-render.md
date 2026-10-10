# 0015 Window surface and UI painting live in render
Status: accepted
Date: 2026-10-10
Source: m2-wgpu-seam review (docs/status.md); ADR 0004, ADR 0008; constitution CON-02, CON-03

## Context
ADR 0004 makes `render`'s public API the only seam to wgpu, and `cargo xtask layering` bans graphics-API crates outside `render`. The editor window (M2+) needs a presentable surface and an immediate-mode UI (egui). The usual integration (`egui-wgpu` in the editor crate, the editor creating the wgpu surface from its window) would pull wgpu above `render`. The owner decided on 2026-10-10 that wgpu must not leak above `render`.

## Options (with trade-offs)
1. **`render` owns the surface and paints the UI.** `editor` hands `render` a raw window handle and receives frame timing and input-independent results; it builds UI as egui output (`egui::FullOutput`: shapes, textures delta), which is GPU-free data, and `render` tessellates and paints it. Keeps the seam intact; `render` gains a UI pass and a dependency on `egui` (data types) and an egui painter.
2. `editor` uses `egui-wgpu` directly. Least code; breaks ADR 0004 and the layering rule.
3. A separate `ui-render` crate beside `render`. Same as option 1 with an extra crate; no benefit until there is a second UI backend.

## Decision
Option 1.
- `render` creates and owns the wgpu `Surface` from a window handle passed through `raw-window-handle` (no wgpu types in the call), handles resize and present, and reports surface errors as `render` errors.
- The UI pass is one more pass in the `FramePlan` (or its successor): `editor` produces egui's GPU-free output, `frame` or `editor` places it in the plan, and `render` paints it on top of the scene. `render` makes no UI decisions.
- `editor` and `app` may depend on `egui` (UI logic) and `winit` (windowing), never on `wgpu`, `egui-wgpu` or other graphics-API crates; the layering check enforces it.

## Consequences
- Easier: the backend swap promised by ADR 0004 stays possible; UI frames are plain data and can be snapshot-tested and stored in repro bundles.
- Harder: the egui painter lives in `render` (an adopted `egui-wgpu` painter wrapped inside `render`, or a small own one).
- Tests: tier A for UI output placed in the plan; tier C golden for a fixed UI frame (debug view class); layering test rejects `egui-wgpu` and `wgpu` outside `render`.

## Licences of adopted code or techniques
egui, egui-wgpu, winit, raw-window-handle: MIT OR Apache-2.0 (checked by `cargo deny`, CON-27) when adopted.
