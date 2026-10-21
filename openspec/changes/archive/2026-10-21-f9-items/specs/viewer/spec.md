## MODIFIED Requirements
### Requirement: VIEW-024 The window keeps the camera's last minute and F9 writes it
The window SHALL keep a rolling log of the camera path (`planet_frame::pathlog::PathLog`): a pose recorded at most every 100 ms of the window's clock (the first always), each with the time, the position, the view direction and the up vector in f64, the altitude above the terrain and the frame time, and nothing older than 60 s before the newest; in the viewer and in the editor mode alike. F9 (`Command::SaveRepro`, which changes nothing in the viewer and is never blocked by the editor mode) SHALL write `frame-<n>-path.txt` (the log) and `frame-<n>.bundle` (a terrain repro bundle, TEST-008, of that frame's request over the world as the window holds it, edits included) with `<n>` the frame count, to `target/review` or to the directory `PLANET_REPRO_DIR` names, and print their names. The path file SHALL be written first, so a bundle that fails never loses it. With authored items the bundle's world section SHALL carry the definition text without items and each item as its item-file TOML (`world_item` lines), so that the replay builds a definition equal to the window's, items included; a record that does not rebuild to the definition the frame was planned from SHALL be refused with both hashes. A path file is `planet-path 1`, the window and period, the sample count and one line per sample, with floats in the shortest round-trip form so that reading restores them bit for bit; an unknown header, a wrong count, a malformed line or a time that goes back SHALL be refused with the line. The `--smoke` script SHALL press F9 once where `PLANET_REPRO_DIR` is set and the summary SHALL report the writes.

Verify: A, C
Status: active
Source: opus-planner (2026-10-19), TEST-008, VIEW-022; opus-planner (2026-10-21, H0: F9 with authored items), AUTH-001

#### Scenario: Ten poses a second, one minute
- **WHEN** frames come every 7 ms for 3 minutes, and again 50 ms, 99 ms and 100 ms after a recorded pose
- **THEN** the log holds about 10 poses a second over the last 60 s and no more than 601, and the 50 ms and 99 ms poses are not recorded while the 100 ms one is

#### Scenario: A path file round trip
- **WHEN** a log is written to text and read back, and damaged texts are read
- **THEN** every float restores bit for bit, and each damaged text is refused naming the problem (header, count, line, time going back)

#### Scenario: F9 in the real window
- **WHEN** the editor smoke runs with `PLANET_REPRO_DIR` set
- **THEN** one F9 write is reported, the directory holds `frame-<n>-path.txt` with at least two poses and `frame-<n>.bundle` that starts `planet-terrain-repro` and records the world

#### Scenario: A world with a stamp, a line and a brush stroke
- **WHEN** the definition of an editor holding a stamp, a line and a brush stroke is recorded and rebuilt from the recorded texts
- **THEN** the rebuilt definition equals the editor's (and hashes the same), and an item list that is not the definition's is refused naming both hashes
