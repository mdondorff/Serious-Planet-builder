# 0008 App structure
Status: proposed
Date: 2026-10-10
Source: background report §10; constitution CON-01, CON-02, CON-04

## Context
One product, three ways to run it (interactive, headless generation, headless test rendering). Splitting into several apps adds IPC and versioning with no benefit before a cloud tile server exists.

## Options (with trade-offs)
1. **One binary, three run modes, layered library crates.**
2. Several applications. More deployment surface; shared code still needs libraries.

## Decision
Option 1. Binary `planet` (package `planet`, crate directory `crates/app`) with modes `editor`, `generate`, `test-render`. Library crates, downward dependencies only: `core → world-def → generators → cache → streaming → frame → render → editor → app`; the first six compile without any graphics crate. Directory names are the short names above; package names are `planet-<name>`. Support crates outside the chain: `planet-testkit` (golden compare, image metrics; dev-dependency only) and `planet-perf` (performance-harness protocol; depends on nothing), plus `xtask`.
- Time, I/O and task spawning are injected into streaming and scheduling code (`Clock`, `Io`, `Spawner`; CON-04).
- A GPU-free `planet` build (`--no-default-features`) is possible once feature flags exist; the layering check already proves the tree.
- ECS is for placed objects and simulation only; terrain tiles are quadtree + cache.

## Consequences
- `cargo xtask layering` enforces the chain and the GPU-free property (CON-02) on every run of `check`.
- A cloud tile server later is another thin binary over `generators + cache`.

## Licences of adopted code or techniques
None.

Addendum (proposed, M0 review): `planet-testkit` may appear only under `[dev-dependencies]` (enforced by `xtask layering`); dev-dependencies of chain crates on `testkit` are the only allowed upward edge. `planet-perf` has no dependencies. Types of the graphics API stay inside `render`: layers above see `render`'s own types (for example `AdapterKind`).
