## Why

ADR 0054 (accepted 2026-10-17): the compact tiles' 8-byte texels sit in 4096 x 4096 `Rgba16Uint` atlas textures for no recorded reason; the storage-buffer exploration (PR #225) drew byte-identical images on WARP and the RTX at equal GPU time, with cheaper writes and a free slot layout (per-tile data can follow the texels without a second texture). Queue item 7c, before item 7b and the M5 perf window, so the window measures the final layout.

## What Changes

- render: the compact blocks are storage buffers (`block_slots x slot_bytes`, at most 128 MiB), read by `slot start + j x (cells + 1) + i` in the vertex stage and for the climate in the fragment stage (GEN-021, the 4th channel of the first texels, byte for byte); the atlas texture path, `MAX_ONE_SHOT_SIDE` and the texture readback are removed; uploads are `write_buffer` per slot or per run of adjacent slots of one block (REND-018); warm-up creates block 0 as a buffer (REND-017).
- frame: `DrawBuffer::Compact` (was `Atlas`), `COMPACT_BLOCK_BYTES`, `block_slots_in`; `compact_format_pool_with` takes block slots, not an atlas side (15,406 slots per block at cells 32: 15 blocks = 231,090 mid, 23 = 354,338 high).
- streaming: `PoolConfig::block_alloc_bytes` removed (a block is always `slot_bytes x block_slots`).
- gate: the limits the compact block needs are tested by name (`max_storage_buffer_binding_size`, `max_buffer_size`, `max_storage_buffers_per_shader_stage`); all three were already required.
- tests: the image differential against the atlas path while both existed (byte-identical, WARP and RTX), replaced by known-answer pixel hashes recorded from the atlas path.
- spec: REND-016, REND-017, REND-018 (modified), STRM-006 (modified), VIEW-001 (modified).

Not in this change: texel bytes, records, packs, mesh keys, MESH_VERSION, any golden, the vertex format, the upload cap, the climate layout.
