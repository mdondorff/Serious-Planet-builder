## Why
- The E4 design note (§3.3) says KR does not cut: when the running minimum along a trunk reach lies more than the G-INCISION envelope below the surface, the reach is reported and D-1's surface must fix it, never a burn. E3 needs to know, per variant and window, whether a surface can carry H1's trunks without a burn (lead #154).

## What changes
- BUILD-057: a `trunks` line and eight fields. Every H1 reach of at least 1 m3/s is sampled every L cell. Its bed is the running minimum downstream. The gate counts the reaches and vertices standing more than 50 m above it (the valley-depth bar, until G-INCISION has a number) and reports the deepest vertex with its reach.
- TEST-137: fixtures for a draining floor, a dam, the bar and the counted cells.

## Impact
cache (`river_suite.rs` `trunk_burn`, `river_gate.rs` `GateInput::trunks` and the report, the `river_gate` example builds the reaches from H1's graph); tests.
