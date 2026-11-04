## Decisions
- The recorded source decides: no option names the source unless the store holds both (`for_store`'s refusal); the options are `--class-source` and `--legacy-source` as the offline readers take them.
- The layer's keys need the world of the store (the stage key differs between sources): `WorldTerrain` keeps the world it opened (leaked, as the store is) and `with_erosion` clones it into the layer.
- The marker refusal of viewer-erosion (`--erosion-unchecked`) is replaced by the world's source: a legacy world is refused, since the layer is carved on the class source's surface; the manifest's source must equal the world's whatever the flag says.
- A layer without a manifest is accepted with the production shape (a layer made before the manifest); its range is the representable one until it is re-baked or given a manifest.
- Not here: the viewer-side read of the drainage and deposition channels (I8a slice 2), the pre-fill and coupled run (explorer, offline).
