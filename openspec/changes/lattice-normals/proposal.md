## Why
ADR 0073 (accepted 2026-10-20): a tile's vertex normals came from four extra height evaluations per vertex, 80 % of the evaluations of a 32-cell tile and so of every refresh, dive and start-up load (SPIKE-23). The lattice already holds the heights one cell either side of every vertex. The normal is now the cross product of the centred differences of the lattice neighbours' positions: 1,222 evaluations per 32-cell tile instead of 5,446.

## What changes
- `planet_core::TileId::ring_direction` and `corner_stencil`: the one-sample ring around a tile's lattice (a ring point beyond a face edge is the neighbour tile's own lattice direction one cell inside it, found on the exact integer cube point) and the canonical triple of edge points at the eight cube corners.
- `planet_generators::normals` (`lattice_directions`, `lattice_normals`): the one definition for the CPU mesh and the compact tile. The old stencil functions (`normal_stencil`, `normal_from_stencil`, `surface_normal*`, the tangent frame) are removed; they had no other caller.
- `MESH_VERSION` 9: every tile key, persistent mesh key, pre-filled pack key and committed known answer is made again once (mesh hashes, compact hashes, the stress-field answer, the key pins in `cache/tests/keys.rs`, `generators/tests/body_keys.rs`, the app manifest test, the plan snapshots, the per-driver pixel hashes of `gpu_stamps` and `gpu_compact_blocks`). A pre-filled pack is made again with `generate --prefill-dir`.
- Specs: GEN-010 (version 9), GEN-019 (the canonical normal, three scenarios), GEN-020 (the 1,222 directions), GEN-025 and the renderer's pixel-hash scenario (version 9). ADR 0033's status line gets "point 4 refined by ADR 0073".
- Tests (tier A): the ring and the corner stencils against the neighbour tiles' lattices (`core/tests/lattice_ring.rs`); the evaluation count; the lattice normal against a converged normal on a smooth field (inside a face 0.005 degrees, at a face edge its own bound of 3 degrees); the shared vertices bit for bit (compact words, CPU mesh and f64 bits) inside a face, across the twelve cube edges and at the eight corners at levels 3, 12 and 22, on one and three threads, over the eroded and the steep field; the outward orientation as an assertion.

The lit, sky and water goldens that move (`lit_*`, `sky_*`, `water_class_*`) are not blessed by the tool: they go to the owner's `/bless` with the hillshades of SPIKE-24 as evidence (ADR 0073 consequences, ADR 0017). The debug golden `terrain_cube_corner_normals` and the shading golden `lit_orbit_lighting_ev15` are declared changes. A pre-filled pack and the 16384-cell store's tiles are made again with `generate --prefill-dir` (STRM-021, tracked in `docs/status.md`). The saving (about 4.5 times fewer evaluations per tile) is an estimate until a perf window.

Expected golden changes:
- terrain_cube_corner_normals: the normal colours of the cube-corner view, 272 pixels on WARP, max delta 1 (the lattice normal differs from the stencil normal by a mean of at most 0.05 degrees)
- lit_orbit_lighting_ev15: the lighting view of the orbit, 1 pixel on WARP, max delta 1
- lit_orbit_ev15: the lit orbit, 14 pixels on WARP, max delta 1
- lit_aerial20km_ev15: the lit view at 20 km, on the discrete GPU only
- sky_orbit_ev15: the orbit with the sky, 6 pixels on WARP, max delta 1
- water_class_orbit: the water classes of the orbit, 12 pixels on WARP, max delta 1
- water_class_lava_lake: the water classes of the lava lake, on the discrete GPU only

Bound tests:
- terrain_cube_corner_normals: mesh::border_normals_match_across_every_cube_edge_and_within_faces, lattice_normals::tiles_sharing_a_vertex_compute_bit_identical_normals_inside_a_face_across_the_cube_edges_and_at_the_cube_corners_on_one_and_three_threads, compact::a_tiles_stored_normals_equal_the_cpu_normal_within_the_bound
- lit_orbit_lighting_ev15: lattice_normals::the_lattice_normal_equals_a_converged_normal_on_a_smooth_field_inside_a_face_and_at_the_edges_within_their_own_bound, compact::a_tiles_stored_normals_equal_the_cpu_normal_within_the_bound
- lit_orbit_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, lattice_normals::the_lattice_normal_equals_a_converged_normal_on_a_smooth_field_inside_a_face_and_at_the_edges_within_their_own_bound
- lit_aerial20km_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, lattice_normals::the_lattice_normal_equals_a_converged_normal_on_a_smooth_field_inside_a_face_and_at_the_edges_within_their_own_bound
- sky_orbit_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound
- water_class_orbit: lattice_normals::the_lattice_normal_equals_a_converged_normal_on_a_smooth_field_inside_a_face_and_at_the_edges_within_their_own_bound
- water_class_lava_lake: lattice_normals::the_lattice_normal_equals_a_converged_normal_on_a_smooth_field_inside_a_face_and_at_the_edges_within_their_own_bound
