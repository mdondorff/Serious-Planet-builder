## ADDED Requirements
### Requirement: STRM-075 The channel reader owns its handles, reads the area through windows and caches under byte caps
The channel reader (`LayerChannels`, STRM-072) SHALL own its store and world handles and implement the generators' `ChannelSource` (the area as log2 of cells, the deposited depth in metres). It SHALL read the drainage area through windows of the face (256 cells and the one past) streamed from the record, never holding a face whole, with the same codes as the same cells of the whole face (it checks the rows it reads, the key and the header). It SHALL hold windows and depth tiles under byte caps that keep the entry just decoded, and workers asking for one window together SHALL decode it once. The decode that skips the key check SHALL be a diagnostic that no viewer or generation code calls.

Verify: A
Status: active
Source: planner inbox #65 (2026-11-05), ground-channels-2b, STRM-072

#### Scenario: Windows, caps and the diagnostic
- **WHEN** windows of a face are read against the whole face, the caches are given a cap of one window, of less than one and of room for both, six threads ask for one window, and the sources are searched for the diagnostic decode
- **THEN** each window equals the same cells of the whole face and a window outside the face is refused, the one-window cap decodes three times for the reads 0, 1, 0 and holds one window, the smaller cap keeps the entry decoded, the larger decodes twice, the six threads decode once, the trait gives the reader's numbers, and only the definition names the diagnostic
