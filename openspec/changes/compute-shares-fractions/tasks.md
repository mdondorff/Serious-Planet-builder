## PR split (one OpenSpec change, four PRs; IDs from the planner)
1. **Specs (docs).**
   - BUDG-003: the capacity, the formula, the f/min/max table, the fit rule, and the worked examples labelled as examples (status planned until PR 3).
   - The new detection requirement, planned.
   - spec-lint and spec-guardian.
2. **Topology (code).**
   - `planet_streaming::topology`: Windows `GetLogicalProcessorInformationEx` (feature `Win32_System_SystemInformation` on the existing `windows` dependency), Linux sysfs (`cpu_core`/`cpu_atom`, `cpu_capacity`), `available_parallelism` fallback, and `PLANET_CPU_TOPOLOGY=P,E`.
   - Tier A on recorded API bytes, sysfs strings and overrides.
   - The detection requirement turns active. Done in PR 2: Rasierklinge detects 12 P and 8 E threads (Windows core records), on 20 usable threads.
3. **Shares in the code.**
   - `Shares::for_topology`; `GenWorkers::machine()` from the terrain share; the rig's ceiling from it, replacing `TERRAIN_CORES`.
   - Horizon bakes `terrain / 2`, replacing `HORIZON_WORKERS_MAX`; the coupled reduction from `B`; the BUDG-011 loads from their shares; the BUDG-012 scenario.
   - Tier A over synthetic topologies (Rasierklinge 6P+8E, an 8-core desktop, a 4-core laptop, below minimum).
   - BUDG-003 active; deltas of BUDG-011, BUDG-012, STRM-020, STRM-064, and TEST-039/040 (the calibration on a fixed `min(18, logical threads)`, accepted).
   - numerics-reviewer is not needed (no core, generator or shader code); verifier, spec-guardian, prepush.
4. **Cross-references (docs).**
   - TEST-039 and TEST-040 named as reference-machine calibrations (no verdict elsewhere).
   - `docs/domain.md` term check; the README and status pointers; ADR 0083's status line (amended by 0092, ADR 0078).
- After all four: archive with a log fragment. The flights at the share run in the planned quiet slot (step 4, already 6 workers).
