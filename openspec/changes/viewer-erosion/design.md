## Context
- The layer is added to the field's base at runtime (GEN-112). The bake carved `BakeInputs::base` (the class source); today's stored bake is `global::sample` (the legacy generator) read C1: 2,064 m rms apart on the mountains tile of seed 14 (p5 -3,372 m, p95 +125 m), and `m + k (e - m)` with the relief channels (512-cell estimate) is still 2,061 m rms apart. A lattice of `BakeInputs::base` itself read C1 is 3.4 m rms (p95 5.5 m, max 28 m) at 611 m cells, 4.9 m at 1.2 km, 2.0 m at 305 m. So the layer belongs on a lattice baked from the class source, without the channels.

## Decisions
- No definition key: a path in a definition is machine-local; the options are the viewer's.
- The keys of kind-13 tiles hash the bake's parameters, which nothing stored says yet: `layer_tiles::bake_specs(world, shape)` derives them from the bake's shape (seed, pass-1 and pass-2 cells, tile, halo) exactly as `bake_layer` does, so a mismatch is the existing refusal naming both keys. When explorer's layer manifest records the shape the options become overrides.
- The viewer refuses `--erosion on` on a store that does not say it was baked from the class source (the planner has asked explorer to write `BakeInputs::id()` into the field bake's key or manifest and the layer's); until that marker exists `--erosion-unchecked` is the only way, and the message says it is a test of the plumbing, not a look.
- `with_layer` and `with_crease_band` need no relief channels: with none, the base is the lattice read at `k` 1 and `m` is not read; the keys and the id say so (`cpu-erosion-v1+band+layer`).
- The terrain's `id()` is the layered field's implementation id, so a repro bundle made with `--erosion on` is refused by a replay without it (and the reverse) instead of replaying other pixels.
- The status text: `describe()` and the title say "erosion layer", with the layer's folder, the bake shape, the seed and the sea level; the log of the run prints it.

## Risks
- On today's stores the layer lands on the wrong ground: the refusal is the guard.
- The probe at open decodes one tile (about a second at 3,072 cells).
