## ADDED Requirements

### Requirement: TEST-056 The edit-latency scene runs with the chunk stage
The `edit-latency` scene (TEST-019) SHALL, given a store and unless `--no-chunks` is given, run its world with the A-net chunk stage on the M8 default and drive the chunk frames in its loop. After the budget and stages tables it SHALL print a table of the chunk side of every case: the longest chunk frame after the edit, the chunks that were solved again, the instants of the chunk under the edit centre and of the last chunk, and the instant the tiles that read them were drawn again, with a verdict against 10 s.

Verify: A
Status: active
Source: m8-edit-latency-chunks, M8 criterion 6, TEST-019, TEST-041

#### Scenario: The chunk table
- **WHEN** the table is made from cases whose runs carry chunk readings
- **THEN** it gives the medians per case, `ok` where the eroded result is drawn within 10 s and `over` where it is not, and `-` for a case without a chunk stage
