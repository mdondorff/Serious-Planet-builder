## Why
- Lead #171 (owner, 2026-11-18): the prototype conditions its base (A+), so its `S0` and H1 differ from main's; the gate measured its corridor and every delta against main's start and main's H1 trunks.

## What changes
- BUILD-060: `--start FILE` / `#start=`: the run's own start surface; the corridor's trunks are the H1 reaches of the row's graph (`river_gate_graph::h1_trunks`); default unchanged.

## Impact
cache (`river_gate_graph::h1_trunks`, the `river_gate` example), xtask (`rivergate.rs`, help); tests.
