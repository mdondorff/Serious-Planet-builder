## ADDED Requirements

### Requirement: STRM-074 The layer has a manifest
The erosion layer's store directory SHALL hold a manifest, one plain text file (`layer-manifest.txt`), from which a reader rebuilds the keys of the layer's records and bounds its pyramid without options:
- the first line is `layer-manifest 1`; then one `key value` line each for the seed, the radius, pass 1's and pass 2's cells a face edge, the tile and the halo (both resolved), the versions of the kernels and records (`bake_kernel_version`, `pass2_kernel_version`, `tile_layer_version`, `layer_version`, `channels_version`), the quantisation step of the change, the field source (`legacy`, or `class` and the class source's identity, STRM-073), the pyramid's block size (cells a side of a block of level 0) and the range of the change (`change_min_m`, `change_max_m`; absent while no tile is recorded);
- an `f64` is written as its decimal value then its bits in hex and read back from the bits, so every value round-trips exactly; a manifest that is not of this layout, repeats a key, lacks one or holds one end of the range is an error naming the file and what is wrong (the key, or the line that is not `key value`);
- it is written atomically: a temporary file in the directory, then a rename over the manifest, so a killed run or two runs leave no half manifest (a temporary file of a process killed between the write and the rename may remain and is never read; runs are not locked against each other, so two simultaneous runs can lose a widening: runs of one layer are sequential);
- a run adds to the manifest that is there: the shape, the versions and the source must be equal, else an error naming both texts (another layer's run is never mixed), and the range only widens, so after a planet run it is exactly the range of the layer's tiles;
- a reader compares the versions with its build's (`check_current`): each version that differs is named, and a layer of another version is refused, never read;
- the layer's command (`bake layer`) writes it after each run, over the tiles of the run, and its default pass-2 grid is the production one (98,304 cells a face edge) the viewer expects.

Verify: A
Status: planned
Source: owner 2026-11-04 (planner inbox, change `layer-manifest`); STRM-065, STRM-066, STRM-072, STRM-073

#### Scenario: The manifest round-trips
- **WHEN** a manifest of the production shape is made for a legacy and for a class world, with and without a range, written as text and parsed
- **THEN** the tile and the halo are resolved (3,072 and 128), the versions are this build's, the pyramid block size is the pyramid's, and every value (including a range that is not a short decimal) comes back equal to the bit

#### Scenario: Runs merge
- **WHEN** three runs with ranges [-100, 50], [-20, 400] and [-30, 40] are merged into one directory
- **THEN** the manifest says [-100, 400] after the second and the third, and the directory holds the manifest and no temporary file

#### Scenario: Another layer is refused
- **WHEN** a run of another pass-2 grid, another pass-2 kernel version or the other field source is merged, and a manifest with another bake kernel version and channels version is checked, and damaged manifests are read
- **THEN** each merge is an error naming both texts and writes nothing, the check names each version that differs, and each damaged manifest is an error naming the file and what is wrong
