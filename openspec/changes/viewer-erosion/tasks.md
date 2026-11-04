## 1. Tasks
- [x] 1.1 `layer_tiles::bake_specs` and `StoreLayer::open` (tier A in `cache/tests/layer_tiles.rs`: the derivation pinned, a layer under other keys refused)
- [x] 1.2 `with_layer` and `with_crease_band` without the relief channels, ids and keys; `ErosionSwitch`, `build_field_erosion`, `field_chain_erosion` (tier A in `generators/tests/runtime_layer.rs`, `crease_band.rs`)
- [x] 1.3 `WorldTerrain::with_erosion`, the options, the viewer hook (tier A in `app` tests: options, refusals, a tiny bake end to end)
- [x] 1.4a Reviews: `numerics-reviewer` and `spec-guardian` (applied)
- [ ] 1.4b `verifier`, prepush, PR
- [x] 1.5 (explorer's hand-off, design.md) `bake_layer` adopts `bake_specs`, the layer manifest (STRM-074) and the field source record (STRM-073): done by explorer (#1162, #1164)
- [x] 1.6 (change `viewer-class-store`) When explorer's marker and manifest land: check the marker instead of `--erosion-unchecked`, read the bake shape and the global change range from the manifest (the options become overrides)
