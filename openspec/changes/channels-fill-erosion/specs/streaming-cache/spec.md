## MODIFIED Requirements
### Requirement: STRM-075 The channel reader owns its handles, reads the area through windows and caches under byte caps
The channel reader (`LayerChannels`, STRM-072) SHALL own its store and world handles and implement the generators' `ChannelSource`: the area as log2 of cells and, in metres, the deposited depth, the pit depth (kind 17, STRM-076) and the erosion depth, `max(0, deposited - (change + pit))` per cell. It SHALL read the area through 256-cell windows streamed from the record, never holding a face whole, with the codes of the whole face. It SHALL hold windows and tiles under byte caps that keep the entry just decoded, and decode a window or tile once for workers asking together. The decode that skips the key check SHALL be a diagnostic no viewer or generation code calls.

Verify: A
Status: active
Source: planner inbox #65 and #67 (2026-11-05), ground-channels-2b PR 4a, STRM-072, STRM-076

#### Scenario: Windows, caps and the diagnostic
- **WHEN** windows of a face are read against the whole face, the caches are given a cap of one window, of less than one and of room for both, six threads ask for one window, the sources are searched for the diagnostic decode, and the pit and erosion depths are read at cells of tiles that hold pits, across a tile border and where no record exists
- **THEN** each window equals the same cells of the whole face and a window outside the face is refused, the one-window cap decodes three times for the reads 0, 1, 0 and holds one window, the smaller cap keeps the entry decoded, the larger decodes twice, the six threads decode once, the trait gives the reader's numbers, only the definition names the diagnostic, the pit depth is the stored cell, the erosion depth is what the carve removed not counting a pit, neither steps at a border, and a tile without a record reads none
