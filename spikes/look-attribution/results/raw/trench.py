"""Lead #184h item 1: the trench along the trunk curves, per stage. At every 4th curve point inside the box (land), the surface is read bilinear along the curve's
normal at offsets -8..8 L cells (step 0.25). Per profile: the slot depth (the mean of the profile at +-2 cells minus its minimum within +-1 cell), its width at half
that depth (cells, interpolated; only where the depth is over 1 m), the valley depth (the mean at +-8 cells minus the minimum), and the notch: h = ((z(1) + z(-1)) / 2 - z(0)) - ((z(2) + z(-2)) / 2 - (z(1) + z(-1)) / 2),
the centre's depth below the V the +-1 and +-2 cells continue (0 on a V, the slot's depth on a one-cell slot, negative on a flat floor). Reported p50 / p90 per stage, in m
and L cells (101.802 m). Usage: trench.py <chain dir> <trunk-curves.bin> [stage ...]"""
import sys, struct, numpy as np
from scipy import ndimage
D, CB = sys.argv[1], sys.argv[2]
stages = sys.argv[3:] or ['s0', 'kh', 'ks0', 'ks1c', 'band6c', 'kv', 'final']
n, m, cell = 8160, 1600, 101.802
b = open(CB, 'rb').read(); assert b[:4] == b'TRC1'
k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; P, T = [], []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    if npt < 5:
        continue
    t = np.gradient(p, axis=0); t /= np.maximum(np.hypot(t[:, 0], t[:, 1]), 1e-12)[:, None]
    P.append(p[2:-2:4]); T.append(t[2:-2:4])
P = np.concatenate(P); T = np.concatenate(T)
inb = (P[:, 0] >= m + 10) & (P[:, 0] < n - m - 10) & (P[:, 1] >= m + 10) & (P[:, 1] < n - m - 10)
P, T = P[inb], T[inb]
N = np.column_stack([-T[:, 1], T[:, 0]])
offs = np.arange(-8, 8.001, 0.25)
X = P[:, 0][:, None] + offs[None, :] * N[:, 0][:, None]
Y = P[:, 1][:, None] + offs[None, :] * N[:, 1][:, None]
i0 = np.argmin(np.abs(offs)); near = np.abs(offs) <= 1.0; at2 = np.isin(offs, [-2, 2]); at8 = np.isin(offs, [-8, 8])
print(f'{len(P)} profiles (every 4th curve point in the box); p50 / p90')
s0 = None
for st in stages:
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n)
    pr = ndimage.map_coordinates(z, [Y.ravel(), X.ravel()], order=1).reshape(X.shape).astype(np.float64)
    if st == 's0':
        land = pr[:, i0] > 0
    del z
    pr = pr[land]
    zmin = pr[:, near].min(1)
    slot = pr[:, at2].mean(1) - zmin
    valley = pr[:, at8].mean(1) - zmin
    half = zmin + slot / 2
    wid = np.full(len(pr), np.nan)
    sel = slot > 1.0
    for j in np.nonzero(sel)[0]:
        q = pr[j]; c = np.argmin(np.where(near, q, np.inf))
        lo = c
        while lo > 0 and q[lo - 1] < half[j]:
            lo -= 1
        hi = c
        while hi < len(q) - 1 and q[hi + 1] < half[j]:
            hi += 1
        # interpolate the crossings
        l = offs[lo] - 0.25 * (half[j] - q[lo]) / max(q[lo - 1] - q[lo], 1e-9) if lo > 0 else offs[lo]
        r = offs[hi] + 0.25 * (half[j] - q[hi]) / max(q[hi + 1] - q[hi], 1e-9) if hi < len(q) - 1 else offs[hi]
        wid[j] = r - l
    w = wid[sel]
    v = lambda o: pr[:, np.argmin(np.abs(offs - o))]
    notch = ((v(1) + v(-1)) / 2 - v(0)) - ((v(2) + v(-2)) / 2 - (v(1) + v(-1)) / 2)
    q = lambda a, p: np.percentile(a, p)
    print(f'{st:7s} slot depth {q(slot, 50):6.2f} / {q(slot, 90):6.2f} m; slots over 1 m {100 * sel.mean():5.1f} %, their half-depth width {q(w, 50):4.2f} / {q(w, 90):4.2f} cells '
          f'({q(w, 50) * cell:4.0f} / {q(w, 90) * cell:4.0f} m); valley depth at +-8 cells {q(valley, 50):6.1f} / {q(valley, 90):6.1f} m; notch {q(notch, 50):5.2f} / {q(notch, 90):5.2f} m, over 2 m {100 * np.mean(notch > 2):4.1f} %, over 5 m {100 * np.mean(notch > 5):4.1f} %')
