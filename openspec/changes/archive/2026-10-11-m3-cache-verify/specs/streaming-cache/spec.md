## ADDED Requirements

### Requirement: STRM-009 Height tile payload
A height tile SHALL be stored as a payload with a codec version, its resolution and the raw length, followed by an LZ4 block of its `(res + 1)^2` heights as little-endian `f32` bytes shuffled by byte index (ADR 0019). Decoding SHALL give back every `f32` bit pattern exactly (NaN payloads, signed zeros, subnormals), SHALL refuse a payload that is short, of another version, of a resolution that is not a power of two up to 4096, whose raw length does not match its resolution or whose block is damaged, and SHALL never allocate more than the resolution allows. Tiles SHALL be compared on decoded heights, never on compressed bytes.

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
`cargo xtask cache-verify <dir>` SHALL regenerate an evenly spaced sample (or all) of the entries of a tile store from the definition (`--world <file>` or the default planet with `--seed`) and compare each with what the store holds: an entry whose key is the key the current definition gives for its tile SHALL equal the regeneration sample for sample (bit for bit, judged on the decoded heights, not on the compressed bytes); a difference SHALL fail the command and be reported with the tile (id, face, level, x, y), the pack and its offset, the key and the first differing sample with both values. An entry under a key the current definition no longer gives SHALL be counted as stale, not failed; an entry of a kind the verifier does not regenerate SHALL be counted; an entry that cannot be read or decoded SHALL be reported with its pack and offset and fail the command. `generate --cache-dir <dir> --region FACE,LEVEL` SHALL write the tiles of one face at one level into a store, reusing tiles already stored under the current key so that a second run writes nothing.

Verify: A
Status: active
Source: CON-05, report §16, ADR 0006, ADR 0019

#### Scenario: Missing input in a key
- **WHEN** a cached tile was produced with an input that is not part of its key
- **THEN** cache-verify reports the tile ID, the pack offset and the first differing sample, and fails

#### Scenario: Clean store
- **WHEN** every sampled entry was generated from the current definition
- **THEN** all match and the command passes, whatever the sample size

#### Scenario: Changed definition
- **WHEN** the definition changed since the entries were stored
- **THEN** they are counted as stale and the command passes
