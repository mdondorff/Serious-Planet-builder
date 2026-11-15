## Why
- The trunk column (#1532) measures H1's reaches along their own polyline. On M1 a third of them stand more than 50 m above their running minimum, up to 923 m. The E4 note (§3.3) lets KR route each reach inside a corridor of radius k_c x 814 m. Whether "KR never cuts" holds anywhere, or D-2 needs H1 to move trunks on mountains, depends on the corridor (lead #158).

## What changes
- BUILD-057: a "trunks in corridors" line and eight fields per k_c (1, 2, 4).
  - Each reach takes the minimax route inside its corridor, by an exact search: each cell keeps its highest reachable running minimum, highest first, ties by cell id (the maximin form of Dijkstra's search; the verifier caught a first version that kept the lowest).
  - The reaches over the 50 m bar are counted. Their minimax rise is bisected to 0.1 m.
- TEST-137: corridor fixtures (a partial dam, a full dam, two radii; a pit bypass that only the highest-minimum search finds; the same bits twice).

## Impact
cache (`river_suite.rs` `trunk_corridor`, `river_gate.rs` `GateParams::h1_cell_m`, `GateReport::trunk_corridors`, `corridor_line`, fields); tests.
