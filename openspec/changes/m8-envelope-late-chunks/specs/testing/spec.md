## ADDED Requirements

### Requirement: TEST-059 The steady search flies one place per search and the log tells why a chunk was late
Each search of the envelope scene's steady curve SHALL fly all its trials over one start place, `--places K` searches per altitude (default 2) at places of their own, the log giving each and the lowest speed being the figure. For each flight the log SHALL give the chunks that landed more than a second after they were first wanted (the three worst and their count) with when each was first asked to prefetch, first seen with a job running and landed, so that a late chunk is told apart as asked late or solved slowly.

Verify: A
Status: active
Source: opus-planner (2026-10-27, window 12: chunks of 9 to 10 s at two start places), TEST-057, STRM-042

#### Scenario: A late chunk
- **WHEN** a chunk is wanted at 1 s, asked at 0.5 s, seen running at 1.5 s and lands at 9 s while another lands 0.4 s after it was wanted
- **THEN** only the first is late, with a wait of 8 s and a solve of 7.5 s, and the log carries its key and times
