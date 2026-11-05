## Why
- SPIKE-100 D1 (material thresholds) reads the erosion depth ("rock where it exceeds 2 m") and the owner's direction reads the fill depth of keep-pits (shallow fill wet or bog, deep fill gravel or sediment flats). Both come from the baked channels (STRM-076 kinds 16 and 17, the reader of 2b PR 4a) and have no slot in the driver block, whose eight material inputs are all taken.
- Plan: 2b PR 4 is decided as option A (planner inbox #67): a second per-tile block, `INPUTS` 8 to 10, `MESH_VERSION` 11, look schema 10, in four PRs. This is 4b, the slot, the builder and the keys. 4c adds the inputs, the shader and the debug views; 4d the curves.

## What Changes
- GEN-126: a channel block of 81 words after the driver block in every compact slot: fill depth code and erosion depth code per texel (square-root codes of 0 to 256 m), every other bit zero, neutral 0.
- The builder writes it from `HeightField::driver_channels` (`ChannelSample` gains `fill_m` and `erosion_m`; the erosion layer's field reads them through `ChannelSource`, 4a); wrappers forward it with the samples they already forward.
- `MESH_VERSION` 11: the slot is 9,812 B at cells 32 (3,412 at 16, 34,900 at 64), the texels at byte 1,100; every mesh key changes; `CHANNEL_BLOCK_VERSION` and the depth scale enter the channel identity of a field's version word.
- The shader does not read the block yet (4c); the neutral block and the unchanged shader keep every picture.

## Impact
- Code: `generators` (`compact.rs`, `drivers.rs`, `mesh.rs`, `erosion.rs`), `render` (shader offsets follow the constants), `frame` and tests (pool arithmetic), the pinned known answers (mesh key, compact hashes, plan snapshots: keys only).
- Pool: the slot grows 9,488 to 9,812 B (+3.4 %): a 128 MiB block holds 13,678 slots (14,146 before, -3.3 %); mid share 15 blocks 205,170 slots (212,190), high share 23 blocks 314,594 (325,358); an upload cap of 1 MiB moves 106 slots a frame (110); a full ground-1m fill takes 83 frames (80). The shares and the VRAM ceiling are unchanged; the pins are the same fractions.
- Cold after the bump: every prefilled mesh pack of a store (keys include `MESH_VERSION`), so the packs of `global16384-m9c14` and of any other store under the current version are unreachable and must be filled again; the read-only stores `global16384` and `global16384-m9r2` are never written (their stale packs are expected, planner condition 1). Tiles in memory and tiles generated on the fly are rebuilt.
- Stride: the slot is no longer a multiple of 16, only of 4. Safe because every read of the slot is a `u32` word and the block is bound whole (a test pins both); if either changes, pad to 9,824 B.
