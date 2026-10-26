## Why
The first envelope window with the chunk stage (2026-10-26) gave a curve that is not monotone: a flight of 12 s at 135 to 280 m/s covers 1.6 to 3.4 km, a chunk edge is about 4.9 km, so most flights cross no edge and pass whatever the speed; only the flights that happen to cross one expose the limit (the chunk wait of 3.5 to 6 s against the plan's look-ahead). Planner decision 2026-10-26: start each flight so that it crosses one chunk edge half way (a deterministic worst phase), so the curve is a floor.

## What Changes
- `envelopeperf::straddling_start_lat` finds the first chunk edge north of a trial's site by walking the meridian and bisecting, and starts the flight `EDGE_FRACTION` (0.5) of its length before it; `trial_start` applies it when the scene runs the chunk stage (a start moves at most one chunk edge, about 5 km, so trials keep their ground apart).
- The log gives `edge_fraction`.
- No change without a chunk stage; the flights, the search and the shares are as before.

## Impact
- Spec: testing TEST-054 (the start crosses an edge half way) and TEST-055 (the log names the fraction).
- Code: `app/src/envelopeperf.rs`; tier A tests on a fake grid and on the real chunk grid.
- The M8-8 window's cost is unchanged (about 50 min); the curve must be measured again on this code.
