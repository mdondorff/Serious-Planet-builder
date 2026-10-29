## ADDED Requirements

### Requirement: TEST-068 A still camera draws the same pixels frame after frame once streaming has settled
With the camera still and the streaming settled (the edit-latency scene's settle), 6 consecutive frames of the same request on the software adapter SHALL be pixel-identical (`Tolerance::Exact`) in the level, relief and normals debug views. A change SHALL fail the test naming the changed pixels, their box and the drawn tiles that differ between the two frames, and SHALL write both frames and the diff image to `target/review/still-flicker/`. Each run SHALL prove it can falsify: more than one colour in the picture and more than 8 drawn tiles.

Verify: C
Status: active
Source: owner live test 2026-10-22 (orbit flicker and dark-pixel flicker, not reproduced), opus-planner (2026-10-29), STRM-016

Detail: the lit path (atmosphere, water glint, exposure, the hover overlay) is not drawn by these views; a still-frame check of the lit path is a follow-up. The scene is the first view of the edit-latency scene at 200 m over a small memory bake; the software adapter is deterministic, so no tolerance is allowed.

#### Scenario: A still camera
- **WHEN** the scene is settled and 6 frames of the same request are drawn and read back in each of the three views
- **THEN** every pair of consecutive frames has zero differing pixels

#### Scenario: A changing frame
- **WHEN** two consecutive frames differ
- **THEN** the failure names the differing pixels, their box and the differing tiles, and the two frames and the diff image are written under `target/review/still-flicker/`
