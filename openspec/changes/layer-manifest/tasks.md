## 1. Layer manifest
- [x] 1.1 `LayerManifest` (STRM-074), tests in `cache/tests/layer_manifest.rs`
- [x] 1.2 `bake_layer` adopts `bake_specs`, writes the manifest, default `--planet-res` 98,304
- [ ] 1.3 verifier, prepush, PR; tell crusader when it is on main (`StoreLayer::open` and the viewer's key check read it)
