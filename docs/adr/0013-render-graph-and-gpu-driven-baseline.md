# 0013 Render graph and GPU-driven baseline
Status: proposed
Date: 2026-10-10
Source: background report §8, §12 (ADR candidates), §17 item 12; SPIKE-01; constitution CON-03

## Context
Passes will multiply (depth, terrain, atmosphere, shadows, post). A declared graph gives ordering checks, culling of unused passes and transient memory aliasing. Instances (vegetation, buildings) need GPU-driven submission; terrain node selection must stay on the CPU (CON-03).

## Options (with trade-offs)
1. **Small custom render graph** (pass and resource declarations, compile-time checks, lifetimes) plus indirect draws for instances.
2. No graph: a hand-ordered list of passes. Simple now, hard to check and alias later.
3. An external graph library. Extra dependency, API churn.

## Decision
Option 1. `render::graph` declares passes with reads, writes and side effects; `compile` rejects reads without an earlier producer, drops passes whose outputs nobody uses, and computes lifetimes and alias groups per memory class. The terrain executor stays one monolithic pass until a second pass exists. GPU-driven submission is for **instances** only, behind a CPU reference (`frame::instances`: culling and indirect arguments) and a differential test (REND-008). Terrain nodes are never culled on the GPU.

## Consequences
- SPIKE-01 found the needed wgpu features on WARP and the RTX 3080 Ti; they must be requested at device creation.
- Easier: adding passes with checked dependencies. Harder: keeping declarations and executors in step (the tests compile a representative frame; no product code declares a graph yet).
- Tests: graph compile tests (A); GPU culling differential (B) when the pass exists.

## Licences of adopted code or techniques
None.
