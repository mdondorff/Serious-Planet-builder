"""Lead #184m hypotheses (a) and (b), per chain.
(a) KH's side-slope floor: in the curve zone KH's target is bed + max(|grad S0|, 0.05) x reach. Reported: the share of land cells within 3 L cells of a curve raster cell
    where 0.05 wins (|grad S0| < 0.05, central differences).
(b) The bed against S0: along each trunk curve (points every 0.5 cell), the bed is taken as z_s of the H1 cell under the point (zs.f32, 1020 x 1020; KH interpolates z_s
    along the reach's H1 cells by length share, so this is the same field read nearest). S0's own along-reach minimum is the running minimum of S0 (bilinear) from the
    curve's upstream end. Reported: (S0 running minimum - bed) over the points in the box, p50 / p90 / p99, and the share over 10 / 20 m; and one reach's profile.
Usage: bed_vs_s0.py <chain dir> <trunk-curves.bin> [reach]"""
import sys, struct, numpy as np
from scipy import ndimage
D, CB = sys.argv[1], sys.argv[2]; R1 = int(sys.argv[3]) if len(sys.argv) > 3 else None
n, m, cell = 8160, 1600, 101.802
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n).astype(np.float64)
zs = np.fromfile(f'{D}/zs.f32', '<f4').reshape(1020, 1020).astype(np.float64)
gy, gx = np.gradient(s0, cell); slope = np.hypot(gx, gy); del gx, gy
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]
rast = np.zeros((n, n), bool); gaps = []; one = None
for _ in range(cnt):
    r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4
    cc = np.frombuffer(b, np.dtype([('c', '<u4'), ('o', 'u1', 2)]), nr, k)['c'].astype(np.int64); k += 6 * nr
    rast[cc // n, cc % n] = True
    if npt < 3:
        continue
    zs0 = ndimage.map_coordinates(s0, [p[:, 1], p[:, 0]], order=1)
    run = np.minimum.accumulate(zs0)
    hx = np.clip((p[:, 0] // 8).astype(int), 0, 1019); hy = np.clip((p[:, 1] // 8).astype(int), 0, 1019)
    bed = zs[hy, hx]
    inb = (p[:, 0] >= m) & (p[:, 0] < n - m) & (p[:, 1] >= m) & (p[:, 1] < n - m) & (zs0 > 0)
    gaps.append((run - bed)[inb])
    if R1 is not None and r == R1:
        one = (p, zs0, run, bed)
g = np.concatenate(gaps)
near = (ndimage.distance_transform_edt(~rast) <= 3) & (s0 > 0)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True
near &= box
name = D.rstrip('/').split('/')[-1]
print(f'{name}: (a) curve zone (land within 3 cells of a curve, in the box) {near.sum()} cells; 0.05 wins (|grad S0| < 0.05) on {100 * np.mean(slope[near] < 0.05):.1f} %; '
      f'|grad S0| p50 {np.median(slope[near]):.3f}')
print(f'  (b) S0 running minimum - bed along the curves ({len(g)} points): p50 {np.percentile(g, 50):.1f}, p90 {np.percentile(g, 90):.1f}, p99 {np.percentile(g, 99):.1f} m; '
      f'over 10 m {100 * np.mean(g > 10):.1f} %, over 20 m {100 * np.mean(g > 20):.1f} %; under 0 (bed above S0 minimum) {100 * np.mean(g < 0):.1f} %')
if one is not None:
    p, z, run, bed = one
    i = np.linspace(0, len(p) - 1, 12).astype(int)
    print(f'  reach {R1}: {len(p)} points, from {p[0].round(0)} to {p[-1].round(0)}; at 12 stations: S0 / S0 running min / bed (m)')
    print('   ' + '; '.join(f'{z[j]:.0f}/{run[j]:.0f}/{bed[j]:.0f}' for j in i))
    print(f'  reach {R1}: S0 running min - bed p50 {np.median(run - bed):.1f}, max {np.max(run - bed):.1f} m')
