# WARP parallel-device crash: exploration results (2026-10-16, sonnet-spikes)

Machine: Rasierklinge, Windows 11 26200, WARP `d3d10warp.dll` 10.0.26100.9711, wgpu 30.0.1, 20 logical cores; another coder session was building at the same time (rates are not timings, but they vary with load).

## Reproduction (our test binary, plain parallel `cargo test`, software adapter)
`cargo test -p planet-render --test gpu_warmup`: 2 of 6 runs end in `STATUS_ACCESS_VIOLATION` (0xc0000005). The Windows application event log (id 1000) names the faulting module both times: `d3d10warp.dll`, offset 0xa89b3 (a further run: 0xa842f), never our exe or a wgpu crate.

Crash rate of the same binary by libtest thread count (12 runs each): 1 thread 0, 2 threads 0, 3 threads 0, 5 threads 4.
By test filter, pairs of tests with `--test-threads=2` (10 runs each): 2 of 10 pairs crashed once (`a_warmed_renderer...` + `growth...`, `a_warmed...` + `warming_up...`), the others never; so two devices can be enough, rarely.
By renderer path at 5 threads (20 runs each): vertex format only 4, compact format only 15, both paths 4/12 at the start of the session; the rates move with machine load.

## Where it crashes (native stack, an in-process vectored exception handler, no debugger installed)
gpu_warmup, worker thread: the whole stack is inside `d3d10warp.dll` on an unnamed thread started by the ntdll thread pool (`KERNEL32!BaseThreadInitThunk` -> `ntdll` worker -> `d3d10warp.dll+0x950b8 -> 0x95388 -> 0x161c83 -> 0xa6741 -> 0xa6905 -> 0xa6cde -> 0xa82b1 -> 0xa89b3`), a read (or, in another run, a write) of an unmapped address. No test had finished yet: this is concurrent execution, not teardown.

Standalone harness `spikes/warp-crash` (plain wgpu 30.0.1, no project code), level 7 (a render pass whose vertex shader reads a storage buffer and `textureLoad`s a `texture_2d<u32>` atlas written with `write_texture`, 8 indirect draws), 6 threads each creating its own device in lock-step: the crash is on the submitting thread: `wgpu::Queue::submit -> wgpu_core::...::submit -> wgpu_hal::dx12::Queue::submit -> D3D12Core.dll -> d3d10warp.dll+0x7b40d` (read of 0x24, a null-based member access).

## Standalone rates (harness; N runs each)
| setup | crashes |
|---|---|
| WARP, 6 threads x 4 devices, level 1 (create + drop) | 0/8 |
| WARP, 6 threads, level 2 and 3 (compute pipeline, dispatch, readback; 64 MB buffer) | 0/8 each |
| WARP, 6 threads x 6 devices, levels 4-6 (render pass + depth, multi-draw indirect, timestamp queries), not in lock-step | 0/8 each |
| WARP, 6 threads, level 7 (storage buffer + `texture_2d<u32>` vertex fetch + `write_texture`), devices created in lock-step | 1/10; two later hunts for a crashing run stopped at run 3 of 40 and run 20 of 60 |
| the same with one shared `wgpu::Instance` | 2/30 |
| the same, 2 threads | 0/30 |
| the same, 1 thread, 36 devices in a row | 0/30 |
| RTX 3080 Ti Laptop, DX12, 6 threads, level 7 | 0/30 |
| RTX 3080 Ti Laptop, Vulkan, 6 threads, level 7 | 0/30 |

## Experiments in the real test (gpu_warmup, 5 threads, 20 or 30 runs)
| change | crashes |
|---|---|
| none | 4/20 (vertex path only), 15/20 (compact path only) |
| `device.poll(wait)` before the renderers are dropped | 14/20 (compact only): not a release-before-completion race at that site |
| a process-wide mutex around `GpuContext::new` (instance, adapter, device) | 0/20 (compact only), 0/30 (both paths) |
| a mutex around renderer construction (pipelines, pools) | 9/20 (compact only): no help |
