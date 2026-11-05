## Why
- I8a slice 2b PR 1 (planner, 2026-11-05; design notes in `ground-channels-2b`): the viewer's tile builder will read the baked channels, drainage area (kind 14) and deposited depth (kind 15), at 81 points a tile. The reader of #1157 borrowed its store and world, and cached whole faces (151 MB at 12,288 cells, over the planner's 64 MB guard).

## What Changes
- `LayerChannels` owns `Arc` handles of the store and the world (no lifetime), so the field that reads it can own it.
- The area is read through windows of the decoded face (256 cells and the one past, 66 KB) streamed from the record; a face is never held whole. Windows and depth tiles are cached under byte caps (`ChannelCaches`), the entry just decoded always kept, and workers asking for one window together decode it once.
- A `ChannelSource` trait in the generators crate (area as log2 cells, deposited depth in metres), implemented by `LayerChannels`.
- A diagnostic decode that skips the key check, for the measurement of a store made under another build's keys; a test pins that no runtime code calls it (STRM-075).
- The measurement probe `channels_probe` (examples), with the figures in `design.md`.

## Impact
- Code: `cache` (`layer_channels.rs`), `generators` (`channels.rs`), `app` example. Requirements: STRM-075 added (STRM-072 lives in the unarchived m9-bake delta and is not edited). No store format, key or picture changes.
