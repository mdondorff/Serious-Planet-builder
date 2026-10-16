# 0047 Adapter gate: a discrete GPU, chosen before the device exists
Status: proposed (sonnet-explorer, 2026-10-16, drafting the owner's decision of 2026-10-16; accepted by the planner or the owner)
Date: 2026-10-16
Source: owner 2026-10-16 (M5 queue item 1); ADR 0004 (render's API is the seam), ADR 0015 (window surface in render), ADR 0020 (readiness gate), ADR 0042 point 4; constitution CON-03, CON-21; change `viewer-window` (VIEW-001)

## Context
Hybrid-graphics laptops hand out the integrated GPU unless told otherwise (Optimus; ADR 0020 refuses to measure on one). The viewer window is the first interactive use: a user who gets the integrated GPU, a software renderer or a GPU that lacks something the renderer needs should read why, not meet a crash inside device creation. The test adapters (WARP on Windows, lavapipe on Linux) must stay available to tests and CI, which need reproducible pixels.

## Decision
1. **Choose before the device.** `render` enumerates the adapters, describes each as plain data (`planet_frame::adapter::AdapterDesc`: name, backend, class, feature names, limits by name, and whether it can present to the window) and the GPU-free function `planet_frame::adapter::select` chooses or refuses. No device is requested on a refused adapter.
2. **Interactive mode accepts a discrete GPU with every required feature and limit and nothing else.** An integrated or virtual GPU, a software renderer, a missing feature, a limit below its minimum and a GPU that cannot present to the window are refused; the error names every adapter turned down with its reason (the feature or limit by name, the value and the minimum) and, for an integrated GPU, the High-performance setting.
3. **Test mode accepts a software adapter only** (`PLANET_ADAPTER=software`, `editor --smoke`): test pixels stay on the adapter the goldens are keyed to. The interactive editor refuses `--adapter software`.
4. **Deterministic choice.** Among acceptable adapters: Vulkan, then Dx12, then other backends; then name; then listing position. This picks the Vulkan adapter on the reference machine, the one the existing goldens are keyed to.
5. **Requirements are the limits the device is requested with** (wgpu's default limits, by name) and no required feature today: `TIMESTAMP_QUERY` and `INDIRECT_FIRST_INSTANCE` have fallbacks (no GPU time; the per-draw loop) and are requested when present. A feature the renderer starts to depend on is added to the list in `render::gate`, and the gate then refuses adapters without it.
6. **`GpuContext::new(HighPerformance)`** (`--adapter hardware`, `test gpu --real`, the perf harness) uses the same gate; the perf harness's own check (ADR 0020) stays.

## Consequences
- Easier: a refusal is a message; the rule is testable on descriptions (tier A) without a GPU.
- Harder: a developer with only an integrated GPU can no longer run `--adapter hardware`; they use the software adapter (the tests' default) or add a deliberate mode.
- Reversible: the rule is one function and one table; relaxing it (allowing an integrated GPU with a flag) is a mode.
- Tests: tier A on descriptions (VIEW-001: discrete accepted, integrated refused with reason, missing feature or limit by name, software only in test mode, several adapters deterministic); tier C smoke through the real flow (VIEW-005).

## Licences of adopted code or techniques
None.
