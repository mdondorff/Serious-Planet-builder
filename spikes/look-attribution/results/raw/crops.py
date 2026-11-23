"""SPIKE-143: the render crops of a window, chosen before any stage render is drawn (pre-registered): 120 x 120 L cells (12.2 km), inside the box.
- river: the crop holding the most high saw-tooth vertices (s above the final surface's p90; m2_m3.py's m2-vertices.npz);
- hill: the crop with the largest lattice excess X8 of the final surface (m1_facet's definition per crop) among crops with over 150 m of relief;
- rim: the crop centred on the spill cell of the deepest kept basin (graph.txt, represented=protected_surface) inside the box.
Usage: crops.py <chain dir> <x0> <y0> <m2-vertices.npz> <out.json>"""
import sys, json, struct, numpy as np
D, x0, y0, vnpz, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], sys.argv[5]
n, m, C = 8160, 1600, 120
z = np.fromfile(f'{D}/final.f32', '<f4').reshape(n, n).astype(np.float64)
v = np.load(vnpz)
hi = v['s_final'] > np.percentile(v['s_final'], 90)
bx, by = (v['vx'][hi] - m) // C, (v['vy'][hi] - m) // C
nb = (n - 2 * m) // C
h = np.zeros((nb, nb)); np.add.at(h, (np.clip(by, 0, nb - 1), np.clip(bx, 0, nb - 1)), 1)
j, i = np.unravel_index(np.argmax(h), h.shape)
river = (int(m + i * C), int(m + j * C))
# hill: X8 per crop of the final surface (d_xx against x mod 8, d_yy against y mod 8)
best, hill = -1, None
for j in range(nb):
    for i in range(nb):
        a = z[m + j * C:m + (j + 1) * C, m + i * C:m + (i + 1) * C]
        if a.max() - a.min() < 150:
            continue
        dxx = (a[:, 2:] - 2 * a[:, 1:-1] + a[:, :-2]) ** 2
        dyy = (a[2:, :] - 2 * a[1:-1, :] + a[:-2, :]) ** 2
        gx = (np.arange(1, C - 1) + m + i * C + x0) % 8
        gy = (np.arange(1, C - 1) + m + j * C + y0) % 8
        px = np.bincount(gx, weights=dxx.sum(0), minlength=8); py = np.bincount(gy, weights=dyy.sum(1), minlength=8)
        ls = lambda p: max(p[k] + p[(k + 1) % 8] for k in range(8)) / max(p.sum(), 1e-30)
        x8 = (ls(px) + ls(py)) / 2 / 0.25 - 1
        if x8 > best:
            best, hill = x8, (int(m + i * C), int(m + j * C))
# rim: the deepest kept basin's spill
deep, rim = -1.0, None
f64 = lambda hx: struct.unpack('>d', bytes.fromhex(hx))[0]
with open(f'{D}/graph.txt', encoding='utf-8') as f:
    for line in f:
        if not line.startswith('basin ') or 'represented=protected_surface' not in line:
            continue
        p = dict(t.split('=', 1) for t in line.split()[2:] if '=' in t)
        fc, fz = p['floor'].split(':'); sc, sz = p['spill'].split(':')
        sc = int(sc); gx, gy = (sc & 0xFFFFFFFF) - x0, (sc >> 32) - y0
        if not (m + C // 2 <= gx < n - m - C // 2 and m + C // 2 <= gy < n - m - C // 2):
            continue
        d = f64(sz) - f64(fz)
        if d > deep:
            deep, rim = d, (int(gx - C // 2), int(gy - C // 2))
crops = {'size': C, 'river': river, 'hill': hill, 'hill_x8': best, 'rim': rim, 'rim_depth_m': deep}
json.dump(crops, open(out, 'w'), indent=1)
print(crops)
