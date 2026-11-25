"""Lead #184p addition (crusader's renderer note: a cut narrower than about 2 mesh samples beads from about 6 km out): the rim-to-rim top width of the curve cuts.
At every 4th curve point inside the box, the final surface and S0 are read bilinear along the curve's normal at -24..24 L cells (step 0.25). The cut is the run of offsets
around the curve (from the lowest point within 1 cell) where the final surface lies more than 0.5 m under S0; its top width is that run's extent (rim to rim), its depth
the largest S0 - final in it. Points with no cut (depth under 0.5 m) are left out of the width table and counted. Reported: top width p10 / p50 / p90 (m and L cells);
the share of curve length with a top width under 3 L cells, overall and at cuts deeper than 20 m. Usage: cut_width.py <chain dir> <trunk-curves.bin>"""
import sys, struct, numpy as np
from scipy import ndimage
D, CB = sys.argv[1], sys.argv[2]
n, m, cell = 8160, 1600, 101.802
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n)
fin = np.fromfile(f'{D}/final.f32', '<f4').reshape(n, n)
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; P, T = [], []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    if npt < 5:
        continue
    t = np.gradient(p, axis=0); t /= np.maximum(np.hypot(t[:, 0], t[:, 1]), 1e-12)[:, None]
    P.append(p[2:-2:4]); T.append(t[2:-2:4])
P = np.concatenate(P); T = np.concatenate(T)
ok = (P[:, 0] >= m + 30) & (P[:, 0] < n - m - 30) & (P[:, 1] >= m + 30) & (P[:, 1] < n - m - 30)
P, T = P[ok], T[ok]
N = np.column_stack([-T[:, 1], T[:, 0]]); offs = np.arange(-24, 24.001, 0.25); c0 = len(offs) // 2
X = P[:, 0][:, None] + offs * N[:, 0][:, None]; Y = P[:, 1][:, None] + offs * N[:, 1][:, None]
zf = ndimage.map_coordinates(fin, [Y.ravel(), X.ravel()], order=1).reshape(X.shape).astype(np.float64)
z0 = ndimage.map_coordinates(s0, [Y.ravel(), X.ravel()], order=1).reshape(X.shape).astype(np.float64)
land = z0[:, c0] > 0
cut = (z0 - zf) > 0.5
near = np.abs(offs) <= 1.0
W = np.full(len(P), np.nan); Dp = np.zeros(len(P))
for i in np.nonzero(land)[0]:
    j = np.argmin(np.where(near, zf[i], np.inf))
    if not cut[i, j]:
        continue
    lo = j
    while lo > 0 and cut[i, lo - 1]:
        lo -= 1
    hi = j
    while hi < len(offs) - 1 and cut[i, hi + 1]:
        hi += 1
    W[i] = (offs[hi] - offs[lo] + 0.25)
    Dp[i] = (z0[i, lo:hi + 1] - zf[i, lo:hi + 1]).max()
has = np.isfinite(W)
w = W[has]; dpt = Dp[has]
q = lambda a, p: np.percentile(a, p)
print(f'{D.rstrip("/").split("/")[-1]}: {land.sum()} land profiles; with a cut (> 0.5 m under S0 at the floor) {has.sum()} ({100 * has.sum() / land.sum():.1f} %)')
print(f'  top width rim to rim: p10 {q(w, 10):.2f} / p50 {q(w, 50):.2f} / p90 {q(w, 90):.2f} L cells ({q(w, 10) * cell:.0f} / {q(w, 50) * cell:.0f} / {q(w, 90) * cell:.0f} m); '
      f'a run reaching 24 cells: {100 * np.mean(w >= 47.9):.1f} %')
print(f'  top width under 3 L cells: {100 * np.mean(w < 3):.1f} % of the cut curve length')
for t in (10, 20, 50):
    s = dpt > t
    if s.any():
        print(f'  cuts deeper than {t:2d} m ({100 * s.mean():.1f} % of the cut length): top width under 3 cells {100 * np.mean(w[s] < 3):.1f} %, p50 width {q(w[s], 50):.2f} cells')
