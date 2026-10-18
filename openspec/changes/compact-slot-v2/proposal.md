## Why
ADR 0062 (accepted 2026-10-18) retires the vertex tile format from rendering. Its first two steps (SPIKE-12 Q5 steps 1 and 2, one session): one compact slot for every tile, with a texel wide enough for cliff tiles (so the vertex fallback has no job left) and with the climate block and the record inside the slot, and the pool arithmetic that charges the record per slot (SPIKE-12 Q3: with the reserve unchanged the mid share would fall from 230,640 to 123,008 slots; those are the spike's atlas-model numbers, the specs state the absolute numbers on the 231,090 baseline and the headroom pins stay fractions).

## What Changes
- generators: the compact texel is a 32-bit height step count plus two 15-bit octahedral normal components (GEN-019); the slot is header 128 B, climate block 324 B, pad 4 B, texels (GEN-025: 9,168 B at cells 32, 456 B of header, climate block and pad before the texels); `MESH_VERSION` 8; the CPU builder and mirror; the climate leaves the texels' spare channel (GEN-021).
- cache: pack payload version 3, the pre-fill is rebuilt, old records are misses (STRM-021).
- render: the shader decodes the wide texel and reads header and climate from the block; the per-slot record buffer is gone; one `write_buffer` range per run of slots (REND-016, REND-018, REND-027); tile reference unchanged.
- frame, streaming: the pool arithmetic: the record is part of the slot, 14,639 slots per block, 15 blocks = 219,585 slots (mid), 23 = 336,697 (high), -5.0 % against 231,090 and 354,338 (ADR 0063 sets the 9,168 B slot and corrects ADR 0062 point 4); ADR 0033 point 3's share table re-cut; headroom pins stay as fractions; upload cap fill time 77 frames of 114 slots (STRM-006).
- tests: CON-13 and CON-14 on the steep field as tier-B tests (the SPIKE-12 T6 rows), the wide-texel image noise check, known-answer hashes re-recorded; every terrain golden regenerated through `golden-review` (rounding noise, ADR 0017).
- The vertex format, the vertex pool, the relief guard and `finest_class_key` stay until `remove-vertex-render-path`.

## Impact
Spec: GEN-010, GEN-019, GEN-021, REND-016, REND-018, REND-027, STRM-006, STRM-021, TEST-011 modified; GEN-025 added. Not in this change: the render floor (`render-floor-3`, independent; STRM-021 is modified by both, archive this one first), the removal of the vertex path, any perf verdict (indicative SPIKE-12 numbers: GPU time +0.1 to 0.5 %; the perf window follows the removal). Order within M6: before the edit path (SPIKE-13).
