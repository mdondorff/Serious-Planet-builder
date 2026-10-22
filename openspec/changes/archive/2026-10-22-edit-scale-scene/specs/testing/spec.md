## ADDED Requirements

### Requirement: TEST-045 The edit-scale scene
`planet test-render --scene edit-scale --store DIR` SHALL make synthetic item sets of the counts named by `PLANET_SCALE_COUNTS` (default 100, 1,000, 5,000 and 20,000) from `PLANET_SCALE_SEED` through the editor session (every put pays what an edit pays) into `PLANET_SCALE_DIR/n<count>` as item files, and for each set measure on the CPU (no GPU context, no readiness gate, indicative): the load of the files into an editor with time and working set, the clone and swap of the definition, the field's `height`, `height_bounds` and `inputs_hash_for` per call over the cluster and over empty ground, the plan job cold and warm at 2 km, 200 m and 30 m over both, the generation of tiles in the cluster, and the editor's lists and frame with the mode on. The sets SHALL be a function of the seed (the same counts, kinds and strokes for the same seed), 80 % of the actions in a disc of 25 km round the place the edit-latency scene edits and the rest over the planet, a mix of hills, craters, plateaus, ridges, channels, regions and brush piles of 1 to 64 strokes (`PLANET_SCALE_NOBRUSH` leaves the brushes out). `PLANET_EDIT_ITEMS=DIR` SHALL load a set into the definition of the edit-latency scene. A set that exists is reused.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29: the owner asked how far authored edits scale), ADR 0066 (the revisit trigger of 20,000 items), TEST-041

#### Scenario: A set is a function of the seed
- **WHEN** 60 actions are made in an in-memory editor twice with the same seed, and once with another
- **THEN** the first two sets have the same kinds, counts and strokes, the other differs, and the sets hold at least four kinds of edit

#### Scenario: The mix and the cluster
- **WHEN** 10,000 kinds and 4,000 places are drawn
- **THEN** the shares of the kinds are within two points of the declared mix (hills 22 %, craters 18 %, plateaus 12 %, ridges 8 %, channels 5 %, regions 5 %, brush piles 30 %), and 80 % of the places (within one point) are within 25 km of the cluster centre

#### Scenario: The counting field
- **WHEN** a field is wrapped by the scene's counting field and asked for heights, bounds and hashes
- **THEN** the values are those of the field and the counts and times of each kind of call are reported
