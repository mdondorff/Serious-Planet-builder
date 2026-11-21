## Why
- Deliver the companion Global Authoring & pipeline debugger UI (`BUILD-064`) on top of the code-generated `pipeline_catalog` and `--pipeline-json` contract merged in PR #1615 (`BUILD-063`, `TEST-142`).
- Let developers and reviewers inspect the real active `HeightField` stack across switches (`world.height_field`, `--erosion`, `--channels`, `--commit`, ADR 0007 stamps, `WaterBounded`), browse or batch-run CPU Global Authoring previews side-by-side, and search all 42 pinned constants and `docs/stages/manifest.txt` probe metrics offline without GPU initialization.

## What changes
- `BUILD-064`: Add `xtask/src/cockpit/mod.rs` and `xtask/src/cockpit/html.rs` (pure `std`, zero new crate dependencies, zero external network assets), wired as `cargo xtask cockpit` in `xtask/src/main.rs` with self-tested `-h`/`--help` living CLI documentation (embedded in `index.html`) plus 1-line discovery hints in `README.md` and `docs/README.md`.
- Add unit tests in `xtask/src/cockpit/mod.rs` covering strict CLI parsing, `FLAG_SPECS` / `parse_args` / example sync, `--run` command planning, pure-`std` JSON parsing and schema-v2 validation, `</script>` safety escaping, PNG `IHDR` header + `.txt` + `manifest.txt` indexing into `index.html`, and `opus-architect`'s follow-up source check confirming `bake::probe::Stage::ALL_KINDS` covers every enum variant of `Stage`.
- Update `docs/design/pipeline-cockpit.md` and add `docs/log.d/2026-11-21-xtask-pipeline-cockpit.md`.

## Impact
`xtask` (`main.rs`, new `cockpit/{mod.rs,html.rs}`), `openspec/specs/build-tooling/spec.md`, `README.md`, `docs/README.md`, `docs/design/pipeline-cockpit.md`, `docs/log.d/2026-11-21-xtask-pipeline-cockpit.md`. Tier-1 cold-zone change (`std`-only `xtask`); zero impact on runtime crates, generator bits, cache keys, or shaders.
