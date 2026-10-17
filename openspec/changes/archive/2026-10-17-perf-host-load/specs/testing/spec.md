## ADDED Requirements

### Requirement: TEST-023 Host load during a perf case
Every perf case (a view of the terrain scene, a streaming scene) SHALL record the machine's CPU load while it runs: a sampler on its own thread reads the cumulative CPU time of every process (the performance counters' raw class, which is the same in every locale) when the case starts, every 5 seconds and when it ends, and turns each pair of readings into the machine's busy share (percent of all logical processors, `Idle` excluded), the harness's share (its own process and its helpers `powershell`, `pwsh`, `nvidia-smi`, `wmiprvse`, `conhost`) and the competing share (the rest), with the five largest competing processes by name (instances of one name added). The log SHALL carry, in the run's `load` object, the number of samples, the mean and maximum of the machine's and the competing share, the harness's mean, the top competitors over the case (mean share over all samples, largest share, samples seen), the first and last samples (start and end) and the series; the console SHALL print one `host load` line per case and one `LOAD NOTE` line per note. A note SHALL be written when the competing share averages more than 15 % (ADR 0020's limit on the machine's load, applied to what is not the harness) and for each process of ADR 0020's busy list (`cargo`, `rustc`, `cargo-nextest`, `nextest`, `planet`, `clippy-driver`) that used 1 % or more of the machine in a sample (judged before the top five are cut); the notes are in the run's `load.notes` and in the log's top-level `load_notes`. The load is recorded only: no note, no share and no missing reading changes a validity answer, a verdict, a comparison or the acceptance of a log, and a log without `load` reads as before. A case whose load cannot be read (a smoke run, a host without the counters; the terrain views and the streaming scenes both print it) says `host load: not measured` and carries `"load":{"measured":false}` or no `load`.

Verify: A, C
Status: active
Source: M5 item 8 (status.md; tier C here is the unmeasured path of the smoke runs, the measuring sampler is read in the architect window and by an ignored live test), ADR 0020, ADR 0034, ADR 0036, M4 perf windows 1 to 5, TEST-012, TEST-019

#### Scenario: Harness, helpers and competitors
- **WHEN** two readings 10 s apart show the harness process at 25 % of an 8-processor machine, its `powershell` at 1 %, two `chrome` processes at 20 % and 10 % and `msedge` at 5 %
- **THEN** the machine's share is 61 %, the harness's 26 %, the competing 35 %, and the top list reads `chrome` 30 % then `msedge` 5 % without the harness

#### Scenario: Notes at ADR 0020's numbers
- **WHEN** a case's competing share averages 20 %, or exactly 15 %, or `rustc` used 2.5 % in one sample
- **THEN** the first and the third write a note naming the figures and ADR 0020, the second writes none, and in no case does `valid_for_budgets`, `frame_valid` or `gpu_session_valid` differ from the same log without `load`

#### Scenario: Not measured
- **WHEN** the host cannot be read, or the run is a smoke run
- **THEN** the console says `host load: not measured`, the log carries no numbers and no notes, and nothing is judged

### Requirement: TEST-024 Store check in the smoke run
A smoke run over a stored bake (`test-render --perf-smoke --scene S --store DIR`, `cargo xtask perf stream S --smoke --store DIR`) SHALL first check the store read-only and print the result before any scene: the store, `GLOBAL_VERSION` and `MESH_VERSION`, the ocean channel read at a fixed grid of 288 directions (12 latitudes by 24 longitudes) with the counts of sea and land, and the number of tiles of levels 0 to L (`--prefill-levels L`, default 4) the tile pack holds in the compact form and in the vertex form, each present tile read and decoded through the pack's checks. The run SHALL fail before any scene, naming the problem and the command that fixes it, when the store has no ocean channel, when the channel reads all land or all sea on the grid, or when the compact form lacks tiles of levels 0 to L (the vertex form is shown for information: the production run draws the compact form). The check SHALL write nothing to the store, and the smoke run is labelled as no measurement.

Verify: A, C
Status: active
Source: opus-planner request of 2026-10-17 (M5 item 8, before window 6), TEST-019, STRM-021, GEN-022, ADR 0045 point 3

#### Scenario: A complete store
- **WHEN** the smoke run reads a stored bake pre-filled to level 1 with `--prefill-levels 1`
- **THEN** it prints `store check (smoke, read-only, NOT a measurement)`, `ocean channel: present, 288 directions read, N sea and M land`, `compact form 30 of 30 present` and `store check ok`, runs its scenes, and the listing of the store directory (names, sizes, modification times) is unchanged afterwards

#### Scenario: An incomplete pack
- **WHEN** the same store is smoked with `--prefill-levels 2`
- **THEN** the run fails with `store check FAILED`, the count `of 126 compact tiles of levels 0-2`, `MESH_VERSION` and the `generate --prefill-dir` command, before any scene runs, and the store is unchanged

#### Scenario: A store without a usable ocean
- **WHEN** the channel is missing, or reads all land or all sea
- **THEN** the problems name `no ocean channel` and `generate --global-dir`, or `reads land on all 288` or `reads sea on all 288`
