## Why
- The trunk column (#1532) measures H1's reaches along their own polyline. On M1 a third of them stand more than 50 m above their running minimum, up to 923 m. The E4 note (§3.3) lets KR route each reach inside a corridor of radius k_c x 814 m. Whether "KR never cuts" holds anywhere, or D-2 needs H1 to move trunks on mountains, depends on the corridor (lead #158).

## What changes
- Lead #159 (the owner, 2026-11-15) replaces #158's scope: k_c 2 is the requirement; a per-window bar `min(50, max(10, 0.5 x start relief))`; a snapped form beside the raw one (nodes to their lowest cell within one H1 cell, edge crossings at the lowest edge cell); the share of reaches passing is reported, no pass share is set.
- BUILD-057: a "trunks in corridors" line and eight fields per k_c (1, 2, 4).
  - Each reach takes the minimax route inside its corridor, by an exact search: each cell keeps its highest reachable running minimum, highest first, ties by cell id (the maximin form of Dijkstra's search; the verifier caught a first version that kept the lowest).
  - The reaches over the 50 m bar are counted. Their minimax rise is bisected to 0.1 m.
- TEST-137: corridor fixtures (a partial dam, a full dam, two radii; a pit bypass that only the highest-minimum search finds; the same bits twice).

## Impact
cache (`river_suite.rs` `trunk_corridor`, `river_gate.rs` `GateParams::h1_cell_m`, `GateReport::trunk_corridors`, `corridor_line`, fields); tests.
