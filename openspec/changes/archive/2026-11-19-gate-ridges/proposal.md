## Why
- Lead #172 item 3: step C's second PR, cut-through saddles at 1 and 5 km2 and crest rounding (p95 ridge lowering) on one shared ridge mask.
- The G0 contracts' section 9 define the ridge cell and the saddle; reading 21 measures the L-stage deltas against `S_KH`, with `S0` beside it.

## What changes
- BUILD-060: the ridge columns on every row, against `S_KH` (from KH's stage field) and against `S0`; a `RIVER-GATE ridges` line.
- BUILD-060: the bar `bar_crest` (p95 at most 15 m, 35 m on M1) on rows with a graph; `bar_saddles1` and `bar_saddles5` at most B's in `--table`.

## Impact
`river_suite` (`ridge_cells`, `ridge_report`), `river_gate` (`GateReport` ridge fields, `GateParams::crest_max_m`, two more routings per row), the `river_gate` example, xtask `b_bars`; tests.
