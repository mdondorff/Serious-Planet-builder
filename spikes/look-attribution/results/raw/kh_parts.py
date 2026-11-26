"""Lead #184o item 1: S_KH - S0 on a step-1 chain by KH component. In curve mode (rg_chain, `--kh-zone curve`, dams off) KH's value is C1 (the Catmull-Rom read of z_s)
everywhere, lowered toward the curve's target (bed + max(|grad S0|, 0.05) x reach, gated 8-12 m above the bed) within 8 L cells of a curve; S_KH = KH + r with S0 = C1 + r,
so outside the curve zones S_KH = S0 (but for the sea + 0.5 m floor), and the h5/h4c zone read and the dams do not enter. Masks by the distance d to the curve's
segments (0.1-cell samples) less 0.71 cells (KH's own distance): (a) the line, d = 0 (the flat floor); (a') the zone off the line, 0 < d <= 8; (c) outside every curve
zone, d > 8. Per mask (land, in the box): cell share, the share KH moves by more than 0.5 m, rms and max |S_KH - S0|; X8 and Xdiag (m1_facet) of S_KH and of S0 on the
mask, split into the H1 channel zone (m1_zone's) and outside it. Usage: kh_parts.py <chain dir> <trunk-curves.bin>"""
import os, sys, struct, numpy as np
from scipy import ndimage, spatial
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from m1_facet import excess  # noqa: E402
D, CB = sys.argv[1], sys.argv[2]
n, m, nh = 8160, 1600, 1020
for line in open(f'{D}/chain.txt'):
    if line.startswith('window '):
        x0, y0 = map(int, line.split()[1:3])
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n).astype(np.float64)
kh = np.fromfile(f'{D}/kh.f32', '<f4').reshape(n, n).astype(np.float64)
riv = np.fromfile(f'{D}/h1-rivers.u8', 'u1').reshape(nh, nh) > 0
hz = np.repeat(np.repeat(ndimage.binary_dilation(riv, structure=np.ones((3, 3), bool)), 8, 0), 8, 1)
b = open(CB, 'rb').read(); k = 8; cnt = struct.unpack_from('<I', b, 4)[0]; P = []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4 + 6 * nr
    if npt >= 2:
        seg = np.diff(p, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); s = np.maximum(np.ceil(L / 0.1).astype(int), 1)
        for a, d_, q in zip(p[:-1], seg, s):
            P.append(a + d_ * (np.arange(q)[:, None] / q))
        P.append(p[-1:])
P = np.concatenate(P)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True
land = box & (s0 > 0)
ys, xs = np.nonzero(land)
d = np.full((n, n), np.inf)
dd, _ = spatial.cKDTree(P).query(np.column_stack([xs, ys]).astype(np.float64), distance_upper_bound=9.0)
d[ys, xs] = np.maximum(dd - 0.71, 0.0)
np.save(os.path.join(os.path.dirname(os.path.abspath(CB)), os.path.basename(D.rstrip('/')) + '-curve-dist.npy'), d.astype(np.float32))
diff = kh - s0
masks = [('(a) the line, d = 0', land & (d == 0)), ("(a') the zone off the line, 0 < d <= 8", land & (d > 0) & (d <= 8)), ('(c) outside every curve zone', land & ~(d <= 8))]
sub = (slice(m - 2, n - m + 2), slice(m - 2, n - m + 2))
print(f'{D.rstrip("/").split("/")[-1]}: land in the box {land.sum()} cells')
for lab, mk in masks:
    v = diff[mk]
    line = f'  {lab:40s} {100 * mk.sum() / land.sum():5.1f} % of land; moved > 0.5 m {100 * np.mean(np.abs(v) > 0.5):5.1f} %; rms {np.sqrt(np.mean(v * v)):6.2f} m, max |.| {np.abs(v).max():6.1f} m'
    print(line)
    for zl, zm in (('in the H1 zone', hz), ('outside it', ~hz)):
        mm = (mk & zm)[sub]
        if mm.sum() < 1000:
            print(f'      {zl}: {mm.sum()} cells, too few'); continue
        a = excess(kh[sub], mm, x0 + m - 2, y0 + m - 2); c = excess(s0[sub], mm, x0 + m - 2, y0 + m - 2)
        print(f'      {zl:15s} ({mm.sum():8d} cells): S_KH X8 {a["X8"]:+.3f} Xdiag {a["Xdiag"]:+.3f} | S0 X8 {c["X8"]:+.3f} Xdiag {c["Xdiag"]:+.3f}')
