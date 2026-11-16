## Why
- #161's redone stills: B's three worst R1h corridor failures (252, 241, 223 m) all start at a snapped node below sea level (-14, -17, -45 m). The snap took the lowest cell within one H1 cell, which at a coast is sea floor, so coastal land read as a rise above the floor. Lead, DEC explorer-5: a riverbed at the coast sits at sea level.

## What changes
- BUILD-057: the corridor measures route and snap on the water surface, max(z, sea level). The polyline trunk column is unchanged.
- TEST-137: a reach from the sea floor at -45 m onto land at 5 m passes the 10 m bar.

## Impact
cache (`river_gate.rs` `measure`); tests. The D-1 table's corridor columns and #161's corridor stills are re-run.
