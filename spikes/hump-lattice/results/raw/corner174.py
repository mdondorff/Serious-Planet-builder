"""#174: per stage pair (pre -> post), the trenches (cut over 10 m through an S_KH hump over 10 m, humps164b.py's definition) and the share whose S_KH crest sits at
an H1 corner (x and y within one cell of an H1 edge; uniform 6.25 %). Usage: corner174.py <chain dir> <x0> <y0> <pre:post>..."""
import sys, numpy as np
from scipy import ndimage
D, x0, y0 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
n, m = 8160, 1600
ld = lambda k: np.fromfile(f'{D}/{k}.f32', '<f4').reshape(n, n).astype(np.float64)
kh = ld('kh')
box = np.zeros((n, n), bool); box[m:n-m, m:n-m] = True
for pp in sys.argv[4:]:
    a, b = pp.split(':')
    cut = ld(a) - ld(b)
    low = (cut > 0.25) & box
    lab, k = ndimage.label(low, structure=np.ones((3, 3)))
    nb = ndimage.convolve(low.astype(np.int32), np.ones((3, 3), np.int32), mode='constant') - low
    ends = low & (nb <= 1)
    idx = np.arange(1, k + 1)
    mc = ndimage.maximum(cut, lab, idx)
    top = ndimage.maximum(kh, lab, idx)
    e = ndimage.maximum(np.where(ends, kh, -np.inf), lab, idx)
    ne = ~np.isfinite(e)
    e[ne] = ndimage.minimum(kh, lab, idx)[ne]
    ti = idx[(mc > 10) & (top - e > 10)]
    c = np.array(ndimage.maximum_position(kh, lab, ti)) if len(ti) else np.zeros((0, 2), int)
    gx, gy = c[:, 1] + x0, c[:, 0] + y0
    cor = np.isin(gx % 8, (0, 7)) & np.isin(gy % 8, (0, 7))
    print(f'{a} -> {b}: {len(ti)} trenches, S_KH crest at an H1 corner {cor.mean() * 100 if len(ti) else 0:.1f} % ({int(cor.sum())})')
