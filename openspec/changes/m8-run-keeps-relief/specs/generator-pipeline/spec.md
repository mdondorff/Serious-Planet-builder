## ADDED Requirements

### Requirement: GEN-065 The coupled run keeps the relief of its input
`CoupledParams::erode` SHALL select what the run does and SHALL be part of the key of the stored run (STRM-043). With it off, `run` SHALL return `coupled::filled_state`: the heights are the priority-flood filled surface of the input (a cell outside a depression keeps its input height bit for bit, a cell in one takes the level of the fill, the sea is untouched), the receivers are those of the jittered steepest descent on that surface and the drainage areas their accumulation (every receiver lower than its cell, the outlets draining the land area), in one pass, in the fixed order of the run, with `libm` (CON-14). A world's run SHALL have it off (`coupledworld::COUPLED_ERODE`) and SHALL not make the stationary uplift of GEN-063 then. With it on, the stream-power steps of GEN-040 SHALL run as they did.

Verify: A
Status: active
Source: planner (2026-10-25, after the rebuild: the stationary uplift did not hold the input on the real bake), ADR 0080 point 4, GEN-040, GEN-063

#### Scenario: A real reduced bake
- **WHEN** the global-coarse field baked at 256 cells and reduced to 64 per face edge runs without erosion for 80 steps
- **THEN** the heights are the filled surface, every cell outside a depression has its input height bit for bit and the median of |run - input| over land above 1000 m is 0, while the eroding run of the same input moves it by over 100 m (1,175 m measured), and the receivers are downhill and the outlets drain the land

#### Scenario: The key
- **WHEN** two specs differ only in `erode`
- **THEN** their keys differ
