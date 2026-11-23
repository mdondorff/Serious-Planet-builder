"""SPIKE-143, post-hoc (after the renders; not pre-registered): M1's excesses restricted to KH's channel zone (the H1 channel cells of h1-rivers.u8 and their
8 neighbours, read at L) and to the cells outside it, because the triangles the renders show sit along the channels while M1 averages over the whole window.
Usage: m1_zone.py <chain dir> <x0> <y0> <stage>..."""
import sys, numpy as np
from scipy import ndimage
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from m1_facet import excess  # noqa: E402  (the same definition)
D, x0, y0 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
n, m, nh = 8160, 1600, 1020
riv = np.fromfile(f'{D}/h1-rivers.u8', 'u1').reshape(nh, nh) > 0
zone_h = ndimage.binary_dilation(riv, structure=np.ones((3, 3), bool))
zone = np.repeat(np.repeat(zone_h, 8, 0), 8, 1)
s0 = np.fromfile(f'{D}/s0.f32', '<f4').reshape(n, n)
box = np.zeros((n, n), bool); box[m:n-m, m:n-m] = True
land = box & (s0 > 0)
sub = (slice(m - 2, n - m + 2), slice(m - 2, n - m + 2))
print(f'zone cells inside the box: {(land & zone).sum() / land.sum() * 100:.1f} % of land')
for st in sys.argv[4:]:
    z = np.fromfile(f'{D}/{st}.f32', '<f4').reshape(n, n)
    a = excess(z[sub], (land & zone)[sub], x0 + m - 2, y0 + m - 2)
    b = excess(z[sub], (land & ~zone)[sub], x0 + m - 2, y0 + m - 2)
    print(f"{st:7s} in the zone: X6 {a['X6']:+.3f} X8 {a['X8']:+.3f} Xdiag {a['Xdiag']:+.3f} (kink {a['energy']:.2f} m2) | outside: X6 {b['X6']:+.3f} X8 {b['X8']:+.3f} Xdiag {b['Xdiag']:+.3f} (kink {b['energy']:.2f} m2)")
