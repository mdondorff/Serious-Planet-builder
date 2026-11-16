## Why
- #161's stills showed it: in 3 of the 4 distinct worst R1h corridor failures (reaches 5842, 3264, 7459) the rise lies in the window's margin, outside the store's tiles. There a store holds the unprocessed start surface, the same for every variant, so the corridor pass shares counted margin failures for every variant alike.

## What changes
- BUILD-057: a reach's corridor holds the counted cells only: a store's tiles, or the window's box in window mode. A reach whose end lies outside them is skipped (the existing rule).
- TEST-137: a reach ending outside the counted cells is not counted, and a wall whose one gap is not counted is crossed at 60 m (fails if corridors are not restricted).

## Impact
cache (`river_suite.rs` `corridor`); tests. Window mode is unchanged, since its counted cells are the box. The 37-row D-1 table is re-run.
