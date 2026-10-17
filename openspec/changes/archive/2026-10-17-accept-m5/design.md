## Three outcomes
M4's acceptance had no GPU row, so a window was valid or refused. M5's budgets are GPU times, and ADR 0036 makes a view's GPU verdict depend on its clock samples. A window that is trustworthy for frames but has a view below the floor should neither pass nor fail those rows; it is INCOMPLETE and the report names the rows to re-run. The refusal stays for what voids frame verdicts too (ADR 0020 gate, throttling, smoke, another adapter or power scheme, the owner's override, noise). The idle GPU start voids GPU verdicts of the session only (`validity::log_gpu_session_valid`), so the frame rows are still judged.

## Combining runs
A held view has three runs: the view's medians are the median of the runs' medians, its p99 and maximum the largest of the runs'. The rows come from `planet_perf::lit::view_verdicts`, the function the scene prints its rows with, so the console of the window and the report cannot disagree.

## Gated and report-only
Gated rows: `lit/<view>` for the five held views, and the five gated streaming scenes on the lit path with the harness's own verdicts (`case_verdicts`). Flights, `lit-formats`, `dive-fast`, `dive-cold`, the GPU span and the host load are reported. A coast view must hold water (`water_nodes` above zero) or the window is incomplete: a coast view without water measures nothing of the water pass.

## What is trusted
`protocol.field` must say the lit path with compact tiles (the shipped format); a vertex-tile log is a measurement of something else. The frame size must be 2560x1440 and the source the production store. The streaming logs must come from the same source, adapter and power scheme as `lit.json`.

## Capture
The capture renders `render(.., false)` frames and reads the colour target back; it never sets timestamps or the clock, and `xtask` encodes with `ffmpeg` only if `ffmpeg -version` runs. The PNG sequences are the deliverable when it does not.
