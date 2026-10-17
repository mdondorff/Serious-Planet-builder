# WARP device removal with the compact format and the atmosphere: exploration results (2026-10-17, sonnet-spikes)

Machine: Rasierklinge, WARP `d3d10warp.dll` 10.0.26100.9711, wgpu 30.0.1, main at cd2d02e (PR #210). The probe code is on branch `explore/warp-compact-atmosphere-removal` (never merged): env switches that skip passes or draws, a device-removed check after each frame, optional frame splitting, DRED. All rates are removals / runs of `planet editor --smoke --tile-format compact --size 160x90` on the software adapter (`PLANET_ADAPTER=software`), the window smoke (it fails at every window size from 160x90 up, in the same way as at the default size).

## What fails and when
Every run with the compact format fails; with a wait for the GPU after every frame the device is found removed after the frame's own submit at **frame 11** in most runs (in the runs without the extra waits it is found at the first readback after it). Without the extra waits the first symptom is the exposure readback (`read_exposure`): `map_async` fails with `BufferAsyncError`, `poll` returns `QueueEmpty`, and `ID3D12Device::GetDeviceRemovedReason` is 0x887A0005 (`DXGI_ERROR_DEVICE_REMOVED`, no more precise reason). A `GetDeviceRemovedReason` check after waiting for every frame shows the device healthy through frames 0 to 10 (first 11 frames), removed after frame 11, a lit frame with the atmosphere whose draw list mixes vertex-format and compact atlas runs (103 nodes: `Vx6 A0x40 Vx8 A0x24 Vx13 A0x8 Vx4`; frame 10 had 79 nodes of the same kind and passed). In a wait-per-frame variant the first removal came later (frame 38 in one run, 12 in another), so the frame number is a property of the script and timing, not of one operation.

Controls: vertex format on WARP 0/3 (and 6/6 earlier); the **RTX 3080 Ti on Vulkan** 48 frames, 12 exposure readings, smoke ok; the **RTX on the DX12 backend** (same wgpu DX12 code path as WARP) 48 frames, 12 exposure readings, smoke ok.

## What it does not depend on (each tried, still removed)
| change | removals |
|---|---|
| no sky-view and no aerial-perspective pass | 6/6 |
| no exposure-histogram and no exposure-adapt pass | 6/6 |
| no sky-view, aerial-perspective, sky-composite | 5/6 |
| no tonemap pass | 2/4 (two runs completed) |
| no sky-composite, exposure and tonemap passes (no post pass at all) | 5/5 |
| the per-frame atmosphere uniform write left out | 4/4 |
| per-draw instead of multi-draw indirect | 6/6 |
| per-draw and every terrain draw skipped (the pass only clears) | 2/4 (two runs completed) |
| per-draw and only vertex-format draws, or only compact draws | 4/4 each |
| group 3 (the atmosphere's bind group) set again after every pipeline switch | 6/6 |
| ring set 0 for every frame | 4/4 |
| per-slot instead of batched tile uploads | 4/4 |
| atlas side 1024 instead of 4096 | 4/4 |

## What changes it
| change | removals |
|---|---|
| the frame split into three submissions (before the terrain pass, the terrain pass, after it) with a wait and a removed-check after each | 0 in 38+ frames |
| split only after the passes that come before the terrain pass | 4/5 |
| split only before the post passes (the terrain pass and everything before it in one submission) | 1/5 |
| the vertex tile format | 0/3 |
| the RTX (Vulkan or DX12) | 0 |

## Tools that gave nothing
- The D3D12 debug layer and GPU-based validation are not available: `D3D12SDKLayers.dll` (Graphics Tools, an optional Windows feature that needs administrator rights to add) is not installed.
- DRED (auto breadcrumbs and page-fault reporting, forced on before the device is created): 0 breadcrumb nodes and no page-fault address on WARP.
- A renderer-level loop in the test crate (compact pools of 408 or 4096 texel atlases, the viewer's lit plan and warm-up, `render` timed or untimed or `submit`, `read_exposure` every frame, 30 frames) does not fail here: the removal needs the window's streaming mix of vertex and compact tiles (`crates/render/tests/probe_removal.rs` on the probe branch).
