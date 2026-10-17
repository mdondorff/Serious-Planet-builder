## Design

**Layout.** A block is one buffer of `block_slots x slot_bytes` bytes (15,406 x 8,712 B = 134,217,072 B at cells 32, 655 B under the 128 MiB binding limit); slot `s` of the block starts at texel `s x (cells + 1)^2`. The texel is the same 8 bytes (little-endian `u16` x 4), read as `vec2<u32>` and split into the four channels the atlas returned, so the vertex and fragment stages are unchanged after the read. The tile reference holds the slot's first texel (`local x w x w`, below 2^25) in word x; word y is reserved. The ADR text says 15,405 slots; 128 MiB / 8,712 B is 15,406.9, so 15,406 (the share arithmetic is the same, 15 and 23 blocks).

**Bindings.** One bind group per block (group 2), as with the atlases; storage buffers per stage: vertex 4 (nodes, records, tile references, block), fragment at most 4 (references, block, water nodes, nodes), under the default 8. All three limits were already required by name by the gate; the change adds a test that ties them to `COMPACT_BLOCK_BYTES` and the count.

**Uploads.** `write_buffer` at `local x tile_bytes`; the batch coalesces runs of adjacent slots of one block into one contiguous copy (a run is no longer cut at atlas rows). The call counts follow the runs (REND-018). On WARP `write_buffer` runs at about 13.7 MB/s (exploration), so software tests that fill many tiles are slower; times are reported in the PR.

**Safeguard.** The buffer path was added beside the atlas first (a process switch for the differential only), compared frame by frame on WARP and the RTX (debug views, persistent per-slot and batched, lit with the atmosphere, lit water), and only then was the atlas removed; the atlas path's hashes are pinned as known answers (keyed by driver build; another driver build prints its hash).

**Fallback.** Keep the atlas and report (time box 1.5 days); not needed.
