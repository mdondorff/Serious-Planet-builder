## Why
- I8a slice 2b (planner, 2026-11-05): the ground materials read the baked channels, drainage area (kind 14, GEN-110) and deposition and erosion depth (kind 15, GEN-111; read API `LayerChannels`, STRM-072, #1157), per pixel. This change is the design notes only, sent to the planner before any code; D1 (the keep-pits fill-depth channel, SPIKE-102) is an open branch.

## What Changes
- Nothing in code or specs. `design.md` states the decisions the implementation PRs follow: the per-tile carrier, the reader placement and lifetime, the format and memory per tile, eviction, the keys, the relation to M9 I7, and the questions for the owner lane.
