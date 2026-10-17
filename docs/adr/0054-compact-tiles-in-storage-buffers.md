# 0054 Compact tiles in storage buffers instead of texture atlases (amends ADR 0033 point 3)
Status: accepted (owner, 2026-10-17, opus-planner session)
Date: 2026-10-17
Source: the storage-buffer exploration (sonnet-spikes, PR #225, `spikes/compact-storage-buffer/results/results.md`); ADR 0033 points 3 and 4; the tile-format duality exploration (PR #218); learnings of 2026-10-17

## Context
ADR 0033 point 3 put the compact tiles' texels in 4096 x 4096 `Rgba16Uint` texture atlases written by `write_texture`. Nothing records why a texture was chosen over a storage buffer: ADR 0033 and SPIKE-06 compared the atlas only with the vertex format. The exploration built the same texels in storage-buffer blocks read by index: images byte-identical on WARP and the RTX (90 debug-view frames, persistent renderers, the lit atmosphere frame), RTX GPU time equal (ratio 1.001 to 1.011 over four views, indicative, not a perf window), per-slot uploads 33 % cheaper and batched uploads equal with fewer write calls. A buffer block of 128 MiB (`max_storage_buffer_binding_size`) holds 15,405 slots against the atlas's 15,376, so the shares need the same block counts (15 mid, 23 high) and bind groups. The WARP device removal persists with buffers, so the atlas is not its trigger.

## Decision
1. **The compact texels live in storage-buffer blocks**, one block per former atlas, at most 128 MiB each, slots addressed by index; the texel bytes (16-bit height, two 16-bit octahedral normal components, 16 spare bits) and the 128 B record are unchanged, so packs, mesh keys and goldens are unchanged.
2. **The texture atlas path is removed** once the buffer path passes; no runtime switch stays behind.
3. **The vertex format stays** for levels 0-2, the CON-13 fallbacks and the CPU reference. Whether to retire it from rendering stays an M5 gate question (PR #218).
4. **When:** within M5, after item 7 and before item 8, so the M5 perf window measures the final layout and item 7b reviews final images.
5. Everything else in ADR 0033 holds (shares, slot accounting, upload cap, CON-13 guard, finest level).

## Consequences
- ADR 0033 point 3's atlas sizes are read as buffer blocks of the same count; `PoolConfig::allocated_bytes` counts buffer bytes and stays asserted at tier A.
- A slot's layout is no longer bound to a texel format: later per-tile data (climate, water) can sit in the slot without a second texture. Each such change still needs its own ADR.
- On WARP, `write_buffer` runs at about 13.7 MB/s against about 100 MB/s for `write_texture`, so software tests that fill many tiles get slower (a test cost, not a product cost).
- No golden changes are expected; any that move go through `golden-review`.

## Licences of adopted code or techniques
None.
