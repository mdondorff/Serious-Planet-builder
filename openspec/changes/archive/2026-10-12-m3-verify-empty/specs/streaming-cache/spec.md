## MODIFIED Requirements

### Requirement: STRM-004 Cache verification
`cargo xtask cache-verify <dir>` SHALL regenerate an evenly spaced sample (or all) of the entries of a tile store from the definition (`--world <file>` or the default planet with `--seed`; both together are refused) and compare each with what the store holds, and SHALL refuse a directory that does not exist. Every entry SHALL be decoded first, whatever its key: an entry that cannot be read or decoded, and every record the pack scan found unusable, SHALL be reported with its pack, offset and key and fail the command. An entry whose key is the key the current definition gives for its tile SHALL carry the generator version, hash version and node set id the definition gives and SHALL equal the regeneration sample for sample (bit for bit, judged on the decoded heights, not on the compressed bytes); a difference SHALL fail the command and be reported with the tile (id, face, level, x, y), the pack and its offset, the key and either the disagreeing metadata or the first differing sample with both values. An entry under a key the current definition no longer gives SHALL be counted as stale: *explained* when its versions or node set id differ from the current ones, *without a known reason* when they are all equal (a node value changed, or the key function drifted); stale entries are not failures, but a run in which nothing could be compared (an empty store, only entries of another kind or all entries stale) SHALL fail, unless `--allow-stale` says the definition changed on purpose and at least one entry is stale (ADR 0025 point 4). Entries of a kind the verifier does not regenerate SHALL be counted. `generate --cache-dir <dir> --region FACE,LEVEL` SHALL write the tiles of one face at one level (at most level 6, at most 4 GiB of raw tiles, a resolution the codec accepts) into a store, reusing tiles already stored under the current key whose payload decodes, so that a second run writes nothing. The store options SHALL be refused by every other mode and when combined with options they would silently ignore.

Verify: A
Status: active
Source: CON-05, report §16, ADR 0006, ADR 0019, ADR 0025

#### Scenario: Missing input in a key
- **WHEN** a cached tile was produced with an input that is not part of its key, and that input's value differs between building and verifying (or the record was written wrongly)
- **THEN** cache-verify reports the tile ID, the pack offset and the first differing sample (or the disagreeing metadata), and fails

#### Scenario: Clean store
- **WHEN** every sampled entry was generated from the current definition
- **THEN** all match and the command passes, whatever the sample size

#### Scenario: Changed definition
- **WHEN** the definition changed since the entries were stored
- **THEN** they are counted as stale (explained or not), and the command fails because it compared nothing unless `--allow-stale` is given or other entries do match

#### Scenario: Nothing to compare
- **WHEN** the store is an existing empty directory, has only entries of another kind or only packs that are skipped
- **THEN** the command fails because it compared nothing, with or without `--allow-stale` (no entry is stale)

#### Scenario: Damage
- **WHEN** a payload fails its CRC, is damaged inside a stale entry, or the pack scan skipped a record
- **THEN** it is reported with pack, offset and key and the command fails

#### Scenario: Incremental invalidation
- **WHEN** one authored item is edited and the region is built again
- **THEN** exactly the tiles whose footprint and margin hold its cell are written and the rest are reused

## ADDED Requirements

### Requirement: STRM-010 Node set id
Every record of a world SHALL carry a node set id: the first 16 bytes of BLAKE3 `derive_key` (context `planet-builder node set v1`) over the paths of the field's field-wide reads and of the body's radius node (`body.0.radius_m`), deduplicated, sorted by UTF-8 byte order and concatenated, each as its byte length (u32 little endian) followed by its UTF-8 bytes, with no count prefix (ADR 0025 point 3). It SHALL depend on which nodes are read and not on their values, SHALL be equal for the records of one world, and its encoding SHALL have a pinned known answer that is the same on every platform. It is per world until a cell-reading field is stored; then it SHALL become per tile (ADR 0025 point 3).

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019, ADR 0025

#### Scenario: Pinned encoding
- **WHEN** the node set id of the default planet is computed by hand from its sorted paths and compared with the library's
- **THEN** they are equal and equal to the pinned known answer, another seed gives the same id, and every stored record carries it

