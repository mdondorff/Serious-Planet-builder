# Planet builder: agent operating manual

## What this is
A planet-scale generator, editor and real-time renderer: Rust + wgpu, one binary with three run modes (editor/viewer, `generate` headless CLI, `test-render` headless harness). Developed spec-first with OpenSpec. Primary machine: "Rasierklinge" (Windows 11, RTX 3080 Ti Laptop GPU, 16 GB).

## Sources of truth (highest first)
1. `docs/constitution.md`: non-negotiable rules. Read it at the start of every session. Never edit it; propose amendments as ADRs.
2. `openspec/specs/`: current subsystem specs, the single home of current rules (every normative number, budget, limit and format; ADR 0055). Read only the 2–3 specs your task touches.
3. `docs/adr/`: accepted decisions and why they were made. Never edit an accepted ADR except its status line.
4. `docs/background/`: frozen research and setup docs. **Read only when an ADR, spec or task points at a specific section** (e.g. "background report §5"). Never treat them as requirements, never update them.

Working files: `docs/status.md` (forward state only: waiting for the owner, the current queue, the current stop report, tracked items), `docs/log.md` and `docs/log.d/` (the current milestone's log: history plus one fragment per new entry; read it with `cargo xtask log`), `docs/spikes.md` (spike register), `docs/learnings.md` (findings, each with a scope). Finished milestones are archived in `docs/archive/<M>/` by the `housekeeping` skill.

## Commands (always via xtask)
- `cargo xtask test fast`: tier A for changed crates (<= 20 s; owner, 2026-10-16)
- `cargo xtask test`: tiers A + B on the software adapter
- `cargo xtask test gpu`: tier C goldens and debug views
- `cargo xtask test gpu --real`: the same on the discrete GPU. Run it yourself whenever real-GPU evidence is useful; it needs no owner confirmation (owner, 2026-10-12), but never during an architect's perf window. Golden candidates it produces go through `golden-review` (ADR 0017) like any other.
- `cargo xtask spec-lint`: trace IDs, tiers, requirement coverage
- `cargo xtask accept <milestone>`: milestone acceptance
- `cargo xtask repro <bundle>`: replay a repro bundle
- `cargo xtask golden-review [--proposal <file>] [--extra <dir>]... [--apply]`: classify golden candidates under ADR 0017 (the `golden-review` skill)
- `cargo xtask perf`: Rasierklinge only. The harness's readiness gate (ADR 0020) measures the machine and refuses when it is not ready; perf runs need the architect's authorization: the architect coordinates the window, and a coder session started in the native Claude UI runs it on the architect's go (owner, 2026-10-16; the architect's VS Code-terminal session measures about 1.4x slower per thread). Coders never start one on their own. `--machine-ready` is the owner's override of a failed gate

Until an xtask command exists, create it in `xtask/` rather than writing a script. The shell is Git Bash on Windows; keep commands portable and never add `.sh` or `.ps1` files to the repo.

## How to work
1. Pick up work as an OpenSpec change (`openspec/changes/<id>/`): proposal, design, tasks, spec deltas. Keep changes small (one subsystem, one milestone step).
2. For each task: write the failing test at the cheapest tier that can falsify it, then implement. Every test cites its requirement ID (`// spec: COORD-003`).
3. Run the `numerics-reviewer` agent for changes in `core`, `generators` or shaders, and the `spec-guardian` agent before archiving a change.
4. Run the `verifier` agent before declaring any task done. Fix what it finds; don't argue with it.
5. Archive the change in OpenSpec when all its requirements pass; in the same PR delete the archived change's `design.md` and `tasks.md` (keep `proposal.md` and the spec deltas; ADR 0055). Then add a log fragment `docs/log.d/<YYYY-MM-DD>-<id>.md` holding one line (never edit `docs/log.md`; `cargo xtask log` prints both).
6. A new decision → `/adr`. A time-boxed experiment → `/spike`. A surprise or a corrected assumption → `/learning`. How a decision was built goes in the change's design notes, not in a new ADR; write an ADR only for a new decision or a departure from an accepted one (ADR 0055).

## Writing (ADR 0055)
Specs, ADRs and working files are read by agents and the owner. Write definitions, not narration:
- one statement per line or bullet; numbers in tables;
- no conversational asides, no "we" or "I"; who decided goes on the status line, not in the prose;
- an ADR is at most about one page (Context up to 5 bullets, Decision as numbered one- or two-line points, Consequences); the current value of a number lives in the spec, not the ADR.

## Rules that are easy to break
- The renderer executes a `FramePlan`; decisions live in GPU-free crates (CON-03).
- f64 on the CPU, tile-local or camera-relative f32 on the GPU; no planet-radius values in shaders (CON-11).
- `libm` for transcendentals in generator code; no thread-count-dependent results (CON-14).
- Failure output must be actionable: numeric deltas, tile IDs, diff images in `target/review/`.
- Bless goldens only through the `golden-review` skill's policy (ADR 0017): it auto-blesses rounding noise and declared changes, each adapter against its own golden (ADR 0061), and escalates everything else to the owner's `/bless`. Never copy a candidate into `tests/goldens/` by hand.
- Never weaken, skip or delete a test to make a build green. Quarantine only with a tracking entry in `docs/status.md`.

## Git
- One branch per OpenSpec change (`change/<id>`). Small commits with clear messages.
- Push the branch and open a PR with `gh pr create`. Merge it yourself (squash) only when CI is green on Windows and Linux and the verifier passed. A docs-only PR (only `docs/`, `openspec/`, `.claude/`, root `*.md`, `spikes/**/results/**`) runs the light `docs` check (spec-lint) instead and needs only that to be green (owner, 2026-10-16).
- After every push to a PR, start `gh pr checks <n> --watch` as a background command and act on its result (fix a failure, or verify and merge). Never end a turn with CI pending and no such watch running (owner, 2026-10-18).
- Never force-push, never rewrite `main`'s history, never commit secrets or large binaries (goldens are small PNGs; anything over 1 MB needs an ADR).

## When to stop and report
Write a stop report in `docs/status.md` (what was done, what is blocked, what you need) and stop when:
- a test can't be made deterministic;
- a decision is needed that no ADR or spec covers **and** it is hard to reverse (draft the ADR as "proposed" first). Easy-to-reverse decisions: draft a "proposed" ADR and keep going on it. Proposed ADRs from the background report's "decide now" list may be built on before the owner accepts them;
- golden images are waiting for blessing and further work depends on them;
- a performance measurement is needed on Rasierklinge: ask the architect session for a perf window (it authorizes the window and names who runs it; the harness's gate decides); do not run `cargo xtask perf` without that authorization (correctness runs with `test gpu --real` do not count as measurements);
- a milestone go/no-go gate is reached (after M2 and after M5);
- the constitution would have to change.
Otherwise, keep going.
