## Context
- The layer is added to the field's base at runtime (GEN-112). The bake carved `BakeInputs::base` (the class source); today's stored bake is `global::sample` (the legacy generator) read C1: 2,064 m rms apart on the mountains tile of seed 14 (p5 -3,372 m, p95 +125 m), and `m + k (e - m)` with the relief channels (512-cell estimate) is still 2,061 m rms apart. A lattice of `BakeInputs::base` itself read C1 is 3.4 m rms (p95 5.5 m, max 28 m) at 611 m cells, 4.9 m at 1.2 km, 2.0 m at 305 m. So the layer belongs on a lattice baked from the class source, without the channels.

## Decisions
- No definition key: a path in a definition is machine-local; the options are the viewer's.
- The keys of kind-13 tiles hash the bake's parameters, which nothing stored says yet: `layer_tiles::bake_specs(world, shape)` derives them from the bake's shape (seed, pass-1 and pass-2 cells, tile, halo) exactly as `bake_layer` does, so a mismatch is the existing refusal naming both keys. When explorer's layer manifest records the shape the options become overrides.
- The viewer refuses `--erosion on` on a store that does not say it was baked from the class source (the planner has asked explorer to write `BakeInputs::id()` into the field bake's key or manifest and the layer's); until that marker exists `--erosion-unchecked` is the only way, and the message says it is a test of the plumbing, not a look.
- `with_layer` and `with_crease_band` need no relief channels: with none, the base is the lattice read at `k` 1 and `m` is not read; the keys and the id say so (`cpu-erosion-v1+band+layer`).
- The terrain's `id()` is the layered field's implementation id, so a repro bundle made with `--erosion on` is refused by a replay without it (and the reverse) instead of replaying other pixels.
- The status text: `describe()` and the title say "erosion layer", with the layer's folder, the bake shape, the seed and the sea level; the log of the run prints it.

- `--erosion on` upgrades the default eroded field (v1, a bilinear read of the lattice) to the C1 one (`cpu-erosion-v2`): the measured gap (3.4 m rms) is that of the C1 read, and a bilinear base would crease at every lattice cell under the layer. The id says so.
- The layer was carved on the class source at relief scale 1 and sea level 0 (`bake_layer`: `sea_level` 0.0): a definition with another relief scale or sea is refused (VIEW-063), because the keys would be distinct but the ground wrong.
- Hand-off to explorer (planner's standing rule: the writer of a format owns it): `bake_specs` is a reader-side copy of `bake_layer`'s derivation, pinned by a test against the derivation written out; `bake_layer` should call `bake_specs` (and its default `--planet-res` 3,072 is not the viewer's 98,304: a default bake is refused by a default viewer run, clearly), and the layer's manifest should record the shape, the global change range and the class-source marker (`BakeInputs::id()`), after which `--erosion-unchecked` and the shape options become overrides.
- ADR 0100 point 4 and ADR 0098 point 3 say "behind the relief switch"; the layer no longer needs the relief channels (planner, 2026-11-04). A one-line amendment is asked of the planner.
- `examples/base_gap.rs` and `examples/base_lattice_gap.rs` are the measurements behind the Context numbers (2,064 m, 2,061 m, 3.4 m; run on seed 14, tile 2/12/10); dev tools, no test.

## Risks
- On today's stores the layer lands on the wrong ground: the refusal is the guard.
- The probe at open decodes one tile (about a second at 3,072 cells).
