## ADDED Requirements
### Requirement: VIEW-070 The viewer keeps at most a capped number of tiles resident
The viewer's streamer SHALL count its capacity as the smaller of the renderer's pool slots (STRM-006) and the resident-tile cap, 24,000 tiles by default (`--resident-cap N` overrides it). At the cap the streamer evicts by STRM-007; the draw set with its ancestors and the roots are never evicted. The cap is above R of the standard views (20,517 slots, 10 % of the mid tier), so with every tile of R resident eviction changes no pixel. The perf scenes keep the pool as their capacity. A window prints on closing `planet editor: resident tiles at most N, evicted M (cap C)`.

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E2), crusader's #124 diagnosis (resident tiles grew from 10,922 to 66,114 in a 25 min flight; stalls of 4 to 6.5 s), STRM-007

#### Scenario: A long flight
- **WHEN** the managed loop walks 60 stops, 600 m apart, with a capacity of twice the widest view plus 10, and again with a pool 100 times the widest view
- **THEN** the capped run never has more resident tiles or slots in use than the cap, shows no blank frame, evicts tiles and settles at the exact plan, and the pool run grows beyond the cap and evicts nothing

#### Scenario: The capacity rule
- **WHEN** the pool holds 205,170 slots, or 13,678, and the cap is 24,000
- **THEN** the capacity is 24,000, or 13,678

#### Scenario: Floor tiles that land late
- **WHEN** the clipmap shadow pass has floor tiles pending that were not resident in the renderer when the cube was written, and a frame brings no new floor set
- **THEN** the pending tiles are written alone and the cube is not required to be covered by them; a floor set that arrives is the whole cube and replaces them
