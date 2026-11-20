# Pipeline Cockpit: Global Authoring Companion & Step Inspector
Status: design note, non-normative (ag2-antigravity, 2026-11-20); companion to BUILD-063, TEST-142, and planned BUILD-064.

## 1. Intent & Why Two PRs
Before an in-app Global Authoring UI lands inside the runtime viewer (A3 section 11 / `driver-regions.md`), authoring parameters and pipeline steps are inspected across scattered headless commands (`preview world`, `generate --*-preview`, `stages`, `transects`). A durable local HTML cockpit gives an immediate offline companion for:
- Visualizing the real active terrain & hydrology pipeline across flags and definitions (`--erosion`, `--channels`, `world.height_field`, `turned_noise`, ADR 0007 `stamp` layers),
- Running and side-by-side inspecting headless Global Authoring driver previews (`shelf`, `lithology`, `uplift`, `orography`, `arid`, `ice`, `world`) and stage sheets without needing a GPU or rewriting prose docs.

Scraping `docs/design/terrain-target.md` drifts from live code as flags and `*_VERSION` pins evolve. Instead, the cockpit reads a canonical interface descriptor generated directly from Rust source constants and runtime chain assembly, split into two PRs so the contract and JSON interface can be reviewed before the HTML UI builds on it:

| PR | Branch / IDs | Scope | Gate |
|---|---|---|---|
| **PR 1** | `change/pipeline-descriptor` (`BUILD-063`, `TEST-142`) | Code-generated `pipeline_catalog`, committed `docs/stages/pipeline.json`, `cargo xtask stages [--update]` sync, `planet generate --pipeline-json OUT`, runtime `HeightField::upstream()` contract test | Tier-2 Draft PR (`opus-architect` + owner review before merge) |
| **PR 2** | `change/xtask-pipeline-cockpit` (`BUILD-064`, after PR 1 merge) | `std`-only `cargo xtask cockpit` HTML assembler (`<1 s` offline report) and `--run <world\|drivers\|stages\|all>` batch orchestrator over existing preview CLIs | Tier-1 (`xtask` companion tooling) |

**Scope trim:** No repo-admin/meta context drawer belongs in `cargo xtask cockpit`. It focuses solely on the pipeline graph, active chains, version/key pins, and step outputs.

## 2. Architecture of PR 1 (`BUILD-063`, `TEST-142`)
- **`planet_generators::pipeline_catalog`**: Pure, zero-IO/zero-GPU introspection module referencing the live `*_VERSION` constants in `planet-world-def` and `planet-generators`, enumerating curated pipeline stages with their `PreviewSpec` commands and exact emitted filenames, and mapping `PipelineSwitches` (`HeightFieldKind`, `erosion_layer`, `relief_lattice`, `channels`, ADR 0007 stamp stack, water cull level) to predicted `ChainLink` stacks.
- **Drift prevention (`TEST-142` & `cargo xtask stages`)**:
  - `the_committed_pipeline_json_matches_the_code` fails whenever any pinned version constant, known `HeightFieldKind`, switch variant, or declared preview output changes without refreshing `docs/stages/pipeline.json` via `cargo xtask stages --update`.
  - `the_declared_runtime_chain_matches_the_walked_height_field_upstream_chain` instantiates `field_chain_erosion` across all valid switch/stamp/water configurations and asserts exact equality between `resolve_runtime_chain` and walking `HeightField::upstream()`.
- **CLI resolution (`planet generate --pipeline-json OUT`)**: Builds the real `HeightField` chain for `--seed N` or `--world F` (and optional `--erosion on --channels on`) over zero-IO metadata stubs and emits the resolved JSON chain + active stage IDs.

## 3. Planned Consumer in PR 2 (`BUILD-064`, Non-Normative Preview)
After PR 1 review and merge, `cargo xtask cockpit` will assemble a self-contained single-file HTML report under `target/review/cockpit/` purely with `std` Rust (no `serde` added to `xtask`), offering three focused tabs:
1. **Pipeline DAG & Switch Matrix**: Renders `docs/stages/pipeline.json`, highlights active paths per `HeightFieldKind` and `--erosion`/`--channels`/stamp switches, and optionally runs `planet generate --pipeline-json` on a supplied `--world`.
2. **Global Authoring & Step Inspector**: Indexes existing `target/preview/` and `target/review/` outputs (and triggers `preview world`, `--{shelf,lithology,uplift,climate,arid,ice}-preview`, `stages`, `transects` under `--run`), pairing generated PNGs with their `.txt` metrics and seeded side-by-side comparison.
3. **Version & Cache-Key Pin Ledger**: Displays all 42 live `*_VERSION` constants from `pipeline.json` alongside `docs/stages/manifest.txt` hashes.

## 4. Constitutional & Layering Guarantees
- **Non-authoritative (`CON-06`, `CON-07`)**: `pipeline_catalog` never feeds into generator math, sample bits, or cache keys; `docs/stages/pipeline.json` is strictly a derived inspection artifact.
- **Crate isolation (`CON-02`)**: `xtask` stays `std`-only; all generator knowledge remains inside `planet-generators` (`BUILD-063`) and `planet` CLI (`--pipeline-json`).

## 5. Review Questions for `opus-architect`
1. Does the 14-stage catalog granularity in `docs/stages/pipeline.json` (`global-input`, `global-bake`, `region-bake`, `tile-runtime`) complement P3's evolving `rg_chain` / `FieldGraph` structure cleanly?
2. Should `pipeline.json` or `planet generate --pipeline-json` expose any extra fields (such as explicit `FieldGraph` / river-graph stage placeholders or look/world-hash links) before PR 2 builds the `xtask cockpit` UI on top of this schema?
