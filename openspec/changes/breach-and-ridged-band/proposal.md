## Why
SPIKE-101 (2026-11-04): high mountains reach 0.21 of GLO-30's 3 km relief as built. The hill band's amplitude is the whole base-side cause, and pass 1's 50 m breach lets the carve fill the basins the band leaves. Owner D7 (2026-11-04) put SPIKE-102's M9 items before the planet bake; SPIKE-102 run (a) on 2/12/10 (seed 14) agrees with SPIKE-101 (high mountains 0.62 = 0.62, mountains 0.86 against 0.91).

## What Changes
- **GEN-098:** on the mountain and high-mountain classes the band is x4.5 as high, a ridged multifractal (16 to 1 km), centred on its mean 0.45; land keeps at least 1 m above the sea. `CLASS_SOURCE_VERSION` 6.
- **GEN-100:** pass 1's `breach_max_m` default 50 m becomes 500 m; the option `breach_height_scale_m` (off) scales the limit with the receiver's height. `BAKE_KERNEL_VERSION` 4. Pass 2 keeps breach 0.
- Tests: the mountain band's range, mean and amplitude; the default breach and the option; the class source fingerprint and the class field pin re-recorded.
- Not in this change: keep-pits (a stored channel, owner D1), re-running the preview layer.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-098, GEN-100 (planned, in `m9-bake`)

Expected golden changes: none (the class source is not on a render path before the bake).

Supersedes the numbers of ADR 0088 point 4 / ADR 0099's context that were measured with a 50 m breach (relief kept 73 % and 94 %); the spec is the home of the values (ADR 0055), so no new ADR. Flagged to the planner before merge.
