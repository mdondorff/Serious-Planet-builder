## ADDED Requirements

### Requirement: STRM-030 The roots reach the GPU before frame 0
The frame loop SHALL offer a pre-frame step that writes the roots to the tile sink (`ManagedLoop::upload_roots`): every resident tile at or above the streamer's render floor, in one batch, outside the per-frame cap (STRM-001, ADR 0064), recorded as resident exactly as the first frame records them. A harness that has loaded the roots SHALL call it before its first measured frame, so that frame 0 writes no root and obeys the cap of STRM-006 and the limits of CON-21; the perf rig (`Rig::new`) SHALL call it at the end of its start-up. A caller that does not call it SHALL see the old behaviour: the first frame writes the roots. The step SHALL change no other tile, no count and no key, and the first plan draws the roots as it did.

Verify: A, C
Status: active
Source: STRM-001, ADR 0064, window 13 (2026-10-19)

#### Scenario: Before frame 0
- **WHEN** the roots are loaded and `upload_roots` is called, then the first frame is begun
- **THEN** the sink holds every root in its slot, the first frame uploads none of them (uploaded 0, upload bytes 0) and writes nothing more, and the loop knows every root as resident

#### Scenario: Without the call
- **WHEN** the first frame is begun without `upload_roots`
- **THEN** it writes the roots, beyond the cap, as before (ADR 0064)

#### Scenario: A measured frame
- **WHEN** the streaming scenes run in smoke mode
- **THEN** frame 0 of every scene's frame log writes at most the per-frame cap
