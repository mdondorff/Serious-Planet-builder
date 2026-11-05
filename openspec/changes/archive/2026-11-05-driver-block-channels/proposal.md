## Why
- I8a slice 2b PR 2 (planner, 2026-11-05; notes `ground-channels-2b`, decision 1 accepted): the ground materials need the baked channels, drainage area and deposited depth, per pixel. The carrier is the tile's existing driver block (GEN-074, 81 words, sampled bilinearly into the reserved inputs `SoilDepth` and `Moisture`), so no new block, slot growth or shader interface.

## What Changes
- `HeightField::driver_channels(dirs)` (default none) gives the tile builder the channels at the block's 81 texels; the builder writes the deposited depth as the soil code (square-root coded over 8 m) and a ramp of the drainage area's log2 (0 at 0 cells, 1 at 10) as the moisture code; the insolation stays neutral; a texel without a channel stays neutral (GEN-119).
- The eroded field reads a `ChannelSource` (PR 1) when `--channels on` (with `--erosion on`; off by default; refused when the layer holds no drainage area): `ErosionField::with_channels`, `ErosionSwitch.channels`, `LayerChannels` built from the layer's shape (areas under pass 1's cells, depths under pass 2's) (VIEW-066).
- Keys: the field folds `DRIVER_MAP_VERSION` and the channels' identity into its climate version word, which every mesh key already carries (the precedent of the lens word in `chain.rs`). So only a field that reads channels gets other keys: **no `MESH_VERSION` bump**, no stored tile of any other field is invalidated and no mesh pack rebuilt (the planner accepted this in place of the bump the notes named). Every wrapper of a field forwards the channels; a source-scan test fails when a file forwards the version word and not the channels.
- The default look ignores the drivers (GEN-073), so no picture or golden changes; PR 3 puts curves on `soil_depth` and `moisture`.

## Impact
- Code: `generators` (`drivers.rs`, `channels.rs`, `height.rs`, `erosion.rs`, `height_fields.rs`, `compact.rs`, the forwarding wrappers), `cache` (`LayerChannels` takes the two key resolutions, `identity`, `faces_with_area`), `app` (the option, `worldfield.rs`, `skeleton.rs` guard). Requirements: GEN-119 and VIEW-066 added, GEN-074 modified (its "neutral only" clause).
- The preview store `global16384-m9c14` was baked before the class source changed (#1180), so current keys refuse it: the live check awaits a rebaked store; tier A, B and C use the tiny planet.

## Notes kept from the design
- The preview store global16384-m9c14 was baked before the class source changed (#1180): current keys refuse it, so the live per-tile cost of the 162 channel reads a tile (cached windows and tiles, PR 1) is not measured; it needs a rebaked store and the perf window.
- A failed channel read is recorded in the field's error slot like a height's and leaves the texel neutral; the viewer does not read that slot (as for heights).
- The reader's identity folds the keys of a face's area and of a tile's depth (which carry the layer's parameters, the channels' version and scales), the resolutions of both and CHANNEL_READER_VERSION; the field's climate version word folds DRIVER_MAP_VERSION and that identity.
