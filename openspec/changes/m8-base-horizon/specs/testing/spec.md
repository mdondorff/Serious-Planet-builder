## ADDED Requirements

### Requirement: TEST-065 The gate judges horizon bakes at every site and the re-bake after its edit
The gate SHALL fail a lit run with the horizon jobs on when no horizon map was baked, and when no map the edit touched (its edit word changed) landed within 10 s of its scripted edit (the shadow of a plateau on its neighbour, M8 criterion 6). On the software adapter both are reported and not judged. The edit-latency scene SHALL print a table headed `verdict (10 s, horizon maps)` with one row per wanted map the edit touched, the milliseconds from the edit to the map's landing and ok or over; a map is on screen with its landing plus the frames of its upload, the window putting one landed map on the GPU per frame (`HorizonRig::decide`), so within one frame of landing when it is the only one pending. The window gate SHALL log the time and frame at which the first re-baked map is drawn after its edit.

Verify: A, C
Status: active
Source: opus-planner M8-F step 2, TEST-061

#### Scenario: A slow re-bake
- **WHEN** the first map landing after the edit comes 11 s after it, or none comes
- **THEN** the verdict fails naming the time or that none landed
