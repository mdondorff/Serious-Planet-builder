## ADDED Requirements

### Requirement: STRM-011 Stage output keys
A staged field output that is stored SHALL be keyed by the first 16 bytes of BLAKE3 in `derive_key` mode (context `planet-builder stage key v1`, so it can never equal a height or mesh key) over a canonical encoding with its own format tag: the hash version, the field's version, stage tag, id, implementation id and halo (as bits, `-0.0` read as `0.0`), the scope (the whole body, or one tile; the tile with raw id 0 never aliases the global scope), the body id and the 128-bit hash of its parameters, the face mapping id, the output resolution, the read set of the definition as entries (nodes by path, then queries by kind and parameters, duplicates collapsed) and the 128-bit keys of the outputs of the fields it reads, by field id (ADR 0022, ADR 0023 point 10, ADR 0026 point 6). A change of any of them SHALL change the key, a version bump of one field SHALL re-key exactly its consumers, the order of inputs and reads SHALL NOT matter, a missing, extra or unknown input and a scope of the wrong kind for the field's stage SHALL be programming errors that panic, and the encoding SHALL have a format tag, a pinned layout and known answers that are the same on every platform.

Verify: A
Status: active
Source: CON-05, CON-07, CON-08, CON-14, ADR 0006, ADR 0022, ADR 0023, ADR 0026

#### Scenario: Re-keying
- **WHEN** one field's version or halo, or the resolution, face mapping, body, parameters or reads change
- **THEN** exactly the keys that depend on it change

#### Scenario: Scope and inputs
- **WHEN** a global field is keyed for a tile, a regional field globally, or an input is missing or extra
- **THEN** it panics

#### Scenario: Pinned layout
- **WHEN** the encoding of a small global field is written out byte by byte
- **THEN** it equals the encoding, and the keys of a three-stage example equal their known answers

