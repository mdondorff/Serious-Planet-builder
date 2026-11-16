## Why
- Review B F2 (lead #163): a confluence within one H1 cell (814 m) of a bay snapped to the water surface of the bay; the reach below it then read the climb out of the bay as a rise. Only a river's mouth meets the sea.

## What changes
- BUILD-057: non-mouth H1 nodes and edge crossings snap to land cells only (above the sea level); only a mouth (a reach's last node where no reach starts) may snap to the sea. Routes step over land only; a route that meets the sea before its last node ends there.
- TEST-138 (new): a confluence 450 m from a bay keeps its land snap and records no false rise; a route reaching a strait ends there.

## Impact
cache (`river_suite.rs` `trunk_corridor`, `river_gate.rs` `measure`); tests. The D-1 table's corridor columns are re-read (#163).
