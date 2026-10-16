## ADDED Requirements

### Requirement: STRM-021 Pre-filled coarse tiles
The bake step SHALL also write the finished render tiles of levels `0..=N` (default `N = 4`, 2,046 tiles; `--prefill-levels N`, 0 disables; at most 8) into the tile pack of the bake, as records of kind 2 (`Kind::Mesh`) with the existing record format and the ADR 0018 mesh key of the tile (`mesh_key_inputs`: field, tile, cells, skirt fraction and `MESH_VERSION`) plus one output parameter naming the form of the stored tile (vertex or compact; levels 0-2 are always the vertex form, so those records serve both forms), so that no existing key, record format or version changes. The payload SHALL be the tile exactly as the generators build it, every `f32`, `u16` and `f64` bit (`codec version | form | cells | raw length | LZ4 block`), and SHALL decode to a tile equal to the generated one; a payload that is short, of another version or form, of cells that are not a power of two from 2 to 1024, of a wrong raw length or damaged SHALL be refused naming what is wrong, and the decoder SHALL NOT allocate more than the cells allow. A pre-filled tile SHALL equal the tile the streamer generates on demand for the same field, cells, finest level and form, bit for bit, whatever the number of threads that wrote it (CON-14): tiles are generated on any number of threads, coarsest first, and appended in the fixed order level, face, row, column, so the bytes of the pack do not depend on the thread count. A tile at or below the run's finest level SHALL NOT be stored or looked up (its format depends on its relief, CON-13), and a pre-fill that reaches it, or goes deeper than 8, SHALL be refused before it writes. The pre-fill SHALL be idempotent: a tile the pack already holds under the current key whose payload decodes is skipped, a run that finds none missing opens no writer and changes no byte or file time, a deeper run adds only the new levels, and a record that does not decode counts as missing and is written again (the newer record wins). `generate --global-dir D` SHALL pre-fill after the bake and print the tiles, payload bytes, growth of the packs and wall time; `generate --prefill-dir D --global-res N [--prefill-levels L] [--threads T] [--cells C] [--finest-level F] [--tile-format F]` SHALL pre-fill an existing bake in place and refuse a store that holds no bake of that definition and size. The streamer's tile source (`MeshSource`) SHALL take a tile of level 8 or coarser from the pack when the pack holds it under the current key and generate it otherwise (a cache degrades to a miss: a damaged record, another version, another field, other cells or the other form is a generation, never an error), and a world opened `cold` SHALL never read a tile from the pack (the cold reader of `dive-cold`, TEST-019). Pre-filled tiles are caches evicted with their bake (ADR 0045 point 1) and are not read by `verify-cache` or `verify-global`, which report them as records of another kind.

Verify: A
Status: active
Source: owner (2026-10-16, option A), ADR 0045 point 3, ADR 0018, ADR 0019, STRM-008, STRM-003, CON-05, CON-13, CON-14, M4 fix queue (F9)

#### Scenario: Codec
- **WHEN** a vertex tile (level 2) and a compact tile (level 4) are encoded and decoded, and hostile payloads are decoded
- **THEN** every bit of the tile comes back, and a short payload, another version or form, bad or huge cells, a wrong raw length and a damaged block are each refused naming the cause without allocating by the claim

#### Scenario: Keys
- **WHEN** the key of a tile is made for another field, other cells, the other form or another tile
- **THEN** it differs, while levels 0-2 give the same key and the same tile in both forms

#### Scenario: Idempotent and thread-independent
- **WHEN** levels 0-3 are pre-filled on one thread and on five, the pre-fill is run again, and a deeper one is run
- **THEN** the pack files of the two runs are byte-identical, every stored tile equals the generated one, the second run generates and writes nothing and touches no file, and the deeper run adds only the new levels

#### Scenario: Limits and misses
- **WHEN** a pre-fill reaches the run's finest level or goes deeper than 8, and a store pre-filled for one field is read for another seed, other cells or a finest tile
- **THEN** the first is refused before writing anything and the others are misses

#### Scenario: Damaged record
- **WHEN** a record under a tile's key holds a payload that does not decode
- **THEN** the tile counts as missing, a repeated pre-fill writes it again and the newer record is the one read

#### Scenario: The streamer reads the pack
- **WHEN** a bake of an eroded-field world is pre-filled to level 3 through `generate`, and the streamer's tile source loads tiles of levels 0-3 and a level-5 tile
- **THEN** the level 0-3 tiles are taken from the pack and equal the generated ones (also through the staged, cancellable path), the level-5 tile is generated, a cold reader of the same store reads none from the pack and builds the same tiles, and the vertex form finds only levels 0-2

#### Scenario: The commands
- **WHEN** `generate --global-dir` bakes with `--prefill-levels 1`, `--prefill-dir` pre-fills an existing bake to level 1, then 2, then 2 again, and the options are misused
- **THEN** the reports count 30 then 126 tiles with 30, 96 and 0 written, the last run grows the packs by 0 bytes, `--prefill-levels 0` is disabled, and a missing `--global-res`, a store of another size, a depth over 8, a finest level inside the pre-fill and a clash with another command are each refused naming the cause
