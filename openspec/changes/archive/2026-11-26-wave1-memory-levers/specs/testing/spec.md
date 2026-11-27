## ADDED Requirements
### Requirement: TEST-157 The lean wave-1 paths are checked bit for bit
Tier-A tests SHALL check GEN-192's four scenarios. Each SHALL be able to fail on a broken lever.

Verify: A
Status: active
Source: Lead 2026-11-26 (the RG park plan, step 3), GEN-192
Detail:
| Test | Fails when (the first and third checked by a mutation) |
|---|---|
| `bake_lean_memory::k5_with_the_in_flight_cap_is_k5` | a capped chunk is not written back |
| `bake_lean_memory::hydrology_on_initiation_bits_with_the_area_freed_is_hydrology` | a channel cell, basin or segment differs |
| `bake_lean_memory::the_partitioned_route_s_filled_surface_and_weighted_routing_are_the_resident_ones_on_b_s_grid` | a tile border changes the filled surface, a receiver, the order or the weighted area |
| `region_bake::the_region_front_on_the_lean_paths_is_the_resident_pipeline` | the initiation threshold of the bits differs from hydrology's (the 2,048-cell face; at 64 cells every cell is above it) |

#### Scenario: A broken write-back
- **WHEN** the in-flight cap's chunk is not written back before the next chunk gathers
- **THEN** `k5_with_the_in_flight_cap_is_k5` fails, naming the cap and the cells that differ
