# 0042 M5 frame structure, look data and the viewer window
Status: accepted (opus-planner, 2026-10-16, under the owner's delegation of 2026-10-12, as the M5 queue starts; sketch by opus-planner, 2026-10-15, for M5, revised after the spec-guardian review; decided under the owner's delegation of 2026-10-12 once the M5 queue starts; the viewer window is the owner's first M5 item of 2026-10-15)
Date: 2026-10-15
Source: ADR 0013 (render graph: one monolithic terrain pass "until a second pass exists"; no product code declares a graph yet); ADR 0015 (window surface in render); ADR 0033 (VRAM shares); ADR 0037 to ADR 0041; frame pipelining (STRM-016); owner's M5 decisions 2026-10-15 (budgets, no egui in M5); constitution CON-02, CON-03, CON-04, CON-05, CON-09, CON-10, CON-12, CON-20, CON-21

## Context
M5 adds the first passes beside terrain, which is the trigger ADR 0013 named for a declared graph, and the first on-screen window. The frame loop is pipelined; nothing presents to a surface yet.

## Decision (proposed)
1. **Two paths.** The **lit path** (passes below) is what the M5 views, goldens and perf scenes measure from the change that introduces it; the **debug path** (today's single pass into `Rgba8Unorm`) keeps every debug view and its goldens. The M4 budgets are re-measured on the lit path in the first M5 perf window.
2. **Pass order of the lit path** (declared in `render::graph`, compiled when the frame shape changes): (1) static atmosphere LUTs, uploaded at body load; (2) sky-view LUT and aerial-perspective volume (compute); (3) terrain into HDR colour and depth; (4) water on the terrain grid (ADR 0041 point 4) into HDR colour and the same depth; (5) sky and aerial-perspective composite, reading that single depth buffer (aerial perspective applies over water too); (6) exposure histogram (compute); (7) tone map into the presentable target; (8) debug overlays, later UI (ADR 0015).
3. **The plan stays the decision** (CON-03, CON-04): `FramePlan` gains the sun direction, the time t and dt (from the injected clock), the exposure mode and its parameters, the water tiles and the look parameters' handle; `render` executes it.
4. **Viewer window:** `planet editor` opens a window (`winit`; `render` owns the surface, ADR 0015), runs the pipelined loop with the scripted orbit-to-1 m flight by default (keys: pause, restart, debug view, vsync on/off) on the injected clock, and shows in the title the frame rate, the CPU frame time and the GPU time (meaningful with vsync off). No egui in M5. Tests and perf scenes stay offscreen; a `--smoke` test starts and exits the window path.
5. **Budgets** (owner, 2026-10-15): atmosphere (passes 2 and 5) <= 1 ms, water (pass 4) <= 2 ms, at 1440p on the reference GPU in a valid perf window; frame median <= 16.6 ms and p99 <= 20 ms unchanged; per-pass GPU timestamps in the perf log. **Frame targets** (HDR 29.5 MB, depth 14.7 MB, LUTs and the aerial-perspective volume, histogram) are a count-based frame-target share asserted at tier A from the compiled graph, recorded as an amendment to ADR 0033's reserved remainder when the change lands.
6. **Look data:** presentation parameters (atmosphere coefficients, liquid appearance, albedo tables, exposure range and speeds, tone curve) live in a look file beside the world definition: schema-versioned with tested migrations like the definition (CON-10), but **not** part of the world definition's hash or any cache key, so a look change re-keys nothing and invalidates no authored content (CON-05, CON-09). Kind ids and anything a generator reads stay in the world definition.
7. **Crate homes** (CON-02): f64 references and look maths in a GPU-free module (`frame::look`); fields (climate per tile, W, the ocean mask) in `generators`; passes in `render`. Every change opens its spec deltas with each attribute's class and bound (ADR 0035), traced by spec-lint.

## Consequences
- The render graph becomes product code; its compile tests exist.
- The window gives live viewing (also over remote desktop); judgements use goldens, contact sheets and videos (ADR 0037).
- Tests: tier A for the compiled frame shapes, counts and the look-file schema; tier C goldens per view; the window smoke test.

## Licences of adopted code or techniques
winit, raw-window-handle: MIT OR Apache-2.0 (ADR 0015), checked by `cargo deny` when adopted.
