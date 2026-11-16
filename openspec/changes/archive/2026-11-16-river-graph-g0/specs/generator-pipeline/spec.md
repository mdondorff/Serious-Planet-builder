## ADDED Requirements
### Requirement: GEN-189 The canonical river graph
The bake SHALL describe rivers as one canonical graph over two levels (`bake::river_graph`): nodes, reaches and basins, owned by domains, identified by hashes of their defining fields, serialised canonically and checked by validators that report and never repair. The graph is data and checks only. The algorithms that build it (KS, KG, KV, KC) are later requirements.

Verify: A
Status: active
Source: Lead 2026-11-16 (crusader #155), the river-graph plan's G0 contracts (agreed, owner 2026-11-16) sections 1 to 8 and the readings of section 12 (lead, 2026-11-16)
Detail:

| Item | Rule |
|---|---|
| Levels | H1 (814 m global cells: trunk reaches from 75 km2, H1 basins) and L (101.8 m global cells: trunk images, tributaries, L basins) |
| Domains | `landmass` (H1, every trunk reach at both levels, H1 basins and their L images) or `tile(face, ti, tj)` of K7's canonical grid (GEN-182); a region, window, store or halo is never a domain and never enters an id |
| Node | `id`, `kind`, `level`, `anchor` (global cell id at its level), `parent` (the H1 node of an L trunk image), `owner`; for a marker, also `on_reach` and `at_vertex` |
| Node kinds | `source`, `confluence`, `trib_junction` (marker), `exception` (marker), `tile_crossing`, `outlet_sea` (the first sea cell), `lake_inlet`, `lake_spill` |
| Reach | `id`, `level`, `up`, `down`, `vertices` (an 8-connected chain of global cell ids from `up`'s anchor to `down`'s, both included), `parent` (the H1 reach of an L trunk image), `owner`, per-vertex attributes, `status` (`ok` or `exception(reason, value)`; reasons `rise_over_budget`, `unresolved_endpoint`, `budget_residual`) |
| Attributes | per vertex: `a_geo`, `a_ext`, `a_run` (with and without `a_ext`), `q`, `q_w` (f64), `strahler`, `bed` (codes of 0.5 m, GEN-170) |
| Basin | `id`, `level`, `parent`, floor (cell, height), spill (cell, height, receiving node: a `trib_junction` or a `lake_inlet`), outlet `active` or `endorheic`, represented `protected_surface` or `none`, `owner`; kept: H1's set and L basins from 0.25 km2 and 2 m |
| Ids | node `hash(body, kind, level, anchor)`; reach `hash(body, level, up, down)`; basin `hash(body, level, floor anchor)` (GEN-145's mixer); ownership never enters an id |
| Uniqueness | at most one non-marker node and one marker per (level, anchor); a marker shares its anchor only with the `up` node of the reach it sits on, at vertex 0 |
| Degrees | every node but an outlet or a marker has exactly one outgoing reach; a confluence has two or more incoming reaches; every chain ends in one outlet |
| Tile crossings | a tributary chain leaving a tile core is cut at a `tile_crossing` on its last vertex inside that core; trunk reaches are never cut at tiles |
| Bed | non-increasing on codes along a reach except at an `exception` marker's vertex; one value at a confluence, shared by its reaches |
| Conservation (graph) | `a_geo`, `a_run`, `q`, `q_w` never fall along a reach; at a confluence every reach carries one shared value, at least the sum of the inflows one vertex upstream (1e-9 relative); the exact per-cell rule stays GEN-145's |
| Rasterisation (L) | a cell belongs to one reach: the reach it starts (a junction cell), else the larger `q_w`, ties by the lower reach id |
| Budget | per cell `budget = min(50, max(10, 0.5 relief10))` m against `S0`; stages KS, KV, KC lower `max(0, in - out)` in order, within the remainder; the fills of KV and KC each at most `budget`; a clamp writes a residual `(cell, stage, needed, available, feature)`, and the feature is `exception(budget_residual)`; checked on f64 |
| Canonical form | nodes, reaches, basins sorted by id; fixed field order; f64 as IEEE-754 bit patterns in hex; parsed back to the same graph |
| Diff | ids added or removed (topology), apart from the same id with other fields (attribute); each named by (id, field) |
| Slices | a slice holds the items its domains own; an endpoint owned elsewhere is a store fact (`missing owner`), not a violation |

#### Scenario: Validators report, never repair
- **WHEN** a graph breaks a rule of the contract's section 8
- **THEN** the validator returns that violation, names its id, and leaves the graph unchanged
