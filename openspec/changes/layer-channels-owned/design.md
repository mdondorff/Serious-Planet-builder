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

## Decisions
- Windows over whole faces: a face is over the 64 MB guard (the planner, inbox #65). A window advances 256 cells and holds the one past, so the four cells of a bilinear read are always in one window. A brotli stream cannot be entered in the middle, so a cold window reads the stream from its start to its last row: 0 to 330 ms in a generation worker, and a window serves a region of 208 km, so a flight at 200 m/s meets a new one about every 17 minutes.
- Caps in bytes (planner): `ChannelCaches::DEFAULT` 16 MB of windows (about 250) and 256 MB of depth tiles; the entry just decoded is always kept, so a cap below one entry still works.
- One decode per window or tile under concurrent workers (a lock around the miss path, the cache rechecked after it), counted in `ChannelStats` for the measurement.
- A blocked kind 14 (the explorer lane's format) would make a cold window about 2 ms; not requested.
- `diagnostic_decode_unchecked` stays in the library because the probe is an example of another crate; its doc names it a diagnostic and a test fails if any file outside its definition (tests and examples excepted) mentions it.
