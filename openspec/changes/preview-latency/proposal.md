# preview-latency (briefing item 16, step 2; TEST-099)

## What
- `crates/generators/examples/preview_latency.rs`: builds the resident base of a preview session at the H0 grid and times the level-1 preview of a height stamp, of a rainfall region and of both, with the phases, the process peak and the resident bytes.
- `cargo xtask perf preview`: the scene under the readiness gate (ADR 0020), `--smoke` on a small grid (never a measurement).
- TEST-099 gets its measurements and the shape of its report.

## Not in this change
- Optimising the preview. The number first; the phase split says what to optimise.
- The overlay (VIEW-067) and the editor path.

## Load
Builds and runs only after sonnet-bulldozer says the explorer's runs have ended; design and spec text may be written before. A perf run needs the architect's authorisation of a window (CLAUDE.md); `--smoke` does not.
