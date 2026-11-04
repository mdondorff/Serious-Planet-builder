## Decisions
- The recorded source decides: no option names the source unless the store holds both (`for_store`'s refusal); the options are `--class-source` and `--legacy-source` as the offline readers take them.
- The layer's keys need the world of the store (the stage key differs between sources): `WorldTerrain` keeps the world it opened (leaked, as the store is) and `with_erosion` clones it into the layer.
- The marker refusal of viewer-erosion (`--erosion-unchecked`) is replaced by the world's source: a legacy world is refused, since the layer is carved on the class source's surface; the manifest's source must equal the world's whatever the flag says.
- A layer without a manifest is accepted with the production shape (a layer made before the manifest); its range is the representable one until it is re-baked or given a manifest.
- Not here: the viewer-side read of the drainage and deposition channels (I8a slice 2), the pre-fill and coupled run (explorer, offline).
- For explorer (STRM-074's side): the manifest records `quant_step_m` and the pyramid block size, and `check_current` does not compare them with this build; a reader cannot tell a layer of another step from one of this build. The reader does not change the format; the planner is told.
- `open_recorded` lives in `cache::layer_tiles` beside `StoreLayer` (a reader; the manifest, `for_store` and `bake_specs` are untouched); the only edit in a file of explorer's besides is the derive of `Clone` on `GlobalWorld`, which clones plain data and the class source's `Arc`.
