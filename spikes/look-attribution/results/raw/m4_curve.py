"""Lead #184: M4 along the trunk curves. M4 (m4_along.py) takes the channel's direction from the H1 D8 steps; a curve leaves those lines, so here the direction is the
curve's own tangent. Cells: the L land cells inside the box within 4 L cells of a curve point, each with the tangent of its nearest point. k(p) = z(p + H t) - 2 z(p) +
z(p - H t), bilinear, H = 2 L cells (the registered stencil); reported: the RMS and the p90 of |k|. Usage: m4_curve.py <chain dir> <curves.bin> <stage>..."""
import sys, struct, numpy as np
from scipy import ndimage, spatial
D, cb = sys.argv[1], sys.argv[2]
n, m, R, H = 8160, 1600, 4.0, 2.0
b = open(cb, 'rb').read()
assert b[:4] == b'TRC1'
k = 8; cnt = struct.unpack_from('<I', b, 4)[0]
P, T = [], []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    if npt < 3:
        continue
    t = np.gradient(p, axis=0); t /= np.maximum(np.hypot(t[:, 0], t[:, 1]), 1e-12)[:, None]
    P.append(p); T.append(t)
P = np.concatenate(P); T = np.concatenate(T)
inb = (P[:, 0] >= m) & (P[:, 0] < n - m) & (P[:, 1] >= m) & (P[:, 1] < n - m)
P, T = P[inb], T[inb]
tree = spatial.cKDTree(P)
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n)
lo, hi = np.floor(P.min(0) - R).astype(int), np.ceil(P.max(0) + R).astype(int)
yy, xx = np.mgrid[max(lo[1], m):min(hi[1], n - m), max(lo[0], m):min(hi[0], n - m)]
q = np.column_stack([xx.ravel(), yy.ravel()]).astype(np.float64)
d, i = tree.query(q, distance_upper_bound=R)
ok = np.isfinite(d)
q, i = q[ok], i[ok]
land = s0[q[:, 1].astype(int), q[:, 0].astype(int)] > 0
q, i = q[land], i[land]
tx, ty = T[i, 0], T[i, 1]
print(f'{len(P)} curve points in the box; {len(q)} L land cells within {R} cells of a curve')
for st in sys.argv[3:]:
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n)
    f = lambda sx, sy: ndimage.map_coordinates(z, [q[:, 1] + sy, q[:, 0] + sx], order=1)
    kk = f(H * tx, H * ty) - 2 * z[q[:, 1].astype(int), q[:, 0].astype(int)].astype(np.float64) + f(-H * tx, -H * ty)
    print(f'{st:7s} M4 along the curves, stencil {H:.0f} L cells: RMS {np.sqrt(np.mean(kk * kk)):.3f} m, p90 |k| {np.percentile(np.abs(kk), 90):.3f} m')
