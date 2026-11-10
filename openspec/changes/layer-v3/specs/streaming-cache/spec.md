## ADDED Requirements
### Requirement: STRM-081 The local change L is a kind 13 record of layer version 3
The local change L of the new chain SHALL be stored as one record of kind 13 per tile: layer version 3, codec byte 3, the core's change in 0.25 m counts (row deltas, brotli) with K7's stats block (land cells, burned cells, clamped cells, the pits left after the pass) and a key that covers the base's identity, the identity of K7's other inputs, the tile grid, K7's parameters and the kernel versions. There SHALL be no fill channel. A record of the previous layer version SHALL be refused naming both versions, a tile stored under another key SHALL be refused and never mixed, and a damaged record SHALL be refused naming what was found.

Verify: A
Status: planned
Source: Architect D16 (inbox #115), item 14 (GEN-156), item 15

#### Scenario: Round trip and refusals
- **WHEN** a tile of L is written and read, and read under another base, parameters or version, and a payload is damaged
- **THEN** the core is within half a quantisation step with the stats equal and the clamped cells counted, a previous-layer record is refused with both versions in the message, and every other mismatch is refused or reads as absent as stated
