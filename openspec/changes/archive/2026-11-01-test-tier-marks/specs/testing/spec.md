## MODIFIED Requirements

### Requirement: TEST-076 The edit pair
`planet::editperf::pair_frames` SHALL draw, per debug view, the stale frame of an edit case at an altitude, the first frame after the edit as the editor makes it (the definition swapped, the edited tiles named, the REND-033 preview set), the exact frame once the refresh has settled, and a control frame after the edit is taken back, which SHALL equal the stale frame bit for bit. Each pair SHALL be judged by TEST-077 and its verdict reported, not asserted. The tier-C test over the smoke world SHALL assert the control and that the edit is seen in at least one view.

Verify: C
Status: active
Source: opus-planner (2026-10-30), owner test 2 (drag against rest), REND-033, REND-045, B4-4

Detail: The pair is a measurement of the preview, not a gate: a pair over the bound is a finding with its pictures in `target/review/edit-pair/`. The real-store scene with the lit frame, the horizon off and on and `pair.md` extends this requirement.

#### Scenario: A trustworthy pair
- **WHEN** a 50 m hill is edited at 200 m on the smoke world
- **THEN** the control equals the stale frame in every view and the edit changes more than 50 pixels in at least one

#### Scenario: A verdict over the bound
- **WHEN** the preview leaves more than a twentieth of the changed pixels in a view
- **THEN** the test still passes and the verdict is printed

#### Scenario: The pair over a real store (by hand)
- **WHEN** the ignored tests `gpu_edit_pair::the_pair_over_a_real_store` (the discrete GPU, `PLANET_ADAPTER=hardware`, and an unlocked desktop) and `…the_plan_over_a_real_store_is_reported_per_parameter_set` (CPU only) run with `PLANET_PAIR_STORE` naming a store
- **THEN** each case's pair and plan are printed, with the pictures in `target/review/edit-pair/`; this is run by hand for the owner's drag report, not by a tier, `gate` or `accept`, and the owner checks the pictures

### Requirement: TEST-079 The terrain scorecard run
The scorecard run SHALL compute, for each landform of the sites file, the seven descriptors (GEN-089 to GEN-092) on three rasters: the real tile's central 0.5-degree square and the generator's two matched sites (`world.field()` of the store), each 1,853 x 1,853 cells at 30 m on an east-north grid. Per site and descriptor it SHALL give a distance to the tile: Wasserstein-1 over percentiles p1 to p99 divided by the real p5-to-p95 range for continuous descriptors, L1 between class histograms for geomorphons, and the generator-to-real ratio for drainage density.

Verify: A, B
Status: active
Source: SPIKE-57 (seven descriptors, about 1 minute), SPIKE-55 (sites, 30 m windows)

#### Scenario: Distances
- **WHEN** two identical percentile sets and two sets shifted by a quarter of the real p5-to-p95 range are compared, and two class histograms with disjoint support
- **THEN** the distances are 0, 0.25 and 2

#### Scenario: SPIKE-55 reproduced
- **WHEN** the ignored test `the_scorecard_reproduces_spike_55` runs over the store (`cargo test -p planet --release --test it the_scorecard_reproduces_spike_55 -- --ignored --nocapture`)
- **THEN** each generator site's slope and 1 km and 5 km relief p50 and p99 lie within 2 % of SPIKE-55's generator rows (`crates/app/data/spike55-generator-rows.toml`); the real rows are re-stated on the central 0.5-degree square (N47E011 slope p50 is 23.7, not the whole tile's 19.3), not compared; without the store the check is NOT RUN, never a pass; it is run by hand (by the lane that changes the scorecard or the generator rows, or the owner), not by a tier, `gate` or `accept`
