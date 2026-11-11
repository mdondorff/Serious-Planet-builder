## ADDED Requirements
### Requirement: STRM-080 The basin table is a stored kind
The lakes of the lake producer (GEN-163) SHALL be stored as one record of kind 19 per cube face, sorted by basin id, holding per lake the id, the floor and spill ids, the floor, spill and water levels, `q_in`, `q_out`, the area, the cells, the class and flags, and the tiles that hold its water. The key SHALL cover the table version, the producer version, the hydrology parameters, the tile grid, the base id and the identity of the other inputs (climate, intentional mask, routing rules), so that a table of another spec reads as absent. A record that is damaged or of another version or grid SHALL be refused naming what was found, and a decoder SHALL reserve no more than the payload allows. The encoder SHALL refuse what the decoder refuses (a number that is not finite, a dry lake with a level over its floor, tiles out of order or outside the grid).
- The runtime reads a table once and indexes it by tile; no per-tile copy is stored.

Verify: A
Status: planned
Source: Architect 2026-11-10, GEN-163, item 15

#### Scenario: Round trip and refusals
- **WHEN** a table is written, read, and read under another spec, version or grid, and a payload is damaged
- **THEN** the lakes come back in id order, another spec reads as absent, and every damaged or foreign record is refused with the found value in the message
