## ADDED Requirements

### Requirement: WDEF-009 128-bit item ids and content hashes
The id of an authored item (`ItemId`) and its content hash (`ItemHash`) SHALL be 128 bits (`u128`; ADR 0066 point 6, ADR 0023 point 2), and every encoding of an id SHALL be 16 bytes big-endian, so the byte order is the numeric order (the application order of equal priorities, the order of `item_ids`, the order of a footprint result). A hash that was defined over 64-bit ids and content hashes SHALL be unchanged for values below 2^64: the definition hash, the cell hash and the footprint hash (64-bit words) write the low words where the 64-bit value stood and the high words of the rows that have any, behind a marker and a count, after the rows (the header fixes the rows' number and width, so the tail cannot be taken for rows; no tail when every value is below 2^64); the cache's query-result hash (bytes) has no header, so it writes 40-byte rows of the low 8 bytes and, when a row is wide, a tail read from the end (the wide rows' index and high words, the count of wide rows, the count of rows and a marker byte: `20 n + 9` bytes, never a multiple of 40); the item hash of the footprint writes the content hash as 8 bytes when it is below 2^64 and as 16 bytes otherwise. A value above 2^64 SHALL reach every hash it reaches below it, and two different sets of items SHALL NOT hash alike because of the tail. The ULID text of an id and the item file come with E4.

Verify: A
Status: active
Source: ADR 0066 point 6, ADR 0023 point 2, ADR 0018

#### Scenario: Wide values everywhere
- **WHEN** items with the largest id and content hash are put into a definition, found in a cell and in a footprint, and removed
- **THEN** they work as any item, the ids order numerically, and the definition hash is a function of the set of items whatever the insertion order

#### Scenario: Narrow values keep their hashes
- **WHEN** the pinned known answers of the footprint and summary hashes and a query result of ids and hashes below 2^64 (the old encoding written out in the test) are computed
- **THEN** they equal what a 64-bit build gave

#### Scenario: Wide never aliases narrow
- **WHEN** an id or a content hash differs from another only in its high 64 bits, in any row of a result
- **THEN** the definition, cell, footprint and query-result hashes all differ, and which row is wide matters
