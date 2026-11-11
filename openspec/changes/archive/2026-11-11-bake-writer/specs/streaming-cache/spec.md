## ADDED Requirements
### Requirement: STRM-082 The bake writer commits the L tiles and the basin tables together
The bake writer SHALL write the L tiles of a set of tiles (each with its lake water mask in the water section), the basin tables of the six faces and then a commit record, and a reader SHALL read them only through that commit.
- The commit record is a text file beside the layer manifest, written last and atomically; it names the L spec (so a reader forms the tiles' keys), the commit id and the key of every tile and table. The id is a hash of the spec and the keys.
- A commit that names a key other than the one the reader's spec makes is refused naming both keys; a damaged record or an id that does not match its keys is an error.
- A bake that fails or is refused before its commit leaves the previous commit current.
- The channel feed carries `Q_w` in m3/s; a feed in area units, a halo under the ring's depth and producer lists that are not sorted and disjoint are refused before any record.
- The records do not depend on the thread count (CON-14).

Verify: A
Status: planned
Source: Architect D17/D18/D33/D34, design section 14

#### Scenario: Round trip and refusals
- **WHEN** a small face is baked and read back through its commit, read under another spec, and baked with a feed in area units
- **THEN** every tile, mask and table reads back, the other spec is refused naming both keys and the area feed writes nothing
