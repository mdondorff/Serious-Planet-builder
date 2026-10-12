## ADDED Requirements

### Requirement: STRM-009 Height tile payload
A height tile SHALL be stored as a payload with a codec version, its resolution and the raw length, followed by an LZ4 block of its `(res + 1)^2` heights as little-endian `f32` bytes shuffled by byte index (ADR 0019). Decoding SHALL give back every `f32` bit pattern exactly (NaN payloads, signed zeros, subnormals), SHALL refuse a payload that is short, of another version, of a resolution that is not a power of two up to 4096, whose raw length does not match its resolution or whose block is damaged, and SHALL never allocate more than the resolution allows. The encoder SHALL refuse a tile the decoder would refuse (a resolution that is not a power of two up to 4096, a number of heights that does not match). Tiles SHALL be compared on decoded heights, never on compressed bytes.

Verify: A
Status: active
Source: CON-05, CON-14, ADR 0019

#### Scenario: Round trip
- **WHEN** a tile is encoded and decoded
- **THEN** every height has the same bits, and a smooth tile is smaller than its raw size

#### Scenario: Layout
- **WHEN** a hand-built payload of the documented layout is decoded
- **THEN** it gives the expected heights, independent of the compressor in use

#### Scenario: Hostile payload
- **WHEN** a payload is truncated, has another version, resolution or raw length, or a damaged block
- **THEN** decoding fails with an error naming what is wrong

## MODIFIED Requirements

### Requirement: STRM-004 Cache verification
`cargo xtask cache-verify <dir>` SHALL regenerate an evenly spaced sample (or all) of the entries of a tile store from the definition (`--world <file>` or the default planet with `--seed`; both together are refused) and compare each with what the store holds, and SHALL refuse a directory that does not exist. Every entry SHALL be decoded first, whatever its key: an entry that cannot be read or decoded, and every record the pack scan found unusable, SHALL be reported with its pack, offset and key and fail the command. An entry whose key is the key the current definition gives for its tile SHALL carry the generator version, hash version and node set id the definition gives and SHALL equal the regeneration sample for sample (bit for bit, judged on the decoded heights, not on the compressed bytes); a difference SHALL fail the command and be reported with the tile (id, face, level, x, y), the pack and its offset, the key and either the disagreeing metadata or the first differing sample with both values. An entry under a key the current definition no longer gives SHALL be counted as stale: *explained* when its versions or node set id differ from the current ones, *without a known reason* when they are all equal (a node value changed, or the key function drifted); stale entries are not failures, but a run in which entries were looked at and none could be compared (all stale) SHALL fail unless `--allow-stale` says the definition changed on purpose. Entries of a kind the verifier does not regenerate SHALL be counted. `generate --cache-dir <dir> --region FACE,LEVEL` SHALL write the tiles of one face at one level (at most level 6, at most 4 GiB of raw tiles, a resolution the codec accepts) into a store, reusing tiles already stored under the current key whose payload decodes, so that a second run writes nothing. The store options SHALL be refused by every other mode and when combined with options they would silently ignore.

Verify: A
Status: active
Source: CON-05, report §16, ADR 0006, ADR 0019

#### Scenario: Missing input in a key
- **WHEN** a cached tile was produced with an input that is not part of its key, and that input's value differs between building and verifying (or the record was written wrongly)
- **THEN** cache-verify reports the tile ID, the pack offset and the first differing sample (or the disagreeing metadata), and fails

#### Scenario: Clean store
- **WHEN** every sampled entry was generated from the current definition
- **THEN** all match and the command passes, whatever the sample size

#### Scenario: Changed definition
- **WHEN** the definition changed since the entries were stored
- **THEN** they are counted as stale (explained or not), and the command fails because it compared nothing unless `--allow-stale` is given or other entries do match

#### Scenario: Damage
- **WHEN** a payload fails its CRC, is damaged inside a stale entry, or the pack scan skipped a record
- **THEN** it is reported with pack, offset and key and the command fails

#### Scenario: Incremental invalidation
- **WHEN** one authored item is edited and the region is built again
- **THEN** exactly the tiles whose footprint and margin hold its cell are written and the rest are reused
