Opened: 2026-11-01
Question: What runs on the main thread in the editor smoke's 1-2.2 s "input, commands and the smoke script" frames (SPIKE-87) and in the K view stall at orbit (Tracked, report `20261030-110527Z`: 1.3 s, 1,063 ms in "camera, ground heights and the globe overview" with 4 main-thread block decodes), and can it be avoided?
Why it matters: CON-22 (streaming never blocks a frame) and VIEW-061's rule (count and name main-thread decodes); the K view stall is a Tracked item.
Time box: 2 h (Sonnet)
Success criteria: the main-thread work in each long frame named; avoidable or not; one line for a fix PR with the expected longest frame; a verdict on VIEW-061's rule.
Fallback if it fails: the phase split from the stall probe alone.
Code location: branch `spike/first-read-stalls` (a key-timing probe in `window.rs`, a read-only coupled-lens request, `crates/app/examples/spike88_kview.rs`; never merged); patch and bench in `proto/`.
Closed: 2026-11-01

## Headline
- **The smoke's 1-2.2 s "input" frames are not first reads: they are the scripted vsync toggle.** `ToggleVsync` at frame 14 reconfigures the swapchain on the main thread: 884 ms (marker) and 1,292 ms (face site) on the software adapter, 69-89 ms at frame 12; every other scripted key takes under 0.1 ms and every edit-script step under 5 ms. A smoke artefact of WARP, not a first read; nothing to fix for users.
- **The K view stall is main-thread block decodes slowed by the tile refresh K starts, not by the coupled run itself.** A cold `field.height` on the main thread decodes a block in about 20 ms (p50) on an idle CPU. While loader threads saturate the CPU it takes 70 ms p50 (20 threads on 20 logical CPUs) and 106 ms p50, 317 ms max (40 threads). The report's 4 decodes in 1,063 ms (about 265 ms each) match the oversubscribed case: K re-keys all resident tiles once the run is ready (`managed.edit`, 7,489 resident; the report shows 36 jobs in flight and 517 queued).
- **The coupled run's load is not the cause**: from the store it loads in 1.7 s, and the main thread's cold reads during it stay at 22 ms p50 (max 47 ms).
- **The shared block cache is small and the streamer evicts the camera's blocks.** With 30 distinct directions re-read warm, a read costs 0.003 ms; with 40, every warm read decodes again (21 ms); under loader threads warm reads cost 71-145 ms. The source's budget is 512 MB (`SOURCE_CACHE_BYTES`), about 30 blocks of this store (inferred from the 30/40 boundary). A field rebuild (K, an edit) keeps the cache: the same direction on a new field reads in 0.0016 ms.

## Method
- Smoke probe: `planet editor --smoke --store global16384 --site 35.2538,135.0217` (with and without `--edit`) and `--site 10,10`, software adapter (WARP), with `PLANET_STALL_MS=300` (a stall report per long frame, rate-limited) and `PLANET_STALL_MS=60000` (per-phase means); a spike-branch probe prints the time of every scripted key (the command and its swapchain change) and of every edit-script step over 5 ms.
- K view bench (`spike88_kview`, the real store, read only): cold `field.height` at random directions on the main thread, timed one by one (30-40 per phase), then re-read warm; alone; while the K view's coupled run loads (a spike-only request that loads the stored run or would make it in memory, never writing: it loaded from the store); and under 20 or 40 loader threads reading cold random directions on their own fields of the same world (the stand-in for the tile refresh flood). A field-rebuild check reads one direction on field 1 (cold, warm) and on a new field 2.
- The original report (`raw/report-20261030-110527Z-head.txt`) is from commit 79653500 (2026-10-30), at 26,000 km, 2400 x 1300, RTX.
- Store unchanged: all 15 files under `global16384` have the same size and mtime before and after (`raw/store-files-*.txt`). No perf run. Bench processes at BelowNormal.

## Results
### Smoke: the scripted keys (`raw/smoke-keys-*.txt`)
| Frame | Key | Marker, editor on | Face 10 N 10 E, editor off |
|---|---|---|---|
| 12 | ToggleVsync (swapchain to immediate) | 69 ms | 89 ms |
| 14 | ToggleVsync (back to Fifo) | 884 ms | 1,292 ms |
| 4-45 | the other 19 keys | under 0.1 ms each | under 0.1 ms each |
- The stall reports at 300 ms (`raw/stall-reports-300ms.txt`) name "input, commands and the smoke script" once (1,203 ms, 0 decodes, no lock waits); every other long frame is render submit or exposure readback on WARP.

### Main-thread cold reads (`raw/kview-load.txt`, `raw/load_20.txt`, `raw/load_40.txt`)
| Condition | Cold read p50 / p90 / max (ms) | Warm re-read mean (ms) |
|---|---|---|
| alone, 30 directions | 19.6-20.0 / 21.4-22.3 / 88-98 | 0.003 |
| alone, 40 directions | 19.9 / 23.3 / 96.9 | 21.0 (the cache does not hold 40) |
| K run loading (from the store, 1.7 s) | 22.0 / 23.9 / 46.7 | 21.3 |
| K run ready | 20.7 / 22.5 / 24.9 | 21.4 |
| 20 loader threads | 70.1 / 81.1 / 90.8 | 70.9 |
| 40 loader threads | 106.4 / 188.5 / 317.2 | 145.1 |
| field rebuilt, same direction | 0.0016 | - |

## What runs on the main thread in those frames
| Frame | Work | Avoidable |
|---|---|---|
| Smoke frame 14 (1-2.2 s) | `Surface::set_vsync`: the swapchain reconfigured, waiting for frames in flight (WARP) | smoke only; a user's T press pays it once; not a first read |
| K view at orbit (1,063 ms) | the camera's ground and height reads (`viewer.camera` with `field.height` at the nadir) decoding 4 blocks the refresh flood evicted, at oversubscribed speed | yes: the camera's ground need not decode on the main thread |
| K view at orbit, plan phase (220 ms) | the plan's origin heights, 1 decode under the same load | partly (the same mechanism) |

## Fix (one line)
- The camera's ground height reads the warm-up thread's answer, never the field: `warm_heights` (already every 6th frame for the camera and its 1/2/4 s path) also returns the ground height under the camera, the frame uses the last one (one frame of lag at most, as the hover pick does since #858), and the blocks under the camera are pinned against eviction by streamer jobs. Expected: the camera phase under 1 ms at any load; the K frame then about 220 ms (the plan's own decode), which the same pinning brings to the plan's warm cost.

## VIEW-061's rule
- Counting and naming main-thread decodes is right and is what found this; it is not enough as a ceiling. A decode costs about 20 ms on an idle CPU but 70-320 ms while the streamer is busy, so any nonzero count can stall a frame exactly when work is heaviest (after K, an edit or a fast flight). Proposal for the planner: the camera's ground read joins the hover pick under VIEW-061's "no main-thread phase waits for a bake block" (the gate already fails a run on any main-thread decode, testing spec), and the stall report adds the decode time per phase next to the count.

## Limits
- The loader threads are a stand-in for the tile streamer (cold reads only, no tile generation); the real refresh also generates and uploads tiles.
- The K view stall was not replayed in the window (no K key in the smoke script, and the RTX run is the owner's); the mechanism is inferred from the report's numbers and the bench.
- Cache capacity "about 30 blocks" is inferred from the 30/40-direction boundary and the 512 MB budget, not read from the store's layout.

## Visuals
Nothing to look at: timings only, no renders.
