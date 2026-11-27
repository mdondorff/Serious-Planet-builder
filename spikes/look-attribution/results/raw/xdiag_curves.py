"""Lead #184f check 1, on R1h's own geometry: in-zone Xdiag (m1_zone's zone, m1_facet's definition) of S0 with a facet-free carve along step 1's trunk curves,
against the same carve along the curves' 4-connected raster (a lattice-locked staircase floor). If the smooth carve reads near the staircase, Xdiag measures the
curves' lattice-aligned direction, not facets.
Carve (KH's curve form, simplified): bed = S0 bilinear at the nearest curve point (smooth) or S0 at the nearest raster cell (stair4); target = bed + max(|grad S0|,
0.05) x distance; z = S0 + g x min(0, target - S0), g = 1 - smoothstep((target - bed - 8) / 4) (KH's height gate); cells within 4 L cells of a curve.
With a third argument (a step-1 chain's kh.f32) the bed is that KH surface at the nearest raster cell (to the curve point for smooth), its own Xdiag reported too.
Usage: xdiag_curves.py <chain dir with s0.f32, h1-rivers.u8, chain.txt> <trunk-curves.bin> [kh.f32]"""
import os, sys, struct, numpy as np
from scipy import ndimage, spatial
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from m1_facet import excess  # noqa: E402
D, CB = sys.argv[1], sys.argv[2]
KH = sys.argv[3] if len(sys.argv) > 3 else None
n, m, nh, cell, R = 8160, 1600, 1020, 101.802, 4.0
for line in open(f'{D}/chain.txt'):
    if line.startswith('window '):
        x0, y0 = map(int, line.split()[1:3])
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n).astype(np.float64)
riv = np.fromfile(f'{D}/h1-rivers.u8', 'u1').reshape(nh, nh) > 0
zone = np.repeat(np.repeat(ndimage.binary_dilation(riv, structure=np.ones((3, 3), bool)), 8, 0), 8, 1)
box = np.zeros((n, n), bool); box[m:n - m, m:n - m] = True
land = box & (s0 > 0)

b = open(CB, 'rb').read(); assert b[:4] == b'TRC1'
k = 8; cnt = struct.unpack_from('<I', b, 4)[0]
P, Rc = [], []
for _ in range(cnt):
    _r, npt = struct.unpack_from('<II', b, k); k += 8
    p = np.frombuffer(b, '<f4', 2 * npt, k).reshape(npt, 2).astype(np.float64); k += 8 * npt
    nr = struct.unpack_from('<I', b, k)[0]; k += 4
    for j in range(nr):
        Rc.append(struct.unpack_from('<I', b, k + 6 * j)[0])
    k += 6 * nr
    if npt >= 2:
        seg = np.diff(p, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); s = np.ceil(L / 0.1).astype(int)
        for a, d, q in zip(p[:-1], seg, s):
            P.append(a + d * (np.arange(q)[:, None] / q))
        P.append(p[-1:])
P = np.concatenate(P); Rc = np.unique(np.array(Rc, np.int64))
rxy = np.column_stack([Rc % n, Rc // n]).astype(np.float64)
rast = np.zeros((n, n), bool); rast[Rc // n, Rc % n] = True
near = (ndimage.distance_transform_edt(~rast) <= R + 1.0) & land
ys, xs = np.nonzero(near)
q = np.column_stack([xs, ys]).astype(np.float64)
gy, gx = np.gradient(s0, cell)
slope = np.maximum(np.hypot(gx, gy)[ys, xs], 0.05)
print(f'{len(P)} curve points (0.1-cell spacing), {len(Rc)} raster cells, {len(q)} cells near a curve')


def carve(d, bed):
    target = bed + slope * d * cell
    s = np.clip((target - bed - 8.0) / 4.0, 0.0, 1.0)
    g = 1.0 - s * s * (3.0 - 2.0 * s)
    z = s0.copy()
    v = s0[ys, xs]
    ok = d <= R
    z[ys[ok], xs[ok]] = (v + g * np.minimum(0.0, target - v))[ok]
    return z


d_s, i_s = spatial.cKDTree(P).query(q, distance_upper_bound=R + 1.0)
d_s = np.where(np.isfinite(d_s), d_s, 1e9); i_s = np.minimum(i_s, len(P) - 1)
bed_s = ndimage.map_coordinates(s0, [P[i_s, 1], P[i_s, 0]], order=1)
d_r, i_r = spatial.cKDTree(rxy).query(q, distance_upper_bound=R + 1.0)
d_r = np.where(np.isfinite(d_r), d_r, 1e9); i_r = np.minimum(i_r, len(rxy) - 1)
bed_r = s0[Rc[i_r] // n, Rc[i_r] % n]
kh = None
if KH:
    kh = np.fromfile(KH, '<f4').reshape(n, n).astype(np.float64)
    _, j = spatial.cKDTree(rxy).query(P[i_s])
    bed_s = kh[Rc[j] // n, Rc[j] % n]
    bed_r = kh[Rc[i_r] // n, Rc[i_r] % n]
sub = (slice(m - 2, n - m + 2), slice(m - 2, n - m + 2))
zm = (land & zone)[sub]
runs = [('S0', s0)] + ([('KH (dump)', kh)] if kh is not None else []) + [('smooth carve', carve(d_s, bed_s)), ('stair4 carve', carve(d_r, bed_r)),
        ('smooth flat', carve(np.maximum(d_s - 0.71, 0), bed_s)), ('stair4 flat', carve(np.maximum(d_r - 0.71, 0), bed_r))]  # KH's 0.71-cell floor
for name, z in runs:
    o = excess(z[sub], zm, x0 + m - 2, y0 + m - 2)
    print(f'{name:13s} in the zone: Xdiag {o["Xdiag"]:+.3f}  X8 {o["X8"]:+.3f}  X6 {o["X6"]:+.3f}  (kink {o["energy"]:.2f} m2)')
