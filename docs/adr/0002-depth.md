# 0002 Depth buffer
Status: proposed
Date: 2026-10-10
Source: background report §3 (Depth), Key Finding 3; constitution CON-12

## Context
A planet view spans 0.05 m to 10,000 km in one frame. The depth strategy touches every pipeline, shadow pass and post effect, so it is expensive to change later.

## Options (with trade-offs)
1. **Reversed-Z, D32F, infinite far plane.** Relative error about 2^-23 of distance (about 1 m at 10,000 km, where only limb and atmosphere remain). Keeps early-Z and hierarchical-Z optimisations.
2. Logarithmic depth written in the fragment shader. Outerra pioneered it and later recommended reversed float depth; it disables depth-buffer optimisations.
3. Multi-frustum splitting (KSP style). Extra passes and seams.

## Decision
Option 1: reversed-Z (near = 1, far = 0), `Depth32Float`, depth compare `Greater`, infinite far plane, near plane about 0.05 to 0.1 m. A log-depth shader path stays documented as a fallback and is not built. Multi-frustum is not used unless profiling shows z-fighting.

## Consequences
- Projection matrices, clear values (0.0) and compare functions are set in one place in `render` and exercised by the depth debug view (tier C).
- Shadow and atmosphere passes must be written for reversed-Z from the start.
- A wgpu feature check (D32F depth attachments on every target adapter) is part of the M2 go/no-go review.

## Licences of adopted code or techniques
None.
