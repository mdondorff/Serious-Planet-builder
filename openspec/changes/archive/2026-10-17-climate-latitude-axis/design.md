## Design

**One definition of latitude.** `sin_latitude(d) = d.z` in COORD's axes, used by `sample` for both the temperature (`sin^2`) and the precipitation bands (`asin` to degrees). The unit tests pin it at +z, -z, +x and +y exactly and the field at the poles and the equator.

**Versions.** The channel values of the global bake change, so `GLOBAL_VERSION` moves 2 to 3 (ADR 0006). The climate version word folds `GLOBAL_VERSION` (ADR 0049 point 3), so the mesh keys move without a `MESH_VERSION` bump: the tile layout and the construction are the same, and a stored version 7 tile of the old field is unreachable because its key names the old word. `CLIMATE_VERSION` stays 1 for the same reason (the f16 texture construction and the reader did not change). Stored bakes (global blocks and the stage key) and the packs pre-filled from them are unreachable and are written again (item 8).

**Which pins move.** Everything that hashes temperature or precipitation or the version word: global known answers (only the temperature and precipitation words of the pointwise answers; the continent and elevation words are identical, which is the check that nothing but the climate moved), the grid hash, the reader pins, the climate and compact content hashes (compact: the fourth channel), the mesh keys, the terrain plan snapshots (the `mesh_key` of each node only).

**Goldens.** Debug views of the climate and the lit pictures move (declared change). They go through `golden-review`; escalations are listed, not blessed by hand.
