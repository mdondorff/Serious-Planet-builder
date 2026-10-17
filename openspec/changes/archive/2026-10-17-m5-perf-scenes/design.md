## Per-pass timestamps
A timestamp is written by a pass at its start and end (`timestamp_writes`), so every pass of the lit frame that is a pass carries a pair: the query set holds 16 queries, pass k uses 2k and 2k+1 (terrain 0 and 1, water 2 and 3, as the first versions had them). The pairs are resolved one by one into 256-byte strides (a resolve offset must be aligned to 256), only for the passes that wrote, and the used range is copied to the readback buffer; an unwritten query is never resolved. The pure `PassTimes::from_ticks` turns ticks into milliseconds and returns nothing for a zero or negative interval.

Not passes: the scene copy is a command, so its time is the gap between the end of the terrain pass and the start of the water pass (the copy and its barriers); the static tables' upload (`atmosphere-luts`) is a queue write when a body's tables change, so a steady frame has no such row (the harness says so). The frame's GPU span is first timestamp to last, gaps included.

## What the budgets are
Atmosphere = sky view + aerial perspective + composite, summed per frame, median over frames. Water = water pass + scene copy, per frame. Terrain = the terrain pass (M2's 8 ms). The lit frame's GPU span is judged against 16.6 ms and the waited frame's p99 against 20 ms in the static views, as the terrain scene judges its views; the pipelined frame time of the streaming scenes (TEST-019) is the user-facing frame verdict and gets its `--lit` variant. A GPU row needs the view's clock samples at the floor (ADR 0036); a frame row holds at any clock state (ADR 0034 as amended). Flights are report only. A view without water or without the atmosphere says so instead of passing.

## Warm-up and noise
`enable_lit` is not part of `warm_up`: the lit scenes build the renderer, `warm_up`, `warm_up_lit` with the look's plan and an empty water plan, then seed the exposure at the view before the first timed frame; the streaming `--lit` variant warms before the settle. The noise rule is the accepted 10 % of the waited frame median with one retry. ADR 0036 point 3 (proposed: also above 0.1 ms) is not applied; the scene prints, for the atmosphere and the water sums, the run-to-run noise, its absolute spread and whether the proposed floor would have kept the view noisy, which is the data that ADR waits for.

## The views
The static views are the viewer's (ground 1 m, aerial 20 km, orbit at the terrain scene's site, 20 N 30 E) and two coast views at the first sea-to-land shoreline east of 30 E (searched on the parallels 20, 30, 10, 40, 0, 50 and -10 N, in that order), 2 km and 3 m above the water. The flights are the water pass's flicker test paths (glide: 6 m over the water from 400 m out, 228 m towards the shore; descent: 300 m to 4 m, log-spaced, 1,200 m out closing to 500 m) with as many cameras as asked (240 by default). Each flight camera is planned, its new tiles uploaded, drawn once untimed (the first draw of new tiles pays for their first use) and drawn twice timed.

## Tile formats
`lit-formats` creates a vertex and a compact renderer and measures a view in one and then the other before the next view, so the clock state is as equal as the machine allows. The ratio compact over vertex is printed per pass family and stored as `ratio_*_permille` in the compact run's counts. The software smoke runs the vertex format only (WARP removes its device with the compact format and the atmosphere, status).

## The window
`perf m5 --store DIR` runs `lit`, `lit-formats` and the seven streaming scenes with `--lit`, each part under the gate, each streaming scene in its own process; a failing part does not stop the next.
