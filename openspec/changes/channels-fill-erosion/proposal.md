## Why
- I8a slice 2b PR 4a (planner inbox #67: Option A accepted, split 4a to 4d): the materials will take the pit depth of keep-pits (kind 17, GEN-120, STRM-076; #1192) and the erosion depth as inputs ("shallow fill: wet or bog, deep fill: gravel or sediment flats; rock where eroded deep"). PR 4a is the reader side only: no slot, key, shader or look change.

## What Changes
- `ChannelSource` (generators) gains `fill_depth_m` and `erosion_depth_m`; `LayerChannels` implements them: the pit depth is the stored cell of kind 17 (`fill_m(code)`), the erosion depth per cell is `max(0, deposited - (change + pit))` from kinds 15, 13 and 17 (a tile without a pit record counting 0). The pit depth is added back to the change because the filled look is what the runtime draws, so a pit is not erosion.
- The bilinear read of a per-cell value across tile borders is one helper (`tiled_bilinear`), used by the deposited, pit and erosion depths.
- Byte-capped caches for the pit tiles (kind 17, `tile_bytes`) and for the decoded layer tiles (kind 13, `change_bytes`, 128 MB by default: a core is 37.7 MB at 3,072 cells), each decoded once under concurrent workers; `ChannelStats` counts the decodes.
- The reader's identity folds the pit key (which carries the fill scale, so a new scale is a new identity) and `CHANNEL_READER_VERSION` is 2.

## Impact
- Code: `generators` (`channels.rs`), `cache` (`layer_channels.rs`). Requirements: STRM-075 modified (it is this series' own). Nothing reads the new methods yet (4b fills the channel block); no key, picture or golden changes.
- Open: the erosion depth's definition is mine (explorer's `removed_depth_m` takes the stored change; with keep-pits the stored change includes the pits, so I add the pit depth back first). It is for the explorer lane to confirm. The cost of a layer tile decode (kind 13, up to 37.7 MB) is not measured on a real store (the preview store awaits the fixed re-bake).
