"""Lead #184o item 2 and #184p item 2: the cut's steepness on a chain's final surface, along the curves of a curves file.
- Wall slope across the curve: at every 4th curve point in the box, the steepest 0.5-cell step of the final surface within +-3 L cells across the curve (m per m).
- Cells within 3 L cells of a curve (by the curve-distance file kh_parts.py writes): the steepest slope to any of the 8 neighbours (what a mesh edge sees); the count over
  45 and 30 degrees, split into "at a col over 10 m" (within 1 cell of a cell the KH guard of r1h-mg held: a cut deeper than max(the defaults' lowering, 10 m)) and
  "elsewhere".
- The 155 m col (reach 5519's curve point nearest window L (3152, 3744)): the final profile across, every L cell.
Usage: cut_slopes.py <chain dir> <trunk-curves.bin> <curve-dist.npy> <guard list>"""
import sys, struct, numpy as np
from scipy import ndimage
D, CB, DIST, G = sys.argv[1:5]
n, m, cell = 8160, 1600, 101.802
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n)
z = np.fromfile(f'{D}/final.f32', '<f4').reshape(n, n).astype(np.float64)
d = np.load(DIST)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True
near = box & (s0 > 0) & (d <= 3)
xy = np.loadtxt(G, comments='#', skiprows=1, dtype=int)
col = np.zeros((n, n), bool); col[xy[:, 1], xy[:, 0]] = True
col = ndimage.binary_dilation(col, structure=np.ones((3, 3), bool))
best = np.zeros_like(z)
for dy in (-1, 0, 1):
    for dx in (-1, 0, 1):
        if dx or dy:
            best = np.maximum(best, np.abs(z - np.roll(np.roll(z, dy, 0), dx, 1)) / (cell * np.hypot(dx, dy)))
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; P, T = [], []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    if npt >= 5:
        t = np.gradient(p, axis=0); t /= np.maximum(np.hypot(t[:, 0], t[:, 1]), 1e-12)[:, None]; P.append(p[2:-2:4]); T.append(t[2:-2:4])
P = np.concatenate(P); T = np.concatenate(T)
ok = (P[:, 0] >= m + 10) & (P[:, 0] < n - m - 10) & (P[:, 1] >= m + 10) & (P[:, 1] < n - m - 10); P, T = P[ok], T[ok]
N = np.column_stack([-T[:, 1], T[:, 0]]); offs = np.arange(-3, 3.001, 0.5)
X = P[:, 0][:, None] + offs * N[:, 0][:, None]; Y = P[:, 1][:, None] + offs * N[:, 1][:, None]
pr = ndimage.map_coordinates(z, [Y.ravel(), X.ravel()], order=1).reshape(X.shape)
wall = np.max(np.abs(np.diff(pr, axis=1)), axis=1) / (0.5 * cell)
s = best[near]
q = lambda a, p: np.percentile(a, p)
print(f'{D.rstrip("/").split("/")[-1]}: wall slope across the curve ({len(P)} profiles) p50 {q(wall, 50):.3f}, p90 {q(wall, 90):.3f}, p99 {q(wall, 99):.3f}, max {wall.max():.2f} m/m')
print(f'  cells within 3 cells of a curve ({near.sum()}): steepest neighbour slope p50 {q(s, 50):.3f}, p90 {q(s, 90):.3f}, p99 {q(s, 99):.3f}, max {s.max():.2f} m/m '
      f'({np.degrees(np.arctan(s.max())):.0f} deg)')
for lab, th in (('45 deg', 1.0), ('30 deg', np.tan(np.radians(30)))):
    over = near & (best > th)
    print(f'  over {lab}: {over.sum()} cells; at a col over 10 m {(over & col).sum()}, elsewhere {(over & ~col).sum()}')
i = np.argmin(np.hypot(P[:, 0] - 3152, P[:, 1] - 3744)); p0, nv = P[i], N[i]
o = np.arange(-12, 12.01, 1.0)
qz = ndimage.map_coordinates(z, [p0[1] + o * nv[1], p0[0] + o * nv[0]], order=1)
print(f'  the 155 m col, curve point {p0.round(1)}: final profile across every cell: ' + ' '.join(f'{v:.0f}' for v in qz))
print(f'    its steepest one-cell step {np.abs(np.diff(qz)).max():.0f} m ({np.abs(np.diff(qz)).max() / cell:.2f} m/m)')
