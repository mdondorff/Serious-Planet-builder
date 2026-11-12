## ADDED Requirements
### Requirement: GEN-180 The coast plains rise away from the sea
When the coast tilt is on (`CoastTilt::t_m_per_km` above 0), the base elevation `e0` of a land point SHALL gain `weight x t x D x s(d / D)` metres, with `d` the distance in km to the coast of the field (a lattice of its own with the cap `D`, not the shelf's), faded in by `smoothstep(0, 0.2, land depth)` so that the shore has no step, `s(x) = x (2 - x)` for `x` in `[0, 1]` and 1 beyond, and `weight` the sum of the plain and rolling-plain class weights. The added height SHALL NOT exceed `t x D`, SHALL be 0 at the coast, over the sea and where the weight is 0, and SHALL be added after the class weights are decided, so that no class of the class source moves. With the tilt off every `e0` and every key SHALL be the bits of the build before this requirement; with it on the inputs identity SHALL change with `t` and `D`.

Verify: A
Status: planned
Source: Architect, 2026-11-12 (SPIKE-109 verdict, coast plain; D114 form)

#### Scenario: A coast plain
- **WHEN** the tilt is on and a plain lies 100 km inland
- **THEN** its base is raised by at most `t x D` and by more than the plain 10 km inland, and the class weights are the ones without the tilt

#### Scenario: Details
- **WHEN** the rule is read
- **THEN** the slope of the added height is `2 t` at the coast and 0 at `D`
- **AND** the tilt is a process-wide switch like the band rules (GEN-179): a run sets it once at its start
- **AND** what reads `e0` sees the tilt when it is on: the orographic climate, the arid rules and the basin province term of the lithology (so the carve factor, the sand seas and the uplift change with it); the class source's own weights do not
