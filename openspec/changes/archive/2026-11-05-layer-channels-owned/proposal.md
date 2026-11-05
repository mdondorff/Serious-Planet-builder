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

## Measurement (global16384-m9c14, RTX laptop, release build, indicative, no perf run)
| What | Figure |
|---|---|
| Kind 14 face (12,288 cells) | 151 MB decoded, 28.5 MB compressed |
| Whole-face decode | 390 to 450 ms, about 380 MB/s; process peak +144 MB per live decoded face |
| Window of 257 x 257 (66 KB), first rows of the face | 14 ms, process peak unchanged |
| The same, middle rows | 97 ms |
| The same, last rows | 331 ms |
| Kind 15 tile (3,072 x 3,072, 9.4 MB decoded) | 0.49 to 1.64 MB compressed, 12.7 to 20.8 ms |

- The store holds kind 14 for faces 0 and 2 and kind 15 for 2 to 3 tiles (the preview region).
- The store was baked before the class source changed (#1180): current keys refuse it, so the probe reads its records with the diagnostic decode.

## Decisions (kept from the design)
- Windows of 256 cells and the one past (66 KB) streamed from the record instead of whole faces (over the planner's 64 MB guard); a cold window reads the stream to its last row (0 to 330 ms in a worker), a window serves 208 km. Caps in bytes (16 MB of windows, 256 MB of depth tiles), the entry just decoded always kept; one decode per window under concurrent workers (one lock around the miss path).
- `diagnostic_decode_unchecked` (no key check) stays in the library for the probe example; a test fails if any file outside its definition (tests and examples excepted) names it.
- For the explorer lane: STRM-072's cache sentence (decoded faces, a borrowing reader) is reconciled with STRM-075 when m9-bake archives; a blocked kind 14 (a persisted-format change, the owner's call) would make a cold window about 2 ms.
