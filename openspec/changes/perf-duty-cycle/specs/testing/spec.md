## RENAMED Requirements
- FROM: `### Requirement: TEST-037 Streaming scenes start cool and take no GPU rows`
- TO: `### Requirement: TEST-037 Streaming scenes take no cool start and no GPU rows`

## MODIFIED Requirements
### Requirement: TEST-037 Streaming scenes take no cool start and no GPU rows
Before the measured span of a streaming scene the harness SHALL take no cool start and no warm-up (ADR 0094): the scenes' verdicts are CPU wall time, the GPU temperature at the start stays recorded in the log's clock sample, and a runner of several runs keeps its duty idle between them (BUILD-033; `perf stream all` between its scenes). A run whose first clock sample reads above 70 C SHALL print `hot start: <path> began with the GPU at <t> C (above 70 C; throttle <mask>)`, so a device loss or throttling after a hot start is seen against it (ADR 0094 point 4). There is no warm-up: the loop is paced to 60 Hz and CPU-bound, so no amount of rendering it brings the GPU to the loaded clock (look-2km at 58 C stayed at P3, 1,087 MHz, after 30 s unpaced), the held views and flights are already unpaced back-to-back waited frames, and the streaming scenes take no GPU time for the clock to matter to. A streaming scene (`stream/<scene>`, `stream-lit/<scene>`) SHALL NOT make a session invalid for GPU verdicts by the clock it started at (it has none): the clock stays recorded in the log, the session line reads `no GPU rows (the pipelined scenes take no GPU time)`, and the acceptance facts of a scene say the same; logs written before this rule, which carry the idle-start warning, are still read as valid for frame verdicts. A view that takes GPU rows keeps ADR 0034's idle start rule. The camera scripts, the budgets and the streaming code are unchanged, and the frame verdicts too.

Verify: A
Status: active
Source: perf windows 7 and 8 (every streaming scene started at P5 or P8; the lit dive's two device losses came after long runs on a hot GPU, most likely on a locked desktop: `docs/status.md`), the look-2km diagnostic run of 2026-10-18, ADR 0057, ADR 0094 (SPIKE-95: SPIKE-90 window 2 lost 26 min to capped cool-downs of streaming scenes), TEST-033

#### Scenario: Which steps a scene takes
- **WHEN** a smoke run and a measured streaming scene are started
- **THEN** neither cools down and no scene warms up

#### Scenario: A hot start is named
- **WHEN** a streaming run's first clock sample reads 70 C, and another's 74 C with throttle 0x4
- **THEN** the first prints nothing, the second the hot-start line with 74 C and 0x4

#### Scenario: An idle start of a streaming scene
- **WHEN** a streaming scene began at P8, 210 MHz
- **THEN** the session is valid for frame verdicts and for GPU rows (none exist), its line says `no GPU rows`, and the warnings are empty; the same start of a lit view still voids the session's GPU verdicts

### Requirement: TEST-033 Cool start of a view
Before the warm-up of every view (the terrain scene's and the lit scenes', not in a smoke run) the harness SHALL wait, idle, until the GPU temperature reads 60 C or less, polling every 5 s for at most 30 s, and SHALL print one line per view: the temperatures at the start and the end and the seconds, or `NOT cooled` with the cap. An unreadable sensor ends the wait at once. The wait adds nothing to a measured frame. A view that starts hot is judged by the clock floor's one-sided rule (TEST-034).

Verify: A
Status: active
Source: perf window 7 (the clock follows the GPU temperature under the software power cap), ADR 0057 point 1, ADR 0094 (the cap 120 s -> 30 s; SPIKE-95), TEST-012, TEST-029, TEST-034

#### Scenario: Cooling
- **WHEN** the temperature reads 74, 69, 64 and 59 C on four polls
- **THEN** the wait ends after three pauses (15 s) as cooled, and a GPU that stays at 72 C ends at the 30 s cap as `NOT cooled`

#### Scenario: Cool already, or no sensor
- **WHEN** the first reading is 55 C, or the sensor gives nothing
- **THEN** the wait ends at once, cooled in the first case
